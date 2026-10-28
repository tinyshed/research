"""Round 1d's look at results: each fixture's fifteen best questions by
round 1c's ranker run on its database, beside round 1's five and round 1c's,
their results read for what a chart of them would show, packed blind for
judges; then a small model of what judges keep, learned from those results.

    python -I look.py pool <round 1c learn> <round 1d dir> <corpus/see> <out> <fixture-engine>...
    python -I look.py pack <out> <fixture-engine>...
    python -I look.py evaluate <out> <corpus/see> <round 1d dir> <fixture-engine>...

pool writes <out>/<fixture>/pool.json; pack writes <out>/packet-<n>.txt and
<out>/key.json; evaluate reads the judges' answers, <out>/<judge>-<n>.json,
learns the look from Sonnet's over databases left out one at a time, and
counts every five by both judges.
"""

from __future__ import annotations

import json
import math
import random
import re
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE.parent / "round-1c"))
sys.path.insert(0, str(HERE))
import baseline  # noqa: E402
import choose  # noqa: E402
import learn  # noqa: E402
import rank  # noqa: E402
import see  # noqa: E402
from aq import generate, identity, load  # noqa: E402

TOP = 15
SEED = 20261029


def num(v) -> float | None:
    try:
        x = float(v)
        return x if math.isfinite(x) else None
    except (TypeError, ValueError):
        return None


def result_features(p: dict) -> dict[str, float]:
    """What a chart of the result would show, as numbers: how many points,
    how they spread, what share one holds, how many are empty."""
    rows = p.get("rows") or []
    f: dict[str, float] = {"error": float(bool(p.get("error"))), "rows": float(len(rows)), "empty": float(not rows)}
    vals = [num(r[-1]) for r in rows]
    v = np.array([x for x in vals if x is not None], dtype=float)
    f["values_missing"] = (1 - len(v) / len(rows)) if rows else 1.0
    if len(v):
        tot = float(np.abs(v).sum()) or 1.0
        f["top_share"] = float(np.abs(v).max() / tot)
        f["distinct_values"] = float(len(set(np.round(v, 6))))
        f["all_equal"] = float(len(set(np.round(v, 6))) == 1)
        f["cv"] = float(v.std() / (abs(v.mean()) or 1.0))
        f["negative"] = float((v < 0).any())
        f["log_total"] = math.log1p(abs(float(v.sum())))
        f["zeros"] = float((v == 0).mean())
    else:
        for k in ("top_share", "distinct_values", "all_equal", "cv", "negative", "log_total", "zeros"):
            f[k] = float("nan")
    labels = [r[0] for r in rows] if rows and len(rows[0]) > 1 else []
    f["blank_labels"] = (sum(1 for x in labels if x is None or str(x).strip() == "") / len(labels)) if labels else float("nan")
    timed = p.get("bucket") is not None
    f["timed"] = float(timed)
    if timed and rows:
        days = [see.date_days(str(r[0])) for r in rows]
        days = [d for d in days if d is not None]
        if len(days) >= 2:
            span = max(days) - min(days)
            step = {"day": 1, "week": 7, "month": 30.4, "year": 365.25}.get(p.get("bucket"), 30.4)
            expected = span / step + 1
            f["span_days"] = span
            f["coverage"] = min(1.0, len(set(days)) / expected) if expected > 0 else float("nan")
            f["buckets"] = float(len(set(days)))
        else:
            f["span_days"], f["coverage"], f["buckets"] = 0.0, float("nan"), float(len(days))
    else:
        f["span_days"] = f["coverage"] = f["buckets"] = float("nan")
    return f


def model_and_names(learn_dir: Path):
    import lightgbm as lgb
    model = lgb.Booster(model_file=str(learn_dir / "ranker.txt"))
    return model, model.feature_name(), json.loads((learn_dir / "words.json").read_text(encoding="utf-8"))


