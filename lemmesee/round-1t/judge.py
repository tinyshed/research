"""Round 1t: a blind judge of the keys three models draw on the style set
and no declaration names; added on 9 November 2026, the set development
data since its one reading.

    python -I judge.py pack [fresh]    # packets and the hidden key
    python -I judge.py count [fresh]   # after the judges wrote their answers

`fresh` (added on 9 November 2026, after the fresh set's one reading): the
same protocol on the fresh set, for `1s` and `nine-mended`, into
corpus/round-1t/judge-fresh/.

The pairs judged, for `1s`, `seven-restyled` and `seven-bounded` alike: every
pair one of them draws whose column no declaration names ("drawn, no key
declared" in errors.py's sorting); as controls, up to three declared keys a
database and up to three candidate pairs a database that no model draws and
no declaration names, so the judge's own accuracy is measured. Each pair is
shown once whichever models drew it, shuffled with the controls, under a
number; a packet holds a few databases, each the tables its pairs name, with
columns, types and primary keys and no declared key. The judge answers, for
each number, whether the column holds references to the rows of the table
named: yes, no or unsure. The key from numbers to pairs, models and kinds is
kept apart (corpus/round-1t/judge-style/key.json) and read only by `count`.

`count` gives the judge's answers on the controls (declared keys a judge
should call yes, the candidates no), and for each model its precision on the
set counted twice: declared keys only, as every F1 here, and declared keys
with the undeclared pairs the judge calls yes beside them.
"""

import json
import random
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import cues  # noqa: E402,F401  (first, as in fit.py)
import letters  # noqa: E402

import fit  # noqa: E402
import graph  # noqa: E402

SETS = {"style": ("judge-style", "heldout-github-style", "githubstyle-all", ("1s", "seven-restyled", "seven-bounded")),
        "fresh": ("judge-fresh", "heldout-github-style-fresh", "githubfresh", ("1s", "nine-mended"))}
D, FOLDER, TAG, MODELS = (fit.OUT / SETS["style"][0], *SETS["style"][1:])


def use(which: str) -> None:
    global D, FOLDER, TAG, MODELS
    folder, FOLDER, TAG, MODELS = SETS[which]
    D = fit.OUT / folder
SEED = 20261109
PER_PACKET = 60  # pairs a packet, about


def errors_of(n):
    return json.load(open(fit.OUT / "errors" / f"{n}--{TAG}.json", encoding="utf-8"))


