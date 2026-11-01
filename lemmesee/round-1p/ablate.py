"""Round 1p: what the graph stage adds on round 1p's databases — the chosen
variant's first stage alone, its threshold the best F1 on SchemaPile-Perm's
validation split as the full model's was, against both stages.

    .venv/Scripts/python -I ablate.py <round 1b out> <models> <round 1p out> <variant>

writes ablate-<variant>.json into <round 1p out>: round 1b's task (F1 over
candidates and over every declared key) and the paper's task, for the first
stage alone and for both stages.
"""

import json
import sys
from collections import defaultdict
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import numpy as np  # noqa: E402

import widen  # noqa: E402
import graph  # noqa: E402
import keys  # noqa: E402
from train import counts, prf  # noqa: E402


def main(data: Path, models: Path, out: Path, variant: str) -> None:
    first, second = widen.load_models(models)
    schemas, guesses = keys.load(data)
    with ProcessPoolExecutor(graph.WORKERS) as pool:
        xv, meta_v = widen.build_all(pool, schemas, "validation", guesses, variant)
    yv = np.array([m[graph.LABEL] for m in meta_v], dtype=bool)
    kv = [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta_v]
    t_first = keys.best_threshold(yv, keys.one_target(first.predict_proba(xv)[:, 1], kv))
    t_both = json.load(open(models / "graph-final.json", encoding="utf-8"))["threshold"]
    del xv

    ext = [json.loads(line) for line in open(out / "hidden.jsonl", encoding="utf-8")]
    by_id = {s["id"]: s for s in ext}
    x, meta = widen.build((ext, "external", {}, variant))
    s1 = first.predict_proba(x)[:, 1]
    s2 = second.predict(graph.graph_features(meta, s1))
    k = [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta]
    y = np.array([m[graph.LABEL] for m in meta], dtype=bool)
    src = np.array([by_id[m[graph.SID]]["source"] for m in meta])
    bench = [json.loads(line) for line in open(out / "bench.jsonl", encoding="utf-8")]
    rows = defaultdict(list)
    for i, m in enumerate(meta):
        rows[(m[graph.SID], m[graph.TABLE], m[graph.TARGET])].append(i)
    result = {"thresholds": {"first stage alone": t_first, "both stages": t_both}}
    for how, raw, t in (("first stage alone", s1, t_first), ("both stages", s2, t_both)):
        drawn = keys.one_target(raw, k) >= t
        entry = {}
        for name in ("spider", "bird", "ctu", "all"):
            mask = (src == name) if name != "all" else np.ones_like(y)
            declared = sum(len(s["fk"]) for s in ext if name in ("all", s["source"]))
            tp = int((drawn & y & mask).sum())
            p, r, f = prf(*counts(y[mask], drawn[mask]))
            ra = tp / declared
            hits = total = 0
            for b in bench:
                if b["occurs_in_perm"] or b["too_large"] or name not in ("all", b["source"]):
                    continue
                total += 1
                rr = rows.get((f"{b['source']}/{b['db']}", b["table"], b["foreign_table"]), [])
                if rr:
                    best = max(rr, key=lambda i: (raw[i], -i))
                    key = meta[best][widen.KEY]
                    hits += (meta[best][graph.COLUMN].lower() == b["column"].lower()
                             and key.lower() == b["referred_column"].lower())
            entry[name] = {"candidates": {"precision": p, "recall": r, "f1": f},
                           "every declared key": {"recall": ra, "f1": 2 * p * ra / (p + ra) if p + ra else 0.0},
                           "the paper's task": hits / total}
        result[how] = entry
    (out / f"ablate-{variant.replace(' ', '-')}.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4])