def pool(learn_dir: Path, d1d: Path, see_dir: Path, out: Path, fixtures: list[str], top_n: int = TOP) -> None:
    """A fixture's pool; run again with a larger top, it keeps the panels it
    has and adds the next ones."""
    model, names, table = model_and_names(learn_dir)
    lam = json.loads((d1d / "penalties.json").read_text(encoding="utf-8"))["penalties"]
    for fe in fixtures:
        folder = see_dir / fe
        snap = json.loads((folder / "snapshot.json").read_text(encoding="utf-8"))
        engine, database = snap["engine"], snap["database"]
        schema = load(snap, True)
        qs = [q for q in generate(schema) if rank.veto(schema, q) is None]
        row = {"features": [rank.features(schema, q) for q in qs], "slots": [rank.slots(schema, q) for q in qs]}
        X, _, _ = learn.matrix_free(row, names, table)
        scores = model.predict(X) + np.array([rank.stats_points(schema, q) for q in qs])
        ids = [identity(schema, q) for q in qs]
        fives = {
            "round 1c": rank.choose(schema, qs, list(scores)),
            "round 1d, choice": choose.Pool(ids, scores).choose(lam),
        }
        order = list(np.argsort(-scores, kind="stable"))
        top, seen = [], set()
        for i in order:
            if ids[i] not in seen:
                seen.add(ids[i])
                top.append(int(i))
            if len(top) == top_n:
                break
        members = sorted(set(top) | {i for f in fives.values() for i in f}, key=lambda i: -scores[i])
        earlier = out / fe / "pool.json"
        if earlier.exists():
            old = json.loads(earlier.read_text(encoding="utf-8"))["panels"]
            have = {p["title"] for p in old}
            members = [i for i in members if qs[i].title(schema) not in have]
            prefix = f"{fe}:x{top_n}-"
        else:
            old, prefix = [], f"{fe}:"
        panels = list(old)
        for k, i in enumerate(members):
            p = see.panel_of(schema, qs[i], engine, database, f"{prefix}{k}")
            p["score"] = float(scores[i])
            p["rank"] = int(order.index(i)) + 1
            p["in"] = [n for n, f in fives.items() if i in f]
            p["result"] = result_features(p)
            panels.append(p)
        # round 1's five with statistics, as it ran
        r1 = json.loads((folder / "runs.json").read_text(encoding="utf-8"))
        r1_panels = {p["id"]: p for p in r1["panels"]}
        for pid in ([] if old else r1["configs"].get("B1", [])):
            p = dict(r1_panels[pid])
            p["id"] = f"{fe}:r1:{pid}"
            p["in"] = ["round 1"]
            p["result"] = result_features(p)
            panels.append(p)
        (out / fe).mkdir(parents=True, exist_ok=True)
        (out / fe / "pool.json").write_text(json.dumps({"fixture": fe, "note": r1.get("note", ""), "panels": panels}, ensure_ascii=False), encoding="utf-8")
        print(fe, len(panels), "panels,", len(members), "new;", sum(1 for p in panels if p.get("error")), "errors")


FORM = """You are judging panels proposed for a first overview dashboard of several databases. You did not make them,
and you are not told what made them. Do not read, search or open any file other than this one, and run no
command; when done, write your answers with the Write tool, once, to the path given to you.

For every panel answer:
 1. keep: would you keep this panel on a first overview of this database? true or false
 2. meaning: does the picture mean what it seems to? "yes", "no", or "cannot tell"
 3. wrong: if not, what is wrong, in a few words; else null
A panel is shown by its title, which says what it computes, and the first rows of its result as they came back
from the database. Judge what a person new to the database would want first, and whether the result shown is
worth a chart: a single bar, an empty result or a column of blanks is not.

Write one JSON object: {"panels": {"<panel id>": {"keep": bool, "meaning": "...", "wrong": str|null}, ...}} —
every panel id below must be answered.
"""


