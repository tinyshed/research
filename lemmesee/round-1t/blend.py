"""Round 1t: two or more fits of one variant read together, their second
stages' scores averaged, the threshold the validation split's best F1 one
target a column, as a fit's own.

    python -I blend.py <model> <model> [<model> ...]

The validation split is the fits' own, SchemaPile's validation schemas each
beside its renamed copy, drawn with the dictionary's shortenings as the
version 7 fits drew them. Writes corpus/round-1t/scored/<a>+<b>....json as
fit.py's `score` writes a fit's, so `summarize` reads it beside the others.
"""

import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import cues  # noqa: E402  (first, as in fit.py)
import letters  # noqa: E402

import numpy as np  # noqa: E402

import fit  # noqa: E402
import graph  # noqa: E402
import keys  # noqa: E402
import restyle  # noqa: E402
import stack  # noqa: E402
import widen  # noqa: E402

from train import prf  # noqa: E402

graph.WORKERS = 4


def blended(models, x, meta):
    out = np.zeros(len(meta))
    for first, second, final in models:
        out += second.predict(stack.of(final)(meta, first.predict_proba(x)[:, 1]))
    return out / len(models)


def main(names):
    variants = {fit.MODELS[n][1] for n in names}
    assert len(variants) == 1, "one variant: the pairs are built once"
    variant = variants.pop()
    models = []
    for n in names:
        d = fit.MODELS[n][0]
        first, second = widen.load_models(d)
        models.append((first, second, json.load(open(d / "graph-final.json", encoding="utf-8"))))
    t0 = time.time()
    with letters.pool(graph.WORKERS) as pool:
        restyle.DICTIONARY = True
        schemas, guesses, _ = fit.training_data(True, False, {"dictionary shortening": True})
        restyle.DICTIONARY = False
        xv, meta_v = cues.build_all(pool, [s for s in schemas if s["split"] == "validation"], "validation", guesses,
                                    variant)
        sv = blended(models, xv, meta_v)
        yv = np.array([m[graph.LABEL] for m in meta_v], dtype=bool)
        kept = keys.one_target(sv, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta_v])
        threshold = keys.best_threshold(yv, kept)
        p, r, f = keys.at_threshold(yv, kept, threshold)
        print(f"validation: threshold {threshold:.3f} P {p:.3f} R {r:.3f} F1 {f:.3f} ({time.time() - t0:.0f} s)",
              flush=True)
        del xv, meta_v, schemas
        name = "+".join(names)
        result = {"model": name, "threshold": threshold, "blend of": names,
                  "validation": {"precision": p, "recall": r, "f1": f}, "sets": {}}
        for set_name, (sch, split, g) in fit.sets().items():
            x, meta = cues.build_all(pool, sch, split, g, variant)
            kept = keys.one_target(blended(models, x, meta),
                                   [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
            ids, per = fit.per_database(sch, split, meta, kept, threshold)
            _, at_half = fit.per_database(sch, split, meta, kept, 0.5)
            result["sets"][set_name] = {"databases": ids, "per database": per.tolist(),
                                        "per database at 0.5": at_half.tolist()}
            p, r, f = prf(*per.sum(axis=0))
            print(f"{name:30} {set_name:30} P {p:.3f} R {r:.3f} F1 {f:.3f}  ({time.time() - t0:.0f} s)", flush=True)
    (fit.OUT / "scored" / f"{name}.json").write_text(json.dumps(result) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    assert letters.is_sorted()
    main(sys.argv[1:])
