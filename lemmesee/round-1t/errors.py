"""Round 1t: where a fit loses keys on a development set, each declared key
and each drawn one sorted into what went wrong.

    python -I errors.py <model> <set> [<set> ...]

Sets are fit.py's names; only development sets are meant here (Spider, BIRD,
CTU, LiveSQLBench, SchemaPile's test split): a held-out set read through this
becomes development data. Writes corpus/round-1t/errors/<model>--<set>.json:

- a declared key is *not a candidate* (no pair names its column and target),
  *won by another target* (its column's best pair is another table), *below
  the threshold* (its column's best pair is it, scored under the threshold),
  or *found*;
- a drawn key that is not declared is on a column whose declared key is
  another target (*wrong target*) or on a column with no declared key
  (*no key declared*);
- per database the same counts, and up to 400 of each kind shown with the
  column, the target, the score and, for a key another target won, that
  target and its score.
"""

import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import cues  # noqa: E402  (first, as in fit.py)
import letters  # noqa: E402

import numpy as np  # noqa: E402

import fit  # noqa: E402
import graph  # noqa: E402
import stack  # noqa: E402
import widen  # noqa: E402

SHOWN = 400


def one(name, set_name, schemas, split, guesses, pool):
    d, variant = fit.MODELS[name]
    first, second = widen.load_models(d)
    final = json.load(open(d / "graph-final.json", encoding="utf-8"))
    threshold = final["threshold"]
    x, meta = cues.build_all(pool, schemas, split, guesses, variant)
    p2 = second.predict(stack.of(final)(meta, first.predict_proba(x)[:, 1]))
    best = {}
    for i, m in enumerate(meta):
        k = (m[graph.SID], m[graph.TABLE], m[graph.COLUMN])
        if k not in best or p2[i] > p2[best[k]]:
            best[k] = i
    pair = {(m[graph.SID], m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]): i for i, m in enumerate(meta)}
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["split"] == split and s["fk"]}
    kinds, per_db, shown = Counter(), defaultdict(Counter), defaultdict(list)

    def show(kind, row):
        kinds[kind] += 1
        per_db[row["database"]][kind] += 1
        if len(shown[kind]) < SHOWN:
            shown[kind].append(row)

    declared_cols = {}
    for sid, ks in truth.items():
        for t, c, r in sorted(ks):  # a column declared into two targets keeps the last, the same every run
            declared_cols[(sid, t, c)] = r
    for (sid, t, c), r in declared_cols.items():
        row = {"database": sid, "table": t, "column": c, "target": r}
        i = pair.get((sid, t, c, r))
        if i is None:
            show("not a candidate", row)
            continue
        row["score"] = round(float(p2[i]), 4)
        b = best[(sid, t, c)]
        if b != i:
            row["won by"], row["its score"] = meta[b][graph.TARGET], round(float(p2[b]), 4)
            show("won by another target", row)
        elif p2[i] < threshold:
            show("below the threshold", row)
        else:
            show("found", row)
    for k, i in best.items():
        if p2[i] < threshold:
            continue
        sid, t, c = k
        r = meta[i][graph.TARGET]
        if k in declared_cols and declared_cols[k] == r:
            continue
        row = {"database": sid, "table": t, "column": c, "target": r, "score": round(float(p2[i]), 4)}
        if k in declared_cols:
            row["declared"] = declared_cols[k]
            show("drawn, wrong target", row)
        else:
            show("drawn, no key declared", row)
    tp = kinds["found"]
    fp = kinds["drawn, wrong target"] + kinds["drawn, no key declared"]
    fn = len(declared_cols) - tp
    result = {"model": name, "set": set_name, "threshold": threshold, "declared keys": len(declared_cols),
              "pairs": len(meta), "kinds": dict(kinds),
              "F1": round(2 * tp / (2 * tp + fp + fn), 4) if tp else 0.0,
              "per database": {k: dict(v) for k, v in sorted(per_db.items(), key=lambda kv: -sum(kv[1].values()))},
              "shown": shown}
    out = fit.OUT / "errors"
    out.mkdir(exist_ok=True)
    (out / f"{name}--{set_name.split(' ')[0]}.json").write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    print(name, set_name, json.dumps({k: result[k] for k in ("declared keys", "pairs", "kinds", "F1")}), flush=True)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    assert letters.is_sorted()
    data = fit.sets()
    with letters.pool(graph.WORKERS) as pool:
        for set_name in sys.argv[2:]:
            one(sys.argv[1], set_name, *data[set_name], pool)