def pack(out: Path, fixtures: list[str], per: int = 5) -> None:
    """Every panel no judge has seen, in packets of five databases; the key
    keeps the names already given."""
    key = json.loads((out / "key.json").read_text(encoding="utf-8")) if (out / "key.json").exists() else {}
    judged = {i for k in key.values() for i in k["ids"]}
    first = len(list(out.glob("packet-*.txt")))
    rng = random.Random(SEED + first)
    packets = []
    order = fixtures[:]
    rng.shuffle(order)
    for n in range(0, len(order), per):
        body = [FORM]
        for fe in order[n:n + per]:
            data = json.loads((out / fe / "pool.json").read_text(encoding="utf-8"))
            note = re.sub(r"\s*(A development fixture|A check fixture( of round (1c|2))?)\.", "", data["note"]).replace(" Gold questions answered are marked.", "")
            by_title = {}
            for p in data["panels"]:
                t = p["title"].replace(", the twelve most", " the twelve most").replace(", by ", " by ")
                by_title.setdefault(t, []).append(p)
            titles = [t for t in by_title if not any(p["id"] in judged for p in by_title[t])]
            if not titles:
                continue
            rng.shuffle(titles)
            body.append(f"\n=== database: {fe.rsplit('-', 1)[0]} ===\n{note}\n")
            for k, t in enumerate(titles, 1):
                # the whole fixture's name: four letters of it made basketball's
                # panels and basketballwomen's one name, and their answers one
                name = f"{fe.rsplit('-', 1)[0]}-{first + 1}-{k:02d}"
                key[name] = {"fixture": fe, "ids": [p["id"] for p in by_title[t]]}
                p = by_title[t][0]
                rows = p.get("rows") or []
                lines = [" | ".join("∅" if v is None else str(v)[:40] for v in r) for r in rows[:12]]
                more = f"\n      … {len(rows) - 12} more rows" if len(rows) > 12 else ""
                err = f"\n      error: {str(p['error'])[:120]}" if p.get("error") else ""
                body.append(f"  panel {name}\n    title: {t}\n    rows ({len(rows)}):\n      " + "\n      ".join(lines) + more + err + "\n")
        if len(body) == 1:
            continue
        path = out / f"packet-{first + len(packets) + 1}.txt"
        path.write_text("\n".join(body), encoding="utf-8")
        packets.append(path.name)
    (out / "key.json").write_text(json.dumps(key, indent=1), encoding="utf-8")
    print(packets, len(key), "panels")


def labels(out: Path, judge: str) -> dict[str, dict]:
    """A judge's answers by panel id, from the packets' names."""
    key = json.loads((out / "key.json").read_text(encoding="utf-8"))
    ans = {}
    for p in sorted(out.glob(f"{judge}-*.json")):
        ans.update(json.loads(p.read_text(encoding="utf-8"))["panels"])
    by_id = {}
    for name, k in key.items():
        if name in ans:
            for pid in k["ids"]:
                by_id[pid] = ans[name]
    return by_id


RESULT = ["error", "rows", "empty", "values_missing", "top_share", "distinct_values", "all_equal", "cv", "negative",
          "log_total", "zeros", "blank_labels", "timed", "span_days", "coverage", "buckets"]


QUESTION: list[str] = []  # the question's features the look reads beside its result, set by evaluate


def vector(p: dict) -> list[float]:
    r = p["result"]
    q = p.get("question") or {}
    return ([r.get(k, float("nan")) for k in RESULT] + [p.get("score", float("nan")), float(p.get("rank", 99))]
            + [q.get(k, float("nan")) for k in QUESTION])


def fit_look(panels: list[dict], keep: dict[str, dict]):
    import lightgbm as lgb
    X = np.array([vector(p) for p in panels if p["id"] in keep], dtype=float)
    y = np.array([bool(keep[p["id"]]["keep"]) for p in panels if p["id"] in keep], dtype=int)
    params = dict(objective="binary", learning_rate=0.05, num_leaves=4, min_data_in_leaf=10, feature_fraction=0.5,
                  bagging_fraction=0.9, bagging_freq=1, lambda_l2=1.0, verbose=-1)
    return lgb.train(params, lgb.Dataset(X, y), num_boost_round=150)


