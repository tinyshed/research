"""Round 1s: the language models on the held-out set, asked after the frozen
model's one reading, at the owner's request, beside it.

    python -I heldout_lm.py <out>

reads <out>/heldout/ as heldout.py left it and <out>/llm/<model>/heldout-db
and heldout-pair as ask.py wrote them, and writes <out>/heldout/lm.json:
over every declared key P / R / F1 with intervals, the frozen model minus
each, the macro F1; the pair task; tokens, time and price.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import lm  # noqa: E402
from refit import EXTERNAL_SEED, interval, resampled  # noqa: E402


def main(out: Path) -> None:
    d = out / "heldout"
    schemas = [json.loads(line) for line in open(d / "hidden.jsonl", encoding="utf-8")]
    bench = [json.loads(line) for line in open(d / "bench.jsonl", encoding="utf-8")]
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["fk"]}
    ids = sorted(truth)
    frozen = defaultdict(set)
    for line in open(d / "open-predictions-frozen.jsonl", encoding="utf-8"):
        sid, t, c, r, _ = json.loads(line)
        frozen[sid].add((t, c, r))
    idx = np.random.default_rng(EXTERNAL_SEED).integers(0, len(ids), (1000, len(ids)))
    pf = lm.counts(truth, frozen, ids)
    bf = resampled(pf, idx)

    def entry(per, boot):
        p, r, f = lm.prf(*per.sum(axis=0))
        return {"precision": p, "recall": r, "f1": f, "precision_95": interval(boot[:, 0]),
                "recall_95": interval(boot[:, 1]), "f1_95": interval(boot[:, 2]),
                "macro f1 over databases": float(np.mean([lm.prf(*c)[2] for c in per]))}

    result = {"frozen model": entry(pf, bf), "models": {}}
    outcomes = [json.loads(line) for line in open(d / "bench-outcomes-frozen.jsonl", encoding="utf-8")]
    kept = [o for o in outcomes if not o["occurs_in_perm"] and not o["too_large"]]
    result["frozen model, the paper's task"] = sum(o["hit whole database"] for o in kept) / len(kept)
    lm.SOURCES = ("livesqlbench",)
    for m in lm.MODELS:
        base = out / "llm" / m
        if not (base / "heldout-db").exists():
            continue
        preds, info = lm.whole_schema(schemas, base / "heldout-db")
        pm = lm.counts(truth, preds, ids)
        bm = resampled(pm, idx)
        e = entry(pm, bm)
        point = np.array(lm.prf(*pf.sum(axis=0))) - np.array(lm.prf(*pm.sum(axis=0)))
        e["frozen minus this"] = {w: {"point": float(point[j]), "95": interval(bf[:, j] - bm[:, j])}
                                  for j, w in enumerate(("precision", "recall", "f1"))}
        e["replies"] = info
        e["the paper's task"] = lm.pair_task(bench, base / "heldout-pair")
        e["usage"] = {s: lm.usage(m, base / s) for s in ("heldout-db", "heldout-pair") if (base / s).exists()}
        result["models"][m] = e
    (d / "lm.json").write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    f = result["frozen model"]
    print(f"frozen                   P {f['precision']:.3f} R {f['recall']:.3f} F1 {f['f1']:.3f}")
    for m, e in result["models"].items():
        print(f"{m:24} P {e['precision']:.3f} R {e['recall']:.3f} F1 {e['f1']:.3f} {e['replies']} "
              f"pair {e['the paper' + chr(39) + 's task']['livesqlbench']['recall_at_1']:.3f}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
