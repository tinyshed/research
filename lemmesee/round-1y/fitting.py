"""Round 1y: `eleven--wide`'s recipe fitted with the unlabelled pairs weighted
(README.md, "(a)").

    python -I fitting.py pairs                 the training and validation splits' pairs, once
    python -I fitting.py first <name>              eleven-pu | eleven-sar [-seed2]: five folds and the whole, on the card
    python -I fitting.py score-first <name>        its scores on round 1v's kept pairs of every development set
    python -I fitting.py second <first> <kind>     <kind> = wide | wide-pu | wide-sar; <first> = eleven[-seed2] | eleven-pu[-seed2]
    python -I fitting.py score-second <first> <kind>

Round 1v's first.py and second.py with three differences: the weights
(pu.py), no second stage out of fold (nothing here reads one), and two
thresholds read on the validation split, the recipe's best F1 over declared
keys and the best weighted F1. A frozen first stage (`eleven`,
`eleven-seed2`) is read from corpus/round-1v/; everything written goes to
corpus/round-1y/.
"""

import json
import os
import pickle
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "round-1v"))

import first as v_first  # noqa: E402  (round 1v's; the earlier rounds on the path, the trigrams sorted)

import lightgbm as lgb  # noqa: E402
import numpy as np  # noqa: E402
import scipy.sparse as sp  # noqa: E402
from sklearn.metrics import average_precision_score  # noqa: E402

import graph  # noqa: E402
import keys  # noqa: E402
import letters  # noqa: E402
import ten  # noqa: E402
import whole  # noqa: E402
import wide  # noqa: E402
from train import NEGATIVES  # noqa: E402

import locks  # noqa: E402
import pu  # noqa: E402

V, OUT = pu.V, pu.OUT
BOUNDED = dict(max_delta_step=20.0, min_child_weight=1.0, reg_lambda=1.0)
FIRSTS = {"eleven-pu": "", "eleven-pu-seed2": "-seed2",          # a weighted first stage and its seed's twin
          "eleven-sar": "", "eleven-sar-seed2": "-seed2"}        # weighted by a schema's own label frequency


def weights_of(name: str, twin: str) -> np.ndarray:
    """`...-sar` names take a schema's own label frequency, the others the size's."""
    return pu.weights(twin, "schema" if "sar" in name else "rules")
FROZEN_FIRSTS = {"eleven": "", "eleven-seed2": "-seed2"}
ALIGNED = ("schema", "table", "column", "target", "label")
POOL = int(os.environ.get("LEMMESEE_POOL", "6"))   # processes; graph.WORKERS orders the rows and stays the recipe's


def twin_of(first: str) -> str:
    return {**FIRSTS, **FROZEN_FIRSTS}[first]


def first_dir(first: str) -> Path:
    return (V if first in FROZEN_FIRSTS else OUT) / "first" / first


def weighted(sample: np.ndarray, y: np.ndarray, w: np.ndarray, rows_ok: np.ndarray | None = None):
    """The recipe's sample with the weights: (rows, labels, weights). A sampled
    unlabelled pair is a negative with weight 1 - w; every unlabelled pair with
    w >= FLOOR is a positive once more with weight w."""
    ok = np.ones(len(y), dtype=bool) if rows_ok is None else rows_ok
    base = np.flatnonzero(sample & ok)
    extra = np.flatnonzero((w >= pu.FLOOR) & ~y & ok)
    rows = np.concatenate([base, extra])
    labels = np.concatenate([y[base], np.ones(len(extra), dtype=bool)])
    weights = np.concatenate([np.where(y[base], 1.0, 1.0 - w[base]), w[extra]]).astype(np.float32)
    return rows, labels, weights


def build_pairs() -> None:
    """The training and validation splits' version 11 pairs, built once for every first stage here:
    the recipe keeps no cache of them, and they are the same rows for either seed and any weights.
    Checked row by row against what the frozen fit kept."""
    assert letters.is_sorted()
    d = OUT / "pairs"
    d.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    schemas, guesses, counts = v_first.training_data({"people": True})
    train = [s for s in schemas if s["split"] == "train" and s["fk"]]
    val = [s for s in schemas if s["split"] == "validation" and s["fk"]]
    with locks.held("pairs", "the training and validation splits' pairs"):
        # graph.WORKERS orders the rows and stays the recipe's eight; fewer processes build them, for memory
        with letters.pool(POOL) as pool:
            for split, part in (("validation", val), ("train", train)):
                x, meta = ten.build_all(pool, part, split, guesses, True)
                rec = v_first.compact(meta, part)
                del meta
                frozen = pickle.load(open(first_dir("eleven") / f"{split}.pkl", "rb"))["pairs"]
                for k in ALIGNED:
                    assert (rec[k] == frozen[k]).all(), f"the pairs are not the frozen fit's: {split} {k}"
                del rec, frozen
                sp.save_npz(d / f"{split}.npz", x, compressed=False)
                print(f"{split}: {x.shape[0]} pairs, {x.nnz} features set: {time.time() - t0:.0f} s", flush=True)
                del x
    (d / "pairs.json").write_text(json.dumps({"data": counts, "seconds": round(time.time() - t0)}) + "\n", encoding="utf-8")