def identities(see_dir: Path, fe: str) -> tuple[dict[str, tuple], dict[str, dict]]:
    """Each title's question, as an identity and as the ranker's features."""
    snap = json.loads((see_dir / fe / "snapshot.json").read_text(encoding="utf-8"))
    schema = load(snap, True)
    ids, feats = {}, {}
    for q in generate(schema):
        t = q.title(schema)
        if t not in ids:
            ids[t] = identity(schema, q)
            feats[t] = rank.features(schema, q)
    return ids, feats


def evaluate(out: Path, see_dir: Path, d1d: Path, fixtures: list[str], with_question: bool = True,
             teacher: str = "sonnet") -> None:
    """Databases left out one at a time: the look learned from Sonnet's
    answers on the others, the five chosen with it, and every five counted
    by every judge — Opus, Gemini, DeepSeek and GLM never taught anything."""
    lam = json.loads((d1d / "penalties.json").read_text(encoding="utf-8"))["penalties"]
    pools = {fe: json.loads((out / fe / "pool.json").read_text(encoding="utf-8"))["panels"] for fe in fixtures}
    sonnet, opus = labels(out, "sonnet"), labels(out, "opus")
    taught_by = labels(out, teacher)
    ident, qfeat = {}, {}
    for fe in fixtures:
        ident[fe], qfeat[fe] = identities(see_dir, fe)
        for p in pools[fe]:
            p["question"] = qfeat[fe].get(p["title"], {})
    if with_question:
        QUESTION[:] = sorted({k for fe in fixtures for f in qfeat[fe].values() for k in f})
    weights = (0.0, 0.5, 1.0, 2.0, 4.0, 8.0)
    fives = {w: {} for w in weights}
    for fe in fixtures:
        train = [p for f in fixtures if f != fe for p in pools[f]]
        model = fit_look(train, taught_by)
        mine = [p for p in pools[fe] if "round 1" not in p["in"]]
        pk = model.predict(np.array([vector(p) for p in mine], dtype=float))
        ids = [ident[fe].get(p["title"], ("?", p["title"])) for p in mine]
        for w in weights:
            logit = np.log(np.clip(pk, 1e-3, 1 - 1e-3) / (1 - np.clip(pk, 1e-3, 1 - 1e-3)))
            scores = np.array([p["score"] for p in mine]) + w * logit
            chosen = choose.Pool(ids, scores, size=len(mine)).choose(lam)
            fives[w][fe] = [mine[i]["id"] for i in chosen]
    systems = {
        "round 1, B1": {fe: [p["id"] for p in pools[fe] if "round 1" in p["in"]] for fe in fixtures},
        "round 1c": {fe: [p["id"] for p in pools[fe] if "round 1c" in p["in"]] for fe in fixtures},
        "round 1d, learned choice": {fe: [p["id"] for p in pools[fe] if "round 1d, choice" in p["in"]] for fe in fixtures},
    }
    for w in weights:
        systems[f"round 1d, choice and look w={w}"] = fives[w]

    def kept(judge, five):
        return sum(1 for pid in five if judge.get(pid, {}).get("keep"))

    def false(judge, five):
        return sum(1 for pid in five if judge.get(pid, {}).get("meaning") == "no")

    # Sonnet taught the look; the others, where they answered, only count. Jev
    # answers each panel alone, with a probability; the rest saw a database's
    # panels together
    judges = {"Sonnet": sonnet, "Opus": opus}
    for name, shown in (("gemini", "Gemini"), ("deepseek", "DeepSeek"), ("glm", "GLM"), ("jev", "Jev")):
        if list(out.glob(f"{name}-*.json")):
            judges[shown] = labels(out, name)
    # each judge counted only on the databases where it answered every panel
    # of every five: Opus did not judge the panels repacked after basketball's
    # and basketballwomen's names collided
    covered = {j: [fe for fe in fixtures if all(pid in ans for per in systems.values() for pid in per[fe])]
               for j, ans in judges.items()}
    result = {"databases counted": {j: len(c) for j, c in covered.items()}}
    for name, per in systems.items():
        row = {}
        for j, ans in judges.items():
            row[f"kept of five, {j}"] = round(float(np.mean([kept(ans, per[fe]) for fe in covered[j]])), 2)
        for j, ans in judges.items():
            row[f"false, {j}"] = sum(false(ans, per[fe]) for fe in covered[j])
        for j, ans in judges.items():
            if j != "Sonnet":
                row[f"per database, {j}"] = {fe.rsplit("-", 1)[0]: kept(ans, per[fe]) for fe in covered[j]}
        result[name] = row
    # the weight Sonnet's answers choose, and its difference from round 1 and round 1c by the others, bootstrapped
    best_w = max(weights, key=lambda w: np.mean([kept(taught_by, fives[w][fe]) for fe in fixtures]))
    chosen = systems[f"round 1d, choice and look w={best_w}"]
    rng = np.random.default_rng(SEED)
    for j, ans in judges.items():
        if j.lower() == teacher:
            continue
        for other in ("round 1, B1", "round 1c"):
            d = np.array([kept(ans, chosen[fe]) - kept(ans, systems[other][fe]) for fe in covered[j]], dtype=float)
            boots = [rng.choice(d, len(d)).mean() for _ in range(10000)]
            result[f"look w={best_w} − {other}, {j}, kept of five"] = {
                "mean": round(float(d.mean()), 2), "95%": [round(float(np.percentile(boots, 2.5)), 2), round(float(np.percentile(boots, 97.5)), 2)]}
    names = list(judges)
    for a in range(len(names)):
        for b in range(a + 1, len(names)):
            ja, jb = judges[names[a]], judges[names[b]]
            agree = [ja[k]["keep"] == jb[k]["keep"] for k in ja if k in jb]
            result[f"{names[a]} and {names[b]} agree on keep"] = round(float(np.mean(agree)), 3)
    result["weight chosen by Sonnet"] = best_w
    result["the look reads the question too"] = with_question
    result["the look taught by"] = teacher
    stem = "evaluate" + ("" if with_question else "-results-only") + ("" if teacher == "sonnet" else f"-taught-by-{teacher}")
    (out / f"{stem}.json").write_text(json.dumps(result, indent=1, ensure_ascii=False), encoding="utf-8")
    print(json.dumps({k: (v if not isinstance(v, dict) else {kk: vv for kk, vv in v.items() if not kk.startswith("per database")}) for k, v in result.items()}, indent=1, ensure_ascii=False))


