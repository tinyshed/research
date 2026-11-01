"""Round 1p: round 1b's key model on the databases SchemaPile's paper tested
its foreign-key models on — Spider, BIRD and CTU's relational repository —
in the paper's task and in round 1b's.

    .venv/Scripts/python -I external.py prepare <corpus> <out>
    .venv/Scripts/python -I external.py score <models> <out>

`prepare` reads every database's schema — Spider's and BIRD's SQLite files,
CTU's MySQL dumps — and writes into <out>:
  hidden.jsonl     one database a line, as round 1b's, its declarations hidden
  snapshots.jsonl  the same databases for the rules' `guess`
  bench.jsonl      the paper's pairs: two tables and the one key between them,
                   chosen as its notebook `eval_data_prep.ipynb` chooses them
  summary.json     counts, and what was left out and why

`score` reads round 1b's two saved models and its threshold from <models>
(round 1b's own <out>) and writes score.json into <out>.
"""

import gzip
import hashlib
import json
import re
import shutil
import sqlite3
import sys
import tempfile
import time
import zipfile
from collections import Counter, defaultdict
from pathlib import Path

ROUND_1B = Path(__file__).resolve().parent.parent / "round-1b"
sys.path.insert(0, str(ROUND_1B))

import numpy as np  # noqa: E402

from prepare import family, snapshot  # noqa: E402

SEED = 20261101
LLM_MAX_CHARS = 8000  # the notebook's own bound on two tables' text
PREFIX = 32 << 20  # a BIRD file's first bytes, where its schema was written

# What the paper's figure reports, as its notebook's last cell has it:
# recall at one on its pairs, Spider, BIRD, CTU. Its sets are filtered
# against all of SchemaPile and its instruction data, not only Perm.
PAPER = {
    "Jaccard-Levenshtein (Valentine)": [0.58, 0.63, 0.36],
    "GPT-3.5": [0.88, 0.82, 0.86],
    "StarChat-alpha": [0.64, 0.61, 0.82],
    "T5-base, SchemaPile": [0.92, 0.83, 0.91],
    "StarCoder 15.5B, SchemaPile": [0.97, 0.94, 0.97],
}
SOURCES = ["spider", "bird", "ctu"]


# --- reading a schema -------------------------------------------------------

def sqlite_schema(path: Path) -> list[dict]:
    """Tables as SQLite's pragmas give them, in sqlite_master's order."""
    con = sqlite3.connect(path.resolve().as_uri() + "?immutable=1", uri=True)
    try:
        tables = []
        for (name,) in con.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall():
            q = name.replace("'", "''")
            columns = [{"name": c[1], "type": c[2] or "", "notnull": bool(c[3]), "pk": c[5]}
                       for c in con.execute(f"PRAGMA table_info('{q}')").fetchall()]
            fks = [list(r[:5]) for r in con.execute(f"PRAGMA foreign_key_list('{q}')").fetchall()]
            uniques = []
            for _, iname, unique, origin, *_ in con.execute(f"PRAGMA index_list('{q}')").fetchall():
                if unique and origin != "pk":
                    qi = iname.replace("'", "''")
                    uniques.append([r[2] for r in con.execute(f"PRAGMA index_info('{qi}')").fetchall()])
            tables.append({"name": name, "columns": columns, "fks": fks, "uniques": uniques})
        return tables
    finally:
        con.close()


def zipped_schema(z: zipfile.ZipFile, info: zipfile.ZipInfo, tmp: Path) -> tuple[list[dict], str]:
    """A schema from a file inside an archive: its first bytes when they hold
    the whole schema, the whole file when they do not."""
    with z.open(info) as src, open(tmp, "wb") as dst:
        dst.write(src.read(PREFIX))
    try:
        return sqlite_schema(tmp), "prefix"
    except sqlite3.DatabaseError:
        with z.open(info) as src, open(tmp, "wb") as dst:
            shutil.copyfileobj(src, dst, 1 << 24)
        return sqlite_schema(tmp), "whole"
    finally:
        tmp.unlink(missing_ok=True)


