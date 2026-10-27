"""The key model's first stage on the processor and on the graphics card:
LightGBM on 16 threads against XGBoost on CUDA, the same rows, the same
nominal trees — 63 leaves grown leaf by leaf, a step of 0.05, stopped when
the validation split's precision–recall area stops rising — one after the
other. Measured on validation only; the test is not read.

    .venv/Scripts/python -I gpu.py <out>

writes gpu-dev.json into <out>. One run each on a desktop: the times say
which tool to train with, not how fast either is.
"""

import json
import sys
import time
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import lightgbm as lgb  # noqa: E402
import numpy as np  # noqa: E402
import xgboost as xgb  # noqa: E402
from sklearn.metrics import average_precision_score  # noqa: E402

import keys  # noqa: E402
from graph import LABEL, RULES, SID, TABLE, COLUMN, WORKERS, build_all  # noqa: E402
from train import NEGATIVES, SEED, counts, prf  # noqa: E402

STOP = 100
TREES = 3000


def lightgbm(x, y, xv, yv):
    model = lgb.LGBMClassifier(
        n_estimators=TREES, num_leaves=63, learning_rate=0.05, subsample=0.8, subsample_freq=1,
        colsample_bytree=0.5, min_child_samples=20, random_state=SEED, verbose=-1, n_jobs=-1,
    )
    model.fit(x, y, eval_set=[(xv, yv)], eval_metric="average_precision",
              callbacks=[lgb.early_stopping(STOP, verbose=False)])
    return model.predict_proba(xv)[:, 1], model.booster_.num_trees(), len(model.booster_.model_to_string().encode())


def xgboost(x, y, xv, yv):
    model = xgb.XGBClassifier(
        n_estimators=TREES, tree_method="hist", device="cuda", grow_policy="lossguide", max_leaves=63, max_depth=0,
        learning_rate=0.05, subsample=0.8, colsample_bytree=0.5, eval_metric="aucpr",
        early_stopping_rounds=STOP, random_state=SEED,
    )
    model.fit(x, y, eval_set=[(xv, yv)], verbose=False)
    model.set_params(device="cpu")  # predict where the rows are
    scores = model.predict_proba(xv, iteration_range=(0, model.best_iteration + 1))[:, 1]
    raw = model.get_booster().save_raw(raw_format="ubj")
    return scores, model.best_iteration + 1, len(raw)


def main(out: Path) -> None:
    schemas, guesses = keys.load(out)
    t0 = time.time()
    with ProcessPoolExecutor(WORKERS) as pool:
        x, meta = build_all(pool, schemas, "train", guesses)
        xv, meta_v = build_all(pool, schemas, "validation", guesses)
    y = np.array([m[LABEL] for m in meta], dtype=bool)
    sample = np.flatnonzero(y | (np.random.default_rng(SEED).random(len(y)) < NEGATIVES))
    x, y = x[sample], y[sample]
    yv = np.array([m[LABEL] for m in meta_v], dtype=bool)
    rules_v = np.array([m[RULES] for m in meta_v], dtype=bool)
    keys_v = [(m[SID], m[TABLE], m[COLUMN]) for m in meta_v]
    rp = prf(*counts(yv, rules_v))[0]
    print(f"features: {x.shape[0]} training rows, {xv.shape[0]} validation pairs, {time.time() - t0:.0f} s", flush=True)

    result = {"training_rows": int(x.shape[0]), "validation_pairs": int(xv.shape[0])}
    for name, fit in (("lightgbm, 16 threads", lightgbm), ("xgboost, cuda", xgboost)):
        started = time.time()
        scores, trees, size = fit(x, y, xv, yv)
        seconds = time.time() - started
        entry = {"seconds": round(seconds), "trees": int(trees), "kib": round(size / 1024)}
        for how, s in (("pairs", scores), ("one target", keys.one_target(scores, keys_v))):
            t = keys.best_threshold(yv, s)
            p, r, f = keys.at_threshold(yv, s, t)
            entry[how] = {"precision": p, "recall": r, "f1": f,
                          "average_precision": float(average_precision_score(yv, s)),
                          "recall_at_rules_precision": keys.recall_at(yv, s, rp)}
        result[name] = entry
        print(name, json.dumps(entry), flush=True)
    (out / "gpu-dev.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