def fit(out: Path, see_dir: Path, fixtures: list[str]) -> None:
    """The look from every database's answers by Sonnet, packed as round
    1c's ranker is."""
    import pack
    sonnet = labels(out, "sonnet")
    panels = []
    for fe in fixtures:
        _, qfeat = identities(see_dir, fe)
        for p in json.loads((out / fe / "pool.json").read_text(encoding="utf-8"))["panels"]:
            p["question"] = qfeat.get(p["title"], {})
            panels.append(p)
        QUESTION[:] = sorted(set(QUESTION) | {k for f in qfeat.values() for k in f})
    model = fit_look(panels, sonnet)
    model.save_model(str(out / "look.txt"))
    data = pack.pack(model)
    (out / "look.bin").write_bytes(data)
    print(f"look: {len(data)} bytes packed, {model.num_trees()} trees, {sum(1 for p in panels if p['id'] in sonnet)} answers")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "pool":
        top_n = next((int(a.split("=")[1]) for a in sys.argv if a.startswith("--top=")), TOP)
        pool(Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]), Path(sys.argv[5]), [a for a in sys.argv[6:] if not a.startswith("--")], top_n)
    elif cmd == "pack":
        pack(Path(sys.argv[2]), sys.argv[3:])
    elif cmd == "fit":
        fit(Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4:])
    elif cmd == "evaluate":
        args = [a for a in sys.argv[5:] if not a.startswith("--")]
        teacher = next((a.split("=")[1] for a in sys.argv if a.startswith("--teacher=")), "sonnet")
        evaluate(Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]), args, "--results-only" not in sys.argv, teacher)