def split_top(body: str) -> list[str]:
    """A column definition's words, a parenthesis or a quote kept whole."""
    words, cur, depth, quote = [], "", 0, None
    for ch in body:
        if quote:
            cur += ch
            if ch == quote:
                quote = None
        elif ch in "'\"":
            quote = ch
            cur += ch
        elif ch == "(":
            depth += 1
            cur += ch
        elif ch == ")":
            depth -= 1
            cur += ch
        elif ch == " " and depth == 0:
            if cur:
                words.append(cur)
            cur = ""
        else:
            cur += ch
    if cur:
        words.append(cur)
    return words


TICKED = re.compile(r"`([^`]+)`")


def mysql_schema(text: str) -> list[dict]:
    """Tables from a mysqldump, foreign keys emulated as SQLite's pragma
    lists them: a row a column, one id a constraint."""
    tables = []
    for name, body in re.findall(r"CREATE TABLE `([^`]+)` \((.*?)\n\)[^;\n]*;", text, re.S):
        columns, pk, uniques, fks = [], [], [], []
        for line in body.split("\n"):
            line = line.strip().rstrip(",")
            if line.startswith("`"):
                cname = TICKED.match(line).group(1)
                words = split_top(line[len(cname) + 2:].strip())
                typ = words[0] if words else ""
                if len(words) > 1 and words[1] in ("unsigned", "signed"):
                    typ += " " + words[1]
                columns.append({"name": cname, "type": typ, "notnull": "NOT NULL" in line, "pk": 0})
            elif line.startswith("PRIMARY KEY"):
                pk = TICKED.findall(line)
            elif line.startswith("UNIQUE KEY"):
                uniques.append(TICKED.findall(line)[1:])
            elif line.startswith("CONSTRAINT") and "FOREIGN KEY" in line:
                # A reference may name another database, whose table is then
                # not one of this schema's
                m = re.match(r"CONSTRAINT `[^`]+` FOREIGN KEY \(([^)]*)\) REFERENCES (`[^`]+`\.)?`([^`]+)` \(([^)]*)\)",
                             line)
                if m.group(2):
                    continue
                src, ref = TICKED.findall(m.group(1)), TICKED.findall(m.group(4))
                fid = len({f[0] for f in fks})
                fks.extend([fid, seq, m.group(3), a, b] for seq, (a, b) in enumerate(zip(src, ref)))
        for i, c in enumerate(pk, 1):
            for col in columns:
                if col["name"] == c:
                    col["pk"] = i
        tables.append({"name": name, "columns": columns, "fks": fks, "uniques": uniques})
    return tables


def read_all(corpus: Path) -> tuple[list[dict], Counter]:
    """Every database of the three sources, with how each schema was read."""
    dbs, how = [], Counter()
    for path in sorted((corpus / "spider/spider_data/test_database").glob("*/*.sqlite")):
        dbs.append({"source": "spider", "db": path.stem, "tables": sqlite_schema(path)})
        how["spider: file"] += 1
    with tempfile.TemporaryDirectory() as tmpdir:
        for part in ("train/train_databases.zip", "dev/dev_databases.zip"):
            with zipfile.ZipFile(corpus / "bird/raw" / part) as z:
                for info in z.infolist():
                    if info.filename.startswith("__MACOSX") or not info.filename.endswith(".sqlite"):
                        continue
                    tables, read = zipped_schema(z, info, Path(tmpdir) / "db.sqlite")
                    dbs.append({"source": "bird", "db": Path(info.filename).stem, "tables": tables})
                    how[f"bird: {read}"] += 1
    # The structure of all of CTU's databases once dumped, else the 48 whole
    # dumps earlier rounds fetched
    ctu = corpus / "ctu-schema" if (corpus / "ctu-schema/databases.txt").exists() else corpus / "ctu"
    for path in sorted(ctu.glob("*.sql")):
        dbs.append({"source": "ctu", "db": path.stem,
                    "tables": mysql_schema(path.read_text(encoding="utf-8", errors="replace"))})
        how["ctu: dump"] += 1
    return dbs, how


