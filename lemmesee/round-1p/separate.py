"""Round 1p: whether a model tells two related tables from two unrelated
ones of the same database, the step the pair setting skips.

    .venv/Scripts/python -I separate.py <models> <out> [<variant>]

reads t5-negatives.jsonl, which t5.py's `negatives` wrote, scores the same
pairs with a key model — its best score for any column of the first table
into the second, nothing where it has no candidate — and writes
separate.json: per source, each model's area under the ROC curve, its
average precision, and the share of unrelated pairs it accepts at the score
that keeps 80 % of the related ones.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import numpy as np  # noqa: E402
from sklearn.metrics import average_precision_score, roc_auc_score  # noqa: E402

import external  # noqa: E402


def accepted_at(y: np.ndarray, s: np.ndarray, recall: float) -> float:
    """The share of unrelated pairs scored at least as high as the related
    pair at that recall; 1 when that related pair scores nothing."""
    cut = np.sort(s[y])[::-1][int(np.ceil(recall * y.sum())) - 1]
    return float((s[~y] >= cut).mean()) if cut > s.min() else 1.0


def main(models: Path, out: Path, variant: str | None) -> None:
    build = None
    if variant:
        import widen  # noqa: E402
        build = lambda job: widen.build((*job, variant))  # noqa: E731
    run, graph = external.scorer(models, build)
    schemas = [json.loads(line) for line in open(out / "hidden.jsonl", encoding="utf-8")]
    scores, meta = run(schemas, {})
    best = defaultdict(float)
    for i, m in enumerate(meta):
        k = (m[graph.SID], m[graph.TABLE], m[graph.TARGET])
        best[k] = max(best[k], float(scores[i]))
    rows = [json.loads(line) for line in open(out / "t5-negatives.jsonl", encoding="utf-8")]
    result = {}
    for src in ("spider", "bird", "ctu", "all"):
        for subset in ("all", "novel"):
            r = [x for x in rows if (src == "all" or x["source"] == src) and (subset == "all" or x["novel"])]
            y = np.array([x["related"] for x in r], dtype=bool)
            t5 = np.array([x["best_log_likelihood"] for x in r])
            ours = np.array([best[(f"{x['source']}/{x['db']}", x["table"], x["other"])] for x in r])
            result[f"{src}, {subset}"] = {
                "related": int(y.sum()), "unrelated": int((~y).sum()),
                "auc": {"t5": float(roc_auc_score(y, t5)), "ours": float(roc_auc_score(y, ours))},
                "average precision": {"t5": float(average_precision_score(y, t5)),
                                      "ours": float(average_precision_score(y, ours))},
                "unrelated accepted at 80 % of related": {"t5": accepted_at(y, t5, 0.8), "ours": accepted_at(y, ours, 0.8)},
            }
    name = f"separate-{variant.replace(' ', '-')}.json" if variant else "separate.json"
    (out / name).write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3] if len(sys.argv) > 3 else None)