def pack() -> None:
    schemas = {s["id"]: s for s in (json.loads(line) for line in
                                    open(fit.OUT / FOLDER / "hidden.jsonl", encoding="utf-8"))}
    rng = random.Random(SEED)
    pairs = {}  # (db, table, column, target) -> {"models": [...], "kind": ...}
    drawn_any = set()
    for n in MODELS:
        e = errors_of(n)
        for k in ("found", "drawn, no key declared", "drawn, wrong target"):
            for x in e["shown"][k]:
                drawn_any.add((x["database"], x["table"], x["column"]))
        for x in e["shown"]["drawn, no key declared"]:
            p = (x["database"], x["table"], x["column"], x["target"])
            pairs.setdefault(p, {"models": [], "kind": "undeclared, drawn"})["models"].append(n)
    # controls: declared keys, and candidate pairs no model draws
    with letters.pool(graph.WORKERS) as pool:
        _, meta = cues.build_all(pool, list(schemas.values()), "external", {}, "seven")
    cand = defaultdict(list)
    for m in meta:
        if not m[graph.LABEL] and (m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) not in drawn_any:
            cand[m[graph.SID]].append((m[graph.SID], m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]))
    declared_cols = {(s["id"], t, c) for s in schemas.values() for t, c, _ in s["fk"]}
    for sid, s in sorted(schemas.items()):
        for t, c, r in rng.sample(sorted(map(tuple, s["fk"])), min(3, len(s["fk"]))):
            pairs.setdefault((sid, t, c, r), {"models": [], "kind": "control: declared"})
        pool_ = [p for p in sorted(set(cand[sid])) if (p[0], p[1], p[2]) not in declared_cols]
        for p in rng.sample(pool_, min(3, len(pool_))):
            pairs.setdefault(p, {"models": [], "kind": "control: not a key"})
    by_db = defaultdict(list)
    for p in pairs:
        by_db[p[0]].append(p)
    dbs = sorted(by_db)
    rng.shuffle(dbs)
    packets, cur = [], []
    for db in dbs:
        cur.append(db)
        if sum(len(by_db[d]) for d in cur) >= PER_PACKET:
            packets.append(cur)
            cur = []
    if cur:
        packets.append(cur)
    (D / "packets").mkdir(parents=True, exist_ok=True)
    key = {}
    for k, group in enumerate(packets, 1):
        items = [p for d in group for p in by_db[d]]
        rng.shuffle(items)
        lines = [f"# Packet {k:02d}", ""]
        for j, d in enumerate(group, 1):
            s = schemas[d]
            named = {p[1] for p in items if p[0] == d} | {p[3] for p in items if p[0] == d}
            lines.append(f"## Database {k:02d}-{j}")
            lines.append("")
            for t in s["tables"]:
                if t["name"] in named:
                    cols = ", ".join(f"{c['name']} {c['type'] or c['family']}" for c in t["columns"])
                    pk = ", ".join(t["pk"]) or "none"
                    lines.append(f"- table `{t['name']}` (primary key: {pk}): {cols}")
            lines.append("")
        lines.append("## Pairs")
        lines.append("")
        for i, p in enumerate(items, 1):
            pid = f"{k:02d}-{i:03d}"
            j = group.index(p[0]) + 1
            lines.append(f"{pid}. database {k:02d}-{j}: does `{p[1]}.{p[2]}` hold references to the rows of `{p[3]}`?")
            key[pid] = {"pair": list(p), **pairs[p]}
        (D / "packets" / f"packet-{k:02d}.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (D / "key.json").write_text(json.dumps(key, indent=1) + "\n", encoding="utf-8")
    print(json.dumps({"packets": len(packets), "pairs": len(key),
                      "kinds": Counter(v["kind"] for v in key.values())}, indent=1))


def count() -> None:
    key = json.load(open(D / "key.json", encoding="utf-8"))
    answers = {}
    for f in sorted((D / "answers").glob("packet-*.json")):
        for pid, a in json.load(open(f, encoding="utf-8")).items():
            answers[pid] = str(a).strip().lower()
    out = {"answered": len(answers), "of": len(key)}
    by_kind = defaultdict(Counter)
    for pid, v in key.items():
        by_kind[v["kind"]][answers.get(pid, "missing")] += 1
    out["answers by kind"] = {k: dict(c) for k, c in by_kind.items()}
    real = {tuple(v["pair"]) for pid, v in key.items() if answers.get(pid) == "yes"}
    for n in MODELS:
        e = errors_of(n)
        k = e["kinds"]
        found = k.get("found", 0)
        undeclared = [(x["database"], x["table"], x["column"], x["target"]) for x in e["shown"]["drawn, no key declared"]]
        drawn = found + k.get("drawn, no key declared", 0) + k.get("drawn, wrong target", 0)
        yes = sum(p in real for p in undeclared)
        out[n] = {"drawn": drawn, "found (declared)": found, "drawn, no key declared": len(undeclared),
                  "of them, the judge's yes": yes, "precision, declared keys": round(found / drawn, 4),
                  "precision, declared keys and the judge's yes": round((found + yes) / drawn, 4)}
    # F1 with the judge: a key is right when declared, or undeclared and
    # called yes; the keys to find are the declared and every undeclared pair
    # of the three models' the judge called yes, each database's own
    import numpy as np
    from train import prf
    real_undeclared = {tuple(v["pair"]) for pid, v in key.items()
                       if v["kind"] == "undeclared, drawn" and answers.get(pid) == "yes"}
    dbs = sorted({x["database"] for x in errors_of(MODELS[0])["shown"]["found"]} |
                 {d for n in MODELS for d in errors_of(n)["per database"]})
    per = {}
    for n in MODELS:
        e = errors_of(n)
        rows = []
        for d in dbs:
            c = e["per database"].get(d, {})
            declared = sum(c.get(k, 0) for k in ("found", "below the threshold", "won by another target",
                                                 "not a candidate"))
            yes = sum(1 for x in e["shown"]["drawn, no key declared"]
                      if x["database"] == d and (x["database"], x["table"], x["column"], x["target"]) in real_undeclared)
            drawn = c.get("found", 0) + c.get("drawn, no key declared", 0) + c.get("drawn, wrong target", 0)
            tp = c.get("found", 0) + yes
            to_find = declared + sum(1 for p in real_undeclared if p[0] == d)
            rows.append([tp, drawn - tp, to_find - tp])
        per[n] = np.array(rows)
    idx = np.random.default_rng(20261101).integers(0, len(dbs), (1000, len(dbs)))
    boots = {n: np.array([prf(*per[n][i].sum(axis=0))[2] for i in idx]) for n in MODELS}
    for n in MODELS:
        p, r, f = prf(*per[n].sum(axis=0))
        out[n]["with the judge: P / R / F1"] = [round(p, 4), round(r, 4), round(f, 4)]
        out[n]["with the judge: F1, 95 %"] = [round(float(np.percentile(boots[n], q)), 4) for q in (2.5, 97.5)]
        if n != "1s":
            dlt = boots[n] - boots["1s"]
            out[n]["with the judge: F1 minus 1s, 95 %"] = [round(f - prf(*per["1s"].sum(axis=0))[2], 4)] + [
                round(float(np.percentile(dlt, q)), 4) for q in (2.5, 97.5)]
    out["undeclared pairs the judge called yes, any model"] = len(real_undeclared)
    (D / "count.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(out, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    use(sys.argv[2] if len(sys.argv) > 2 else "style")
    {"pack": pack, "count": count}[sys.argv[1]]()