# --- what round 1b's model is shown -----------------------------------------

def sql_family(raw: str, source: str) -> tuple[str, str]:
    """A declared type as round 1b's families name it, and its bare name."""
    t = raw.lower().split("(")[0].strip()
    t = re.sub(r"\s+(unsigned|signed|zerofill)\b", "", t).replace(" ", "")
    if t in ("enum", "set"):
        return "text", "varchar"  # a closed list hidden, as round 1b hides it
    if t == "text" and source != "ctu":
        return "text", t  # SQLite's text is any string, its varchar
    if t.startswith("int") or t.endswith("int"):
        return "int", t
    return family(t), t


def hidden(db: dict) -> dict:
    """A database as round 1b's hidden.jsonl shows a schema: names, types,
    nullability, uniqueness and primary keys, no index, no declaration."""
    tables, fk = [], []
    names = {t["name"].lower(): t["name"] for t in db["tables"]}
    for t in db["tables"]:
        if t["name"].lower().startswith("sqlite_"):
            continue
        pk = [c["name"] for c in sorted((c for c in t["columns"] if c["pk"]), key=lambda c: c["pk"])]
        unique = {u[0] for u in t["uniques"] if len(u) == 1}
        columns = []
        for i, c in enumerate(t["columns"]):
            fam, typ = sql_family(c["type"], db["source"])
            columns.append({"name": c["name"], "family": fam, "type": typ, "nullable": not c["notnull"],
                            "unique": c["name"] in unique and c["name"] not in pk, "primary": c["name"] in pk,
                            "index": False, "position": i})
        rows = Counter(f[0] for f in t["fks"])
        for fid, _, target, col, _ in t["fks"]:
            to = names.get((target or "").lower())
            if rows[fid] == 1 and to and [t["name"], col, to] not in fk:
                fk.append([t["name"], col, to])
        tables.append({"name": t["name"], "pk": pk, "columns": columns})
    sid = f"{db['source']}/{db['db']}"
    signature = hashlib.sha256(json.dumps(
        sorted([t["name"].lower(), sorted(c["name"].lower() for c in t["columns"])] for t in tables)
    ).encode()).hexdigest()[:16]
    return {"id": sid, "repo": sid, "source": db["source"], "licence": "", "signature": signature,
            "split": "external", "tables": tables, "fk": fk, "closed": []}


# --- the paper's pairs ------------------------------------------------------

def table_string(name: str, columns: list[str], sort: bool = False) -> str:
    if sort:
        columns = sorted(columns)
    return name.lower() + "(" + ", ".join(c.lower() for c in columns) + ")"


def perm_strings(corpus: Path) -> dict[str, set[int]]:
    """Each SchemaPile-Perm table as the notebook writes it sorted, and the
    schemas it occurs in."""
    data = json.load(gzip.open(corpus / "schemapile-perm/12682521/schemapile-perm.json.gz", "rt", encoding="utf-8"))
    where = defaultdict(set)
    for i, s in enumerate(data.values()):
        for name, t in s["TABLES"].items():
            where[table_string(name, list(t["COLUMNS"]), True)].add(i)
    return where


def bench_pairs(db: dict, perm: dict[str, set[int]]) -> list[dict]:
    """The notebook's `collect_foreign_keys` for one database, its quirks
    kept: a second row of a constraint drops it and a third brings it back,
    and two keys between the same tables drop both unless a third follows."""
    columns = {t["name"]: [c["name"] for c in t["columns"]] for t in db["tables"]}
    out = []
    for t in db["tables"]:
        by_id, rows = {}, Counter(f[0] for f in t["fks"])
        for fid, _, target, col, ref in t["fks"]:
            if fid in by_id:
                del by_id[fid]
            elif target in columns and ref in columns[target] and col in columns[t["name"]]:
                ts, fs = table_string(t["name"], columns[t["name"]]), table_string(target, columns[target])
                both = perm.get(table_string(t["name"], columns[t["name"]], True), set()) & \
                    perm.get(table_string(target, columns[target], True), set())
                by_id[fid] = {"source": db["source"], "db": db["db"], "table": t["name"], "column": col,
                              "foreign_table": target, "referred_column": ref, "occurs_in_perm": bool(both),
                              "too_large": len(ts) + len(fs) > LLM_MAX_CHARS, "composite": rows[fid] > 1}
        by_combination = {}
        for p in by_id.values():
            k = (p["table"], p["foreign_table"])
            if k in by_combination:
                del by_combination[k]
            else:
                by_combination[k] = p
        out.extend(by_combination.values())
    # The notebook keys its prompts by database and lower-cased tables
    keyed = {(p["db"], p["table"].lower(), p["foreign_table"].lower()): p for p in out}
    return list(keyed.values())