def fit_first(name: str) -> None:
    assert letters.is_sorted()
    twin = FIRSTS[name]
    seed = pu.SEEDS[twin]
    d = OUT / "first" / name
    d.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    clock = lambda what: print(f"{name}: {what}: {time.time() - t0:.0f} s", flush=True)  # noqa: E731
    w = weights_of(name, twin)
    counts = json.load(open(OUT / "pairs" / "pairs.json", encoding="utf-8"))["data"]
    frozen = pickle.load(open(first_dir(pu.frozen_first(twin)) / "train.pkl", "rb"))
    train, rec = frozen["schemas"], frozen["pairs"]
    y = rec["label"]
    folds = rec["fold"].astype(np.int64)
    graph.FIRST_GPU["random_state"] = seed
    sample = y | (np.random.default_rng(seed).random(len(y)) < NEGATIVES)
    assert (sample == rec["sample"]).all(), "the sample is not the frozen fit's"
    oof = np.zeros(len(y), dtype=np.float32)
    x = xv = None      # read once the card is this fit's: nothing is held in memory while it waits

    def one(k):
        rows, labels, weights = weighted(sample, y, w, (folds != k) if k >= 0 else None)
        model = graph.first_model(True)
        model.fit(x[rows], labels, sample_weight=weights)
        if k < 0:
            return k, model, graph.first_scores(model, xv, True).astype(np.float32)
        held = np.flatnonzero(folds == k)
        return k, None, graph.first_scores(model, x[held], True)

    from concurrent.futures import ThreadPoolExecutor
    model_whole = first_v = None
    with locks.held("gpu", f"{name}: the first stage's six fits"):
        x = sp.load_npz(OUT / "pairs" / "train.npz")
        xv = sp.load_npz(OUT / "pairs" / "validation.npz")
        assert x.shape[0] == len(y)
        clock(f"pairs loaded, {x.shape[0]} training and {xv.shape[0]} validation")
        with ThreadPoolExecutor(v_first.SIDE_BY_SIDE) as fits:
            for k, model, scores in fits.map(one, [*range(graph.FOLDS), -1]):
                if k < 0:
                    model_whole, first_v = model, scores
                else:
                    oof[folds == k] = scores
                clock(f"fold {k}" if k >= 0 else "the whole")
    del x, xv
    model_whole.save_model(str(d / "keys-first.ubj"))
    rec = {k: v for k, v in rec.items() if k != "taught"}
    rec.update(first=oof, weight=w)
    pickle.dump({"schemas": train, "pairs": rec}, open(d / "train.pkl", "wb"), protocol=4)
    val = pickle.load(open(first_dir(pu.frozen_first(twin)) / "validation.pkl", "rb"))
    val["pairs"]["first"] = first_v
    pickle.dump(val, open(d / "validation.pkl", "wb"), protocol=4)
    rows, labels, weights = weighted(sample, y, w)
    result = {"fit": name, "seed": seed, "grams": "sorted", "data": counts, "training pairs": len(y),
              "validation pairs": len(first_v), "sample": int(sample.sum()),
              "pairs entering once more as positives": int(len(rows) - sample.sum()),
              "their weights' sum": float(weights[int(sample.sum()):].sum()),
              "sampled negatives' weight lost": float((1 - weights[:int(sample.sum())]).sum()),
              "weights": "a schema's own label frequency" if "sar" in name else "the name rules' label frequency by size",
              "seconds": round(time.time() - t0)}
    (d / "first.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result), flush=True)


def set_pairs(name: str) -> tuple:
    """(schemas, round 1v's kept rows) of a development set."""
    data = pickle.load(open(V / "sets" / f"{whole.slug(name)}.pkl", "rb"))
    rec = pickle.load(open(V / "pairs-people" / f"{whole.slug(name)}.pkl", "rb"))
    return data["schemas"], rec


def first_scores_of(first: str, name: str) -> np.ndarray:
    if first in FROZEN_FIRSTS:
        return pickle.load(open(V / "first" / first / f"{whole.slug(name)}.pkl", "rb"))["pairs"]["first"]
    return np.load(OUT / "first" / first / "sets" / f"{whole.slug(name)}.npy")


def score_first(name: str) -> None:
    import xgboost as xgb  # noqa: E402
    d = OUT / "first" / name
    (d / "sets").mkdir(exist_ok=True)
    model = xgb.XGBClassifier()
    model.load_model(str(d / "keys-first.ubj"))
    model.set_params(device="cpu")
    for n in whole.SETS:
        t0 = time.time()
        kept = d / "sets" / f"{whole.slug(n)}.npy"
        if kept.exists() and kept.stat().st_mtime > (d / "keys-first.ubj").stat().st_mtime:
            continue          # scored by an earlier run of this command, which memory or a stop cut short
        x = sp.load_npz(V / "pairs-people" / f"{whole.slug(n)}.npz")
        scores = model.predict_proba(x)[:, 1].astype(np.float32)
        np.save(d / "sets" / "part.npy", scores)
        (d / "sets" / "part.npy").replace(kept)
        print(f"{name:18} {n:34} pairs {x.shape[0]:8d}  ({time.time() - t0:.0f} s)", flush=True)
        del x


def second_dir(first: str, kind: str) -> Path:
    if first in FROZEN_FIRSTS and kind == "wide":
        return V / "second" / f"{first}--wide"      # the frozen model's own second stage
    return OUT / "second" / f"{first}--{kind}"


def fit_second(first: str, kind: str) -> None:
    assert kind in ("wide", "wide-pu", "wide-sar")
    d = second_dir(first, kind)
    assert d.parent.parent == OUT, "the frozen model is not fitted again"
    twin = twin_of(first)
    seed = pu.SEEDS[twin]
    params = {"random_state": seed} if twin else {}      # as round 1v: a second seed goes through both stages
    d.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    clock = lambda what: print(f"{d.name}: {what}: {time.time() - t0:.0f} s", flush=True)  # noqa: E731
    with locks.held("pairs", f"{d.name}: the second stage's features of the training split"):
        train = pickle.load(open(first_dir(first) / "train.pkl", "rb"))
        rec = train["pairs"]
        val = pickle.load(open(first_dir(first) / "validation.pkl", "rb"))
        with letters.pool(POOL) as pool:
            x = wide.features(pool, "wide", train["schemas"], rec, rec["first"], chunks=64)
            clock(f"features, {x.shape}")
            xv = wide.features(pool, "wide", val["schemas"], val["pairs"], val["pairs"]["first"])
        y, sample = rec["label"], rec["sample"]
        second = lgb.LGBMClassifier(**{**graph.SECOND, **BOUNDED, **params})
        if kind != "wide":
            rows, labels, weights = weighted(sample, y, weights_of(kind, twin))
            second.fit(x[rows], labels, sample_weight=weights)
        else:
            rows = np.flatnonzero(sample)
            second.fit(x[rows], y[rows])
        clock("fitted")
        del x, train
    sv = second.booster_.predict(xv)
    vrec = val["pairs"]
    kept = whole.one_target(sv, vrec)
    yv = vrec["label"]
    tables = np.array([len(s["tables"]) for s in val["schemas"]])[vrec["schema"]]
    t_declared = keys.best_threshold(yv, kept)
    t_ipw = pu.best_ipw_threshold(kept, yv, tables)
    second.booster_.save_model(str(d / "keys-second.txt"))
    np.save(d / "validation.npy", sv.astype(np.float32))
    result = {"first": first, "kind": kind, "seed": seed, "params": params, "features": wide.NAMES["wide"],
              "sample rows": int(len(rows)), "threshold": t_declared, "threshold, weighted F1": t_ipw,
              "validation at the declared threshold": dict(zip(("precision", "recall", "f1"),
                                                               keys.at_threshold(yv, kept, t_declared))),
              "validation at the weighted threshold": dict(zip(("precision", "recall", "f1"),
                                                               keys.at_threshold(yv, kept, t_ipw))),
              "validation average precision": float(average_precision_score(yv, kept)),
              "importance": dict(sorted(zip(wide.NAMES["wide"], map(int, second.booster_.feature_importance("gain"))),
                                        key=lambda kv: -kv[1])),
              "seconds": round(time.time() - t0)}
    (d / "second.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k not in ("features", "importance")}), flush=True)


def score_second(first: str, kind: str) -> None:
    d = second_dir(first, kind)
    assert d.parent.parent == OUT
    second = lgb.Booster(model_file=str(d / "keys-second.txt"))
    (d / "sets").mkdir(exist_ok=True)
    with letters.pool(POOL) as pool:
        for name in whole.SETS:
            t0 = time.time()
            kept = d / "sets" / f"{whole.slug(name)}.npy"
            if kept.exists() and kept.stat().st_mtime > (d / "keys-second.txt").stat().st_mtime:
                continue
            schemas, rec = set_pairs(name)
            x = wide.features(pool, "wide", schemas, rec, first_scores_of(first, name))
            np.save(d / "sets" / "part.npy", second.predict(x).astype(np.float32))
            (d / "sets" / "part.npy").replace(kept)
            print(f"{d.name:28} {name:34} ({time.time() - t0:.0f} s)", flush=True)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "pairs":
        build_pairs()
    elif cmd == "first":
        fit_first(sys.argv[2])
    elif cmd == "score-first":
        score_first(sys.argv[2])
    elif cmd == "second":
        fit_second(sys.argv[2], sys.argv[3])
    else:
        score_second(sys.argv[2], sys.argv[3])
