"""Round 1t: a column's "has it a key" decided apart from "which target".

    python -I pi.py <model> [<model> ...]

Proposed on 8 November 2026 by a design agent that measured, on
`cues-restyled`'s scores, that a perfect has-a-key decision with today's
targets would read CTU 0.868 and LiveSQLBench 0.782, and perfect targets with
today's decision 0.662 and 0.609. The frozen model's two stages score every
pair; each column keeps its best target, as `keys.one_target` does; a
LightGBM over the column's row — its best and second scores of both stages,
the gap, how many candidates, the entropy of its scores, where its best falls
among the schema's columns' bests, the schema's best and mean, the best
pair's name cues, whether its table declares a key and how many of its
table's columns are keyed — learns whether that best target is the declared
one, on half of SchemaPile's validation split (its renamed copies with it,
split by repository), its threshold the best F1 over every declared key on
the other half. It is measured on the development sets as `fit.py score`
measures, beside the model's own decision, into
corpus/round-1t/scored/<model>+pi.json.
"""

import hashlib
import json
import sys
import time
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import cues  # noqa: E402  (first: the paths and the sorted trigrams)
import fit  # noqa: E402
import graph  # noqa: E402
import keys  # noqa: E402
import letters  # noqa: E402
import stack  # noqa: E402
import widen  # noqa: E402
import lightgbm as lgb  # noqa: E402
import numpy as np  # noqa: E402
from train import prf  # noqa: E402

COLUMNS = ["best second stage", "next second stage", "gap", "best first stage", "next first stage", "candidates",
           "entropy", "place among the schema's bests", "schema best", "schema mean best", "base", "same type",
           "named as the key", "ends in id", "bare", "self", "table declares a key", "table's columns keyed",
           "columns of the table", "tables of the schema"]


def rows(schemas, meta, s1, s2):
    """One row a column: its features, its best pair's index, and whether that pair is the declared key."""
    tables = {}
    for s in schemas:
        for t in s["tables"]:
            tables[(s["id"], t["name"])] = (t, len(s["tables"]))
    by_col = defaultdict(list)
    for i, m in enumerate(meta):
        by_col[(m[graph.SID], m[graph.TABLE], m[graph.COLUMN])].append(i)
    bests_by_schema = defaultdict(list)
    best_of = {}
    for k, idx in by_col.items():
        b = max(idx, key=lambda i: (s2[i], -i))
        best_of[k] = b
        bests_by_schema[k[0]].append(s2[b])
    stats = {sid: (max(v), float(np.mean(v)), np.sort(v)) for sid, v in bests_by_schema.items()}
    keyed = defaultdict(int)
    for k in by_col:
        keyed[(k[0], k[1])] += 1
    out, which, label = [], [], []
    for k, idx in by_col.items():
        b = best_of[k]
        sc2 = np.sort(s2[idx])[::-1]
        sc1 = np.sort(s1[idx])[::-1]
        p = s2[idx] / max(s2[idx].sum(), 1e-9)
        ent = float(-(p * np.log(p + 1e-12)).sum())
        top, mean, ordered = stats[k[0]]
        place = np.searchsorted(ordered, s2[b]) / max(1, len(ordered) - 1)
        m = meta[b]
        t, n_tables = tables[(k[0], k[1])]
        out.append([sc2[0], sc2[1] if len(sc2) > 1 else 0.0, sc2[0] - (sc2[1] if len(sc2) > 1 else 0.0),
                    sc1[0], sc1[1] if len(sc1) > 1 else 0.0, len(idx), ent, min(place, 1.0), top, mean,
                    m[graph.BASE], m[graph.SAME_TYPE], m[graph.NAMED_KEY], m[graph.ENDS_ID], m[graph.BARE],
                    float(m[graph.SELF]), float(bool(t["pk"])), keyed[(k[0], k[1])] / max(1, len(t["columns"])),
                    min(len(t["columns"]), 60) / 60, min(n_tables, 100) / 100])
        which.append(b)
        label.append(bool(m[graph.LABEL]))
    return np.array(out, dtype=np.float32), np.array(which), np.array(label)


def count(schemas, split, meta, which, keep):
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["split"] == split and s["fk"]}
    preds = defaultdict(set)
    for i in which[keep]:
        m = meta[i]
        preds[m[graph.SID]].add((m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]))
    ids = sorted(truth)
    return ids, np.array([[len(truth[i] & preds[i]), len(preds[i] - truth[i]), len(truth[i] - preds[i])]
                          for i in ids], dtype=np.int64)


def main(names):
    assert letters.is_sorted()
    data = fit.sets()
    sp, sp_guesses = keys.load(fit.ROOT / "round-1b")
    restyle_val = [s for s in sp if s["split"] == "validation" and s["fk"]]
    restyle_val += [r for r in (fit.restyled(s, fit.RESTYLE_SEED, "~restyled") for s in restyle_val) if r]
    with letters.pool(graph.WORKERS) as pool:
        for name in names:
            d, variant = fit.MODELS[name]
            first, second = widen.load_models(d)
            gf = stack.of(json.load(open(d / "graph-final.json", encoding="utf-8")))

            def scores(schemas, split, guesses):
                x, meta = cues.build_all(pool, schemas, split, guesses, variant)
                s1 = first.predict_proba(x)[:, 1]
                return meta, s1, second.predict(gf(meta, s1))

            t0 = time.time()
            meta, s1, s2 = scores(restyle_val, "validation", sp_guesses)
            X, which, y = rows(restyle_val, meta, s1, s2)
            repo_of = {s["id"]: s["repo"] for s in restyle_val}
            half = np.array([int(hashlib.sha256(repo_of[meta[i][graph.SID]].encode()).hexdigest(), 16) % 2
                             for i in which])
            model = lgb.LGBMClassifier(n_estimators=300, num_leaves=31, learning_rate=0.05, min_child_samples=50,
                                       subsample=0.8, subsample_freq=1, random_state=fit.SEED, verbose=-1)
            model.fit(X[half == 0], y[half == 0])
            p2 = model.predict_proba(X[half == 1])[:, 1]
            v2_schemas = [s for s in restyle_val if int(hashlib.sha256(s["repo"].encode()).hexdigest(), 16) % 2 == 1]
            best_t, best_f = 0.5, -1.0
            for t in np.arange(0.05, 0.95, 0.01):
                _, per = count(v2_schemas, "validation", meta, which[half == 1], p2 >= t)
                f = prf(*per.sum(axis=0))[2]
                if f > best_f:
                    best_t, best_f = float(t), f
            print(f"{name}+pi: threshold {best_t:.2f}, validation half F1 {best_f:.3f} ({time.time() - t0:.0f} s)",
                  flush=True)
            result = {"model": name + "+pi", "threshold": best_t, "sets": {}}
            for set_name, (schemas, split, guesses) in data.items():
                meta, s1, s2 = scores(schemas, split, guesses)
                X, which, _ = rows(schemas, meta, s1, s2)
                p = model.predict_proba(X)[:, 1]
                ids, per = count(schemas, split, meta, which, p >= best_t)
                _, half_per = count(schemas, split, meta, which, p >= 0.5)
                result["sets"][set_name] = {"databases": ids, "per database": per.tolist(),
                                            "per database at 0.5": half_per.tolist()}
                pr = prf(*per.sum(axis=0))
                print(f"{name + '+pi':24} {set_name:30} P {pr[0]:.3f} R {pr[1]:.3f} F1 {pr[2]:.3f}", flush=True)
            (fit.OUT / "scored" / f"{name}+pi.json").write_text(json.dumps(result) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(sys.argv[1:])