def novelty(schemas: list[dict], round_1b: Path) -> None:
    """A database is novel when no schema round 1b trained on shares half its
    tables' names, as round 1b's prepare.py decides it."""
    names = lambda s: {"".join(ch for ch in t["name"].lower() if ch.isalnum()) for t in s["tables"]}  # noqa: E731
    by_name, train = {}, []
    for line in open(round_1b / "hidden.jsonl", encoding="utf-8"):
        s = json.loads(line)
        if s["split"] == "train":
            train.append(names(s))
            for n in train[-1]:
                by_name.setdefault(n, []).append(len(train) - 1)
    for s in schemas:
        mine = names(s)
        seen = {i for n in mine for i in by_name.get(n, [])}
        best = max((len(mine & train[i]) / len(mine | train[i]) for i in seen), default=0.0)
        s["novel"] = best < 0.5
        s["closest_training_jaccard"] = round(best, 3)


def prepare(corpus: Path, out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    dbs, how = read_all(corpus)
    print(f"read {len(dbs)} databases in {time.time() - t0:.0f} s: {dict(how)}", flush=True)
    perm = perm_strings(corpus)
    schemas = [hidden(db) for db in dbs]
    novelty(schemas, corpus / "round-1b")
    novel = {s["id"]: s["novel"] for s in schemas}
    bench = []
    for db in dbs:
        for p in bench_pairs(db, perm):
            p["novel"] = novel[f"{db['source']}/{db['db']}"]
            bench.append(p)
    with open(out / "hidden.jsonl", "w", encoding="utf-8") as h, \
            open(out / "snapshots.jsonl", "w", encoding="utf-8") as sn:
        for s in schemas:
            h.write(json.dumps(s, ensure_ascii=False) + "\n")
            sn.write(json.dumps(snapshot(s), ensure_ascii=False) + "\n")
    with open(out / "bench.jsonl", "w", encoding="utf-8") as b:
        for p in bench:
            b.write(json.dumps(p, ensure_ascii=False) + "\n")
    summary = {"read": dict(how)}
    for src in SOURCES:
        mine = [s for s in schemas if s["source"] == src]
        pairs = [p for p in bench if p["source"] == src]
        kept = [p for p in pairs if not p["occurs_in_perm"] and not p["too_large"]]
        summary[src] = {
            "databases": len(mine), "databases declaring a key": sum(1 for s in mine if s["fk"]),
            "novel databases": sum(1 for s in mine if s["novel"]),
            "tables": sum(len(s["tables"]) for s in mine),
            "one-column keys declared": sum(len(s["fk"]) for s in mine),
            "paper's pairs before its filters": len(pairs),
            "occurring together in Perm": sum(p["occurs_in_perm"] for p in pairs),
            "too large for the paper's prompt": sum(p["too_large"] for p in pairs),
            "kept": len(kept), "kept from a constraint over several columns": sum(p["composite"] for p in kept),
            "kept from novel databases": sum(p["novel"] for p in kept),
        }
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=2))


# --- scoring ----------------------------------------------------------------

