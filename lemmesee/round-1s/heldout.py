"""Round 1s: the held-out set, LiveSQLBench-Base-Full-v1, read once.

    python -I heldout.py prepare <corpus> <heldout>
    <guess> <heldout>/snapshots.jsonl <heldout>/guesses.jsonl
    python -I heldout.py score <frozen model> <heldout> <reader>

`prepare` reads the 22 `*_schema.txt` files in <heldout>/raw/ — PostgreSQL
DDL, each table followed by three sample rows, which are skipped — into
round 1p's form, as external.py reads Spider's and CTU's, and writes
hidden.jsonl, snapshots.jsonl, bench.jsonl (the paper's pairs, SchemaPile's
notebook's filters, the pairs whose two tables occur together in a
SchemaPile-Perm schema marked) and summary.json, with every line of a table
the parser did not read counted. `score` checks the frozen model's hashes
against <heldout>/../frozen.json, scores it once with external.py's measures
and over every declared key with intervals, and runs the Rust reader on the
same schemas; it refuses to run twice.
"""

import hashlib
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import letters  # noqa: E402  (first: the frozen model's features sort the trigrams)

import numpy as np  # noqa: E402

import external  # noqa: E402  (round 1p's)
from prepare import snapshot  # noqa: E402  (round 1b's)

SOURCE = "livesqlbench"
BLOCK = re.compile(r'^(?:"CREATE"|CREATE) TABLE ("?)([^"\s(]+)\1 \((.*?)\n\);', re.S | re.M)
WORD = r'"?[A-Za-z_][A-Za-z0-9_]*"?'
PK = re.compile(r'^"?PRIMARY"?\s+KEY\s*\(([^)]*)\)$', re.I)
FK = re.compile(r'^"?FOREIGN"?\s+KEY\s*\(([^)]*)\)\s+"?REFERENCES"?\s+(' + WORD + r')\s*\(([^)]*)\)$', re.I)
UNIQUE = re.compile(r'^"?UNIQUE"?\s*\(([^)]*)\)$', re.I)
COLUMN = re.compile(r'^("[^"]+"|[A-Za-z_][A-Za-z0-9_$]*)\s+(.*)$')
TYPE_END = re.compile(r'\s+(?:NOT\s+NULL|NULL|DEFAULT|PRIMARY|UNIQUE|REFERENCES|CHECK|COLLATE)\b', re.I)


def unquote(name: str) -> str:
    name = name.strip()
    return name[1:-1] if len(name) > 1 and name[0] == name[-1] == '"' else name


def names(group: str) -> list[str]:
    return [unquote(x) for x in group.split(",") if x.strip()]


def read_schema(text: str, how: Counter) -> list[dict]:
    """Tables in external.py's form: columns with type, NOT NULL and primary
    key position; one-row foreign keys as SQLite's pragma lists them."""
    raw = []
    for _, name, body in BLOCK.findall(text):
        columns, pk, uniques, fks = [], [], [], []
        for line in body.split("\n"):
            s = line.strip().rstrip(",").strip()
            if not s:
                continue
            if m := PK.match(s):
                pk = names(m.group(1))
                how["primary key"] += 1
            elif m := FK.match(s):
                fks.append((names(m.group(1)), unquote(m.group(2)), names(m.group(3))))
                how["foreign key"] += 1
            elif m := UNIQUE.match(s):
                uniques.append(names(m.group(1)))
                how["unique"] += 1
            elif m := COLUMN.match(s):
                rest = m.group(2)
                typ = TYPE_END.split(rest, maxsplit=1)[0].strip()
                columns.append({"name": unquote(m.group(1)), "type": typ, "notnull": bool(re.search(r"\bNOT\s+NULL\b",
                                                                                                    rest, re.I)),
                                "pk": 0})
                inline = [w for w in ("PRIMARY", "UNIQUE", "REFERENCES") if re.search(rf"\b{w}\b", rest, re.I)]
                if inline:
                    how["a column line with " + "/".join(inline) + " (not read)"] += 1
                how["column"] += 1
            else:
                how["a line not read"] += 1
        raw.append({"name": unquote(name), "columns": columns, "pk": pk, "uniques": uniques, "fks": fks})
    # Names resolved as PostgreSQL folds them: a column or table named in a
    # constraint is matched case-insensitively against those declared
    by_table = {t["name"].lower(): t for t in raw}
    tables = []
    for t in raw:
        col = {c["name"].lower(): c["name"] for c in t["columns"]}
        pk = [col.get(c.lower()) for c in t["pk"]]
        if None in pk:
            how["a primary key naming no column"] += 1
            pk = [c for c in pk if c]
        for i, c in enumerate(pk, 1):
            next(x for x in t["columns"] if x["name"] == c)["pk"] = i
        uniques = [[col[c.lower()] for c in u if c.lower() in col] for u in t["uniques"]]
        fks = []
        for fid, (src, target, ref) in enumerate(t["fks"]):
            to = by_table.get(target.lower())
            if to is None:
                how["a foreign key into a table not declared"] += 1
                continue
            to_col = {c["name"].lower(): c["name"] for c in to["columns"]}
            if any(c.lower() not in col for c in src) or any(c.lower() not in to_col for c in ref):
                how["a foreign key naming no column"] += 1
                continue
            for seq, (a, b) in enumerate(zip(src, ref)):
                fks.append([fid, seq, to["name"], col[a.lower()], to_col[b.lower()]])
        tables.append({"name": t["name"], "columns": t["columns"], "fks": fks, "uniques": uniques})
    return tables


