"""Round 1q: how much of the gap to a language model is the threshold.
Round 1b chose it for F1 on SchemaPile-Perm's validation split, whose labels
miss every key a schema did not declare; this reads the widened variant's
scores on validation, on validation's complete schemas, and on round 1p's
databases, and gives F1 over every declared key at each threshold.

    .venv/Scripts/python -I threshold.py <round 1b out> <models> <round 1p out> <variant>
"""

import json
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

ROUND_1P = Path(__file__).resolve().parent.parent / "round-1p"
sys.path.insert(0, str(ROUND_1P))

import numpy as np  # noqa: E402

import widen  # noqa: E402
import graph  # noqa: E402
import keys  # noqa: E402


def curve(meta, kept, truth_count, ts):
    """F1 over every declared key at each threshold."""
    y = np.array([m[graph.LABEL] for m in meta], dtype=bool)
    out = []
    for t in ts:
        d = kept >= t
        tp = int((d & y).sum())
        p = tp / max(int(d.sum()), 1)
        r = tp / truth_count
        out.append((float(t), p, r, 2 * p * r / (p + r) if p + r else 0.0))
    return out


def main(data: Path, models: Path, out: Path, variant: str) -> None:
    first, second = widen.load_models(models)
    schemas, guesses = keys.load(data)
    with ProcessPoolExecutor(graph.WORKERS) as pool:
        xv, mv = widen.build_all(pool, schemas, "validation", guesses, variant)
    sv = second.predict(graph.graph_features(mv, first.predict_proba(xv)[:, 1]))
    kv = keys.one_target(sv, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in mv])
    complete = graph.complete_mask(mv)
    val = [s for s in schemas if s["split"] == "validation" and s["fk"]]
    complete_ids = {m[graph.SID] for m, c in zip(mv, complete) if c}
    n_all = sum(len(s["fk"]) for s in val)
    n_complete = sum(len(s["fk"]) for s in val if s["id"] in complete_ids)

    ext = [json.loads(line) for line in open(out / "hidden.jsonl", encoding="utf-8")]
    x, me = widen.build((ext, "external", {}, variant))
    se = second.predict(graph.graph_features(me, first.predict_proba(x)[:, 1]))
    ke = keys.one_target(se, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in me])
    src = {s["id"]: s["source"] for s in ext}
    ts = np.round(np.arange(0.05, 0.96, 0.01), 2)
    result = {"validation": curve(mv, kv, n_all, ts),
              "validation, complete schemas": curve([m for m, c in zip(mv, complete) if c], kv[complete], n_complete, ts)}
    for name in ("spider", "bird", "ctu", "all"):
        mask = np.array([name in ("all", src[m[graph.SID]]) for m in me])
        n = sum(len(s["fk"]) for s in ext if name in ("all", s["source"]))
        result[f"external, {name}"] = curve([m for m, k in zip(me, mask) if k], ke[mask], n, ts)
    (out.parent / "round-1q").mkdir(exist_ok=True)
    (out.parent / "round-1q" / "threshold.json").write_text(json.dumps(result) + "\n", encoding="utf-8")
    for k, rows in result.items():
        best = max(rows, key=lambda r: r[3])
        at = min(rows, key=lambda r: abs(r[0] - 0.81))
        print(f"{k:32} best t {best[0]:.2f} P {best[1]:.3f} R {best[2]:.3f} F1 {best[3]:.3f} | at 0.81: P {at[1]:.3f} R {at[2]:.3f} F1 {at[3]:.3f}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4])