def scorer(models: Path, build=None):
    """Two stages as saved, scoring a list of schemas; `build` makes the
    pairs and their features, round 1b's own by default."""
    import graph  # noqa: E402
    import lightgbm as lgb  # noqa: E402
    import xgboost as xgb  # noqa: E402

    first = xgb.XGBClassifier()
    first.load_model(str(models / "keys-first.ubj"))
    first.set_params(device="cpu")
    second = lgb.Booster(model_file=str(models / "keys-second.txt"))

    def run(schemas, guesses):
        x, meta = (build or graph.build)((schemas, "external", guesses))
        if not meta:
            return np.zeros(0), meta
        s1 = first.predict_proba(x)[:, 1]
        return second.predict(graph.graph_features(meta, s1)), meta

    return run, graph


def choose(rows, scores, meta, graph):
    """The column the scores put first among a pair's rows, and the one the
    name score alone does, ties to the earlier column."""
    if not rows:
        return None, None
    best = max(rows, key=lambda i: (scores[i], -i))
    by_name = max(rows, key=lambda i: (meta[i][graph.BASE], -i))
    return meta[best][graph.COLUMN], meta[by_name][graph.COLUMN]


def resample(groups: np.ndarray, values: np.ndarray, rng, n: int = 1000) -> list[float]:
    """A 95 % interval of a mean, resampling whole databases."""
    ids = np.unique(groups)
    index = {g: np.flatnonzero(groups == g) for g in ids}
    means = []
    for _ in range(n):
        pick = np.concatenate([index[g] for g in rng.choice(ids, len(ids))])
        means.append(values[pick].mean())
    return [float(np.percentile(means, 2.5)), float(np.percentile(means, 97.5))]