def prepare(corpus: Path, out: Path) -> None:
    how = Counter()
    dbs = []
    for path in sorted((out / "raw").glob("*_schema.txt")):
        dbs.append({"source": SOURCE, "db": path.name.removesuffix("_schema.txt"),
                    "tables": read_schema(path.read_text(encoding="utf-8"), how)})
    perm = external.perm_strings(corpus)
    schemas = [external.hidden(db) for db in dbs]
    external.novelty(schemas, corpus / "round-1b")
    bench = []
    for db in dbs:
        for p in external.bench_pairs(db, perm):
            p["novel"] = next(s["novel"] for s in schemas if s["id"] == f"{SOURCE}/{db['db']}")
            bench.append(p)
    with open(out / "hidden.jsonl", "w", encoding="utf-8") as h, \
            open(out / "snapshots.jsonl", "w", encoding="utf-8") as sn:
        for s in schemas:
            h.write(json.dumps(s, ensure_ascii=False) + "\n")
            sn.write(json.dumps(snapshot(s), ensure_ascii=False) + "\n")
    with open(out / "bench.jsonl", "w", encoding="utf-8") as b:
        for p in bench:
            b.write(json.dumps(p, ensure_ascii=False) + "\n")
    kept = [p for p in bench if not p["occurs_in_perm"] and not p["too_large"]]
    summary = {
        "lines read": dict(how), "databases": len(schemas), "databases declaring a key": sum(1 for s in schemas if s["fk"]),
        "novel databases": sum(1 for s in schemas if s["novel"]), "tables": sum(len(s["tables"]) for s in schemas),
        "columns": sum(len(t["columns"]) for s in schemas for t in s["tables"]),
        "tables with a one-column primary key": sum(1 for s in schemas for t in s["tables"] if len(t["pk"]) == 1),
        "tables with no primary key": sum(1 for s in schemas for t in s["tables"] if not t["pk"]),
        "foreign keys declared, rows": sum(len(t["fks"]) for db in dbs for t in db["tables"]),
        "one-column keys declared": sum(len(s["fk"]) for s in schemas),
        "families": dict(Counter(c["family"] for s in schemas for t in s["tables"] for c in t["columns"])),
        "the paper's pairs before its filters": len(bench),
        "occurring together in Perm": sum(p["occurs_in_perm"] for p in bench),
        "too large for the paper's prompt": sum(p["too_large"] for p in bench), "kept": len(kept),
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=1))


def score(models: Path, out: Path, reader: Path) -> None:
    import refit  # noqa: E402
    import widen  # noqa: E402
    assert letters.is_sorted()
    done = out / "frozen-result.json"
    if done.exists() or (out / "score-frozen.json").exists():
        raise SystemExit("the held-out set has been scored once already")
    frozen = json.load(open(out.parent / "frozen.json", encoding="utf-8"))
    for f, k in (("keys.bin", "keys.bin sha256"), ("keys-first.ubj", "keys-first.ubj sha256"),
                 ("keys-second.txt", "keys-second.txt sha256")):
        assert hashlib.sha256((models / f).read_bytes()).hexdigest() == frozen[k], f
    threshold = json.load(open(models / "graph-final.json", encoding="utf-8"))["threshold"]
    assert threshold == frozen["threshold"]
    external.SOURCES = [SOURCE]  # external.py's measures by source, this one
    external.score(models, out, build=lambda job: widen.build((*job, "both")), suffix="-frozen")
    schemas = [json.loads(line) for line in open(out / "hidden.jsonl", encoding="utf-8")]
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["fk"]}
    drawn = {}
    for line in open(out / "open-predictions-frozen.jsonl", encoding="utf-8"):
        sid, t, c, g, _ = json.loads(line)
        drawn.setdefault(sid, set()).add((t, c, g))
    per = np.array([[len(truth[i] & drawn.get(i, set())), len(drawn.get(i, set()) - truth[i]),
                     len(truth[i] - drawn.get(i, set()))] for i in sorted(truth)], dtype=np.int64)
    result_by_db = {i: per[k].tolist() for k, i in enumerate(sorted(truth))}
    rng = np.random.default_rng(refit.EXTERNAL_SEED)
    idx = rng.integers(0, len(per), (1000, len(per)))
    boot = refit.resampled(per, idx)
    p, r, f = refit.prf(*per.sum(axis=0))
    result = {"frozen": frozen, "databases declaring a key": len(per), "keys": int(per[:, 0].sum() + per[:, 2].sum()),
              "over every declared key": {"precision": p, "recall": r, "f1": f,
                                          "precision_95": refit.interval(boot[:, 0]),
                                          "recall_95": refit.interval(boot[:, 1]), "f1_95": refit.interval(boot[:, 2]),
                                          "macro f1 over databases": float(np.mean([refit.prf(*c)[2] for c in per]))},
              "by database, tp fp fn": result_by_db}
    # The Rust reader on the same schemas
    stem = out / "rust-frozen"
    subprocess.run([str(reader), "parity", str(models / "keys.bin"), repr(threshold), str(out / "hidden.jsonl"),
                    "external", str(stem)], check=True)
    rust = set()
    for line in open(f"{stem}.keys.jsonl", encoding="utf-8"):
        sid, t, c, g, _ = json.loads(line)
        rust.add((sid, t, c, g))
    py = set()
    for line in open(out / "open-predictions-frozen.jsonl", encoding="utf-8"):
        sid, t, c, g, _ = json.loads(line)
        py.add((sid, t, c, g))
    result["Rust"] = {"keys drawn": len(rust), "drawn by Python only": sorted(py - rust),
                      "drawn by Rust only": sorted(rust - py)}
    done.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "prepare":
        prepare(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        score(Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]).resolve())