def score(models: Path, out: Path, build=None, suffix: str = "") -> None:
    sys.path.insert(0, str(ROUND_1B))
    from train import bootstrap, counts, prf  # noqa: E402
    import keys  # noqa: E402

    t0 = time.time()
    run, graph = scorer(models, build)
    threshold = json.load(open(models / "graph-final.json", encoding="utf-8"))["threshold"]
    schemas = [json.loads(line) for line in open(out / "hidden.jsonl", encoding="utf-8")]
    by_id = {s["id"]: s for s in schemas}
    guesses = {}
    for line in open(out / "guesses.jsonl", encoding="utf-8"):
        g = json.loads(line)
        guesses[(g["schema"], g["table"], g["column"])] = g
    bench = [json.loads(line) for line in open(out / "bench.jsonl", encoding="utf-8")]
    rng = np.random.default_rng(SEED)
    result = {"threshold": threshold, "paper": PAPER, "paper_order": SOURCES}

    # Round 1b's task: every column of a database, its targets unnamed
    scores, meta = run(schemas, guesses)
    kept = keys.one_target(scores, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
    y = np.array([m[graph.LABEL] for m in meta], dtype=bool)
    rules = np.array([m[graph.RULES] for m in meta], dtype=bool)
    sid = np.array([m[graph.SID] for m in meta])
    src = np.array([by_id[s]["source"] for s in sid])
    novel = np.array([by_id[s]["novel"] for s in sid], dtype=bool)
    predicted = kept >= threshold
    # The keys it would draw, for measures against all that is declared
    with open(out / f"open-predictions{suffix}.jsonl", "w", encoding="utf-8") as f:
        for i in np.flatnonzero(predicted):
            m = meta[i]
            f.write(json.dumps([m[graph.SID], m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET], float(kept[i])],
                               ensure_ascii=False) + "\n")
    open_task = {}
    for name in SOURCES + ["all"]:
        for subset in ("all", "novel"):
            mask = (src == name) if name != "all" else np.ones_like(y)
            if subset == "novel":
                mask = mask & novel
            if not mask.any() or not y[mask].any():
                continue
            low, high = bootstrap(y[mask], predicted[mask], rules[mask], sid[mask], rng)
            declared = sum(len(s["fk"]) for s in schemas if (name == "all" or s["source"] == name)
                           and (subset == "all" or s["novel"]))
            open_task[f"{name}, {subset}"] = {
                "databases": len(set(sid[mask])), "pairs": int(mask.sum()), "keys": int(y[mask].sum()),
                "one-column keys declared": declared,
                "rules": dict(zip(("precision", "recall", "f1"), prf(*counts(y[mask], rules[mask])))),
                "model": dict(zip(("precision", "recall", "f1"), prf(*counts(y[mask], predicted[mask])))),
                "f1_minus_rules_95": [low, high],
            }
    result["round 1b's task"] = open_task
    print("round 1b's task scored", flush=True)

    # The paper's task: two tables known to hold one key between them
    rows_of = defaultdict(list)
    for i, m in enumerate(meta):
        rows_of[(m[graph.SID], m[graph.TABLE], m[graph.TARGET])].append(i)
    minis = []
    for j, p in enumerate(bench):
        s = by_id[f"{p['source']}/{p['db']}"]
        tables = [t for t in s["tables"] if t["name"] in (p["table"], p["foreign_table"])]
        minis.append({"id": f"pair-{j}", "repo": s["id"], "split": "external", "novel": s["novel"],
                      "tables": tables, "fk": [[p["table"], p["column"], p["foreign_table"]]], "closed": []})
    mini_scores, mini_meta = run(minis, {})
    mini_rows = defaultdict(list)
    for i, m in enumerate(mini_meta):
        mini_rows[(m[graph.SID], m[graph.TABLE], m[graph.TARGET])].append(i)

    outcomes = []
    for j, p in enumerate(bench):
        s = by_id[f"{p['source']}/{p['db']}"]
        target = next(t for t in s["tables"] if t["name"] == p["foreign_table"])
        rows = rows_of[(s["id"], p["table"], p["foreign_table"])]
        # The target's key as the pairs took it, a pseudo key included
        key = meta[rows[0]][14] if rows and len(meta[rows[0]]) > 14 else \
            target["pk"][0] if len(target["pk"]) == 1 else None
        whole, name_whole = choose(rows, scores, meta, graph)
        pair, _ = choose(mini_rows[(f"pair-{j}", p["table"], p["foreign_table"])], mini_scores, mini_meta, graph)
        right_key = key is not None and key.lower() == p["referred_column"].lower()
        hit = lambda c: bool(c and right_key and c.lower() == p["column"].lower())  # noqa: E731
        why = ("referred column is not the target's one-column key" if not right_key
               else "no candidate of the key's kind" if whole is None else "")
        outcomes.append({**p, "key": key, "two tables": pair, "whole database": whole, "name score": name_whole,
                         "hit two tables": hit(pair), "hit whole database": hit(whole), "hit name score": hit(name_whole),
                         "unanswerable": why})
    with open(out / f"bench-outcomes{suffix}.jsonl", "w", encoding="utf-8") as f:
        for o in outcomes:
            f.write(json.dumps(o, ensure_ascii=False) + "\n")

    paper_task = {}
    sets = {
        "the paper's filters": lambda o: not o["occurs_in_perm"] and not o["too_large"],
        "and no constraint over several columns": lambda o: not o["occurs_in_perm"] and not o["too_large"]
        and not o["composite"],
        "and novel databases only": lambda o: not o["occurs_in_perm"] and not o["too_large"] and o["novel"],
        "no filter": lambda o: True,
    }
    for set_name, keep in sets.items():
        for name in SOURCES:
            chosen = [o for o in outcomes if o["source"] == name and keep(o)]
            if not chosen:
                continue
            groups = np.array([o["db"] for o in chosen])
            entry = {"pairs": len(chosen), "databases": len(set(groups)),
                     "unanswerable": dict(Counter(o["unanswerable"] for o in chosen if o["unanswerable"]))}
            for how in ("two tables", "whole database", "name score"):
                v = np.array([o[f"hit {how}"] for o in chosen], dtype=float)
                entry[how] = {"recall_at_1": float(v.mean()), "95": resample(groups, v, rng)}
            paper_task[f"{name}: {set_name}"] = entry
    result["the paper's task"] = paper_task
    result["seconds"] = round(time.time() - t0)
    (out / f"score{suffix}.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "prepare":
        prepare(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        score(Path(sys.argv[2]), Path(sys.argv[3]))
