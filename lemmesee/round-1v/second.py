"""Round 1v: a second stage fitted on a first stage's kept scores (first.py),
and measured on every set by the number of tables of a schema.

    python -I second.py fit <first> <kind> [<tag>]     the stage on the training split's scores out of fold
    python -I second.py score <first> <kind> [<tag>]   every set dump.py wrote, beside the frozen models

<kind> names what the stage reads (wide.py's KINDS): `graph` is round 1b's
nineteen features, `relative` round 1t's stack.py, `style` its style.py,
`wide` this round's. Every stage is LightGBM as round 1b's, its leaves
bounded as `seven-bounded`'s; the threshold is the validation split's best
F1, one target a column, the split read beside its renamed copies as round
1t's fits read it. Models go to corpus/round-1v/second/<first>--<kind>[-<tag>]/,
scorings to corpus/round-1v/scored/.
"""

import json
import pickle
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import first as first_stage  # noqa: E402  (the earlier rounds on the path, the trigrams sorted)

import lightgbm as lgb  # noqa: E402
import numpy as np  # noqa: E402
from sklearn.metrics import average_precision_score  # noqa: E402

import graph  # noqa: E402
import keys  # noqa: E402
import letters  # noqa: E402
import whole  # noqa: E402
from whole import one_target, unlabeled  # noqa: E402
import wide  # noqa: E402

OUT = first_stage.OUT
BOUNDED = dict(max_delta_step=20.0, min_child_weight=1.0, reg_lambda=1.0)
FROZEN = ("1s", "seven-bounded", "nine-mended")


def predictions(rec: dict, schemas: list, kept: np.ndarray, threshold: float) -> list[dict]:
    out = [dict() for _ in schemas]
    for i in np.flatnonzero(kept >= threshold):
        s = schemas[rec["schema"][i]]
        t = s["tables"][rec["table"][i]]
        out[rec["schema"][i]][(t["name"], t["columns"][rec["column"][i]]["name"])] = s["tables"][rec["target"][i]]["name"]
    return out


def directory(first: str, kind: str, tag: str) -> Path:
    return OUT / "second" / (f"{first}--{kind}" + (f"-{tag}" if tag else ""))


def teach(rec: dict, schemas: list, name: str) -> tuple:
    """A first stage fitted on the declarations, its pairs taught afterwards
    from the teachers' answers (teach.py), by index: a pair both teachers
    name in a schema under its own names becomes a key; every other pair of
    a column a teacher names and no declaration names, and such a column of
    a renamed schema, is left out. Returns the labels, the rows to leave out
    and the rows taught."""
    taught = json.load(open(OUT / "lm" / name, encoding="utf-8"))
    both, unsure = set(), set()
    for si, s in enumerate(schemas):
        t = taught.get(s["id"].replace("~restyled", ""))
        if not t:
            continue
        for a, c, g in t["both"]:
            unsure.add((si, a, c))
            if "restyled" not in s:
                both.add((si, a, c, g))
        for a, c in t["one"]:
            unsure.add((si, a, c))
    n = len(rec["schema"])
    label = rec["label"].copy()
    is_taught = np.zeros(n, dtype=bool)
    is_unsure = np.zeros(n, dtype=bool)
    sc, tb, cl, tg = rec["schema"], rec["table"], rec["column"], rec["target"]
    for i in range(n):
        k = (int(sc[i]), int(tb[i]), int(cl[i]))
        if k in unsure:
            is_unsure[i] = True
            if (*k, int(tg[i])) in both and not label[i]:
                label[i] = is_taught[i] = True
    ck = whole.column_keys(rec)
    uniq, inv = np.unique(ck, return_inverse=True)
    keyed = np.zeros(len(uniq), dtype=bool)
    np.logical_or.at(keyed, inv, label)
    return label, is_unsure & ~keyed[inv], is_taught


TAGS = {
    "": {},
    # the second stage alone taught from the teachers, on a first stage fitted on the declarations
    "taught": {"teach": "taught-more.json"},
    # the sample without the columns most likely keys nobody declared (unlabeled, on the first stage's scores out of fold)
    "sure": {"sure": 0.5, "complete": 0.8},
    "sure3": {"sure": 0.3, "complete": 0.8},
    "sure-all": {"sure": 0.5, "complete": 1.01},
}


def fit(first: str, kind: str, tag: str = "", params: dict | None = None) -> None:
    d = directory(first, kind, tag)
    opts = TAGS[tag]
    seed = first_stage.FITS.get(first, {}).get("seed")
    if seed is not None:  # a first stage fitted with another seed takes it through its second stage too
        params = {**(params or {}), "random_state": seed}
    d.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    clock = lambda what: print(f"{d.name}: {what}: {time.time() - t0:.0f} s", flush=True)  # noqa: E731
    train = pickle.load(open(OUT / "first" / first / "train.pkl", "rb"))
    rec = train["pairs"]
    with letters.pool(graph.WORKERS) as pool:
        x = wide.features(pool, kind, train["schemas"], rec, rec["first"], chunks=64)
        clock(f"features, {x.shape}")
        val = pickle.load(open(OUT / "first" / first / "validation.pkl", "rb"))
        xv = wide.features(pool, kind, val["schemas"], val["pairs"], val["pairs"]["first"])
    y, sample = rec["label"], rec["sample"]
    left_out = 0
    if "teach" in opts:
        y, drop, rec["taught"] = teach(rec, train["schemas"], opts["teach"])
        rec["label"] = y
        left_out = int((sample & drop).sum())
        sample = (sample | rec["taught"]) & ~drop
        vrec = val["pairs"]
        vrec["label"], vdrop, vrec["taught"] = teach(vrec, val["schemas"], opts["teach"])
        vrec["unsure"] = vdrop
    if "sure" in opts:
        u = unlabeled(rec, rec["first"], opts["sure"], opts["complete"])
        left_out = int((sample & u).sum())
        sample = sample & ~u
    second = lgb.LGBMClassifier(**{**graph.SECOND, **BOUNDED, **(params or {})})
    second.fit(x[sample], y[sample])
    clock("fitted")
    # the training split's scores out of fold too, for a stage that reads this one
    oof = np.zeros(len(y), dtype=np.float32)
    for k in range(graph.FOLDS):
        m = lgb.LGBMClassifier(**{**graph.SECOND, **BOUNDED, **(params or {})})
        m.fit(x[sample & (rec["fold"] != k)], y[sample & (rec["fold"] != k)])
        held = rec["fold"] == k
        oof[held] = m.booster_.predict(x[held])
    clock("out of fold")
    del x
    sv = second.booster_.predict(xv)
    vrec = val["pairs"]
    kept = one_target(sv, vrec)
    yv = vrec["label"]
    threshold = keys.best_threshold(yv, kept)
    p, r, f = keys.at_threshold(yv, kept, threshold)
    declared = None
    if "unsure" in vrec and vrec["unsure"].any() and not vrec["taught"].any():
        # the teachers' answers used to doubt: the threshold is read without the unsure columns,
        # and against every declaration beside it
        keep = ~vrec["unsure"]
        td = threshold
        declared = dict(zip(("precision", "recall", "f1"), (p, r, f)), threshold=td)
        threshold = keys.best_threshold(yv[keep], kept[keep])
        p, r, f = keys.at_threshold(yv[keep], kept[keep], threshold)
        declared["f1 at the taught threshold"] = keys.at_threshold(yv, kept, threshold)[2]
    elif "taught" in vrec:
        # the same scores against the declarations alone: a taught key drawn is a false one
        yd = yv & ~vrec["taught"]
        td = keys.best_threshold(yd, kept)
        declared = dict(zip(("precision", "recall", "f1"), keys.at_threshold(yd, kept, td)), threshold=td)
        declared["f1 at the taught threshold"] = keys.at_threshold(yd, kept, threshold)[2]
    second.booster_.save_model(str(d / "keys-second.txt"))
    np.save(d / "train-oof.npy", oof)
    np.save(d / "validation.npy", sv.astype(np.float32))
    result = {"first": first, "kind": kind, "tag": tag, "params": params or {}, "features": wide.NAMES[kind],
              "threshold": threshold, "sample": int(sample.sum()), "left out as unlabeled": left_out,
              "validation": {"precision": p, "recall": r, "f1": f,
                             "average_precision": float(average_precision_score(yv, kept))},
              "validation, declarations only": declared,
              "importance": dict(sorted(zip(wide.NAMES[kind], map(int, second.booster_.feature_importance("gain"))),
                                        key=lambda kv: -kv[1])),
              "seconds": round(time.time() - t0)}
    (d / "second.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k not in ("features", "importance")}), flush=True)


def frozen_counts(model: str, name: str):
    sc = whole.Scored(model, name)
    return sc.counts(sc.baseline())


def score(first: str, kind: str, tag: str = "", which: str = "") -> None:
    """`which` = `declared` reads a taught fit at the threshold its validation's declarations alone choose."""
    d = directory(first, kind, tag)
    result = json.load(open(d / "second.json", encoding="utf-8"))
    second = lgb.Booster(model_file=str(d / "keys-second.txt"))
    threshold = result["threshold"]
    label = d.name
    if which == "declared":
        threshold = result["validation, declarations only"]["threshold"]
        label = d.name + "@declared"
    out = {"model": label, "threshold": threshold, "sets": {}}
    with letters.pool(graph.WORKERS) as pool:
        for name in whole.SETS:
            path = OUT / "first" / first / f"{whole.slug(name)}.pkl"
            if not path.exists():
                continue
            data = pickle.load(open(OUT / "sets" / f"{whole.slug(name)}.pkl", "rb"))
            schemas = data["schemas"]
            rec = pickle.load(open(path, "rb"))["pairs"]
            kept_raw = d / "sets" / f"{whole.slug(name)}.npy"
            if kept_raw.exists() and kept_raw.stat().st_mtime > max((d / "keys-second.txt").stat().st_mtime,
                                                                    path.stat().st_mtime):
                raw = np.load(kept_raw).astype(np.float64)  # this stage's scores of the set, kept by an earlier reading
            else:
                x = wide.features(pool, kind, schemas, rec, rec["first"])
                raw = second.predict(x)
                (d / "sets").mkdir(exist_ok=True)
                np.save(kept_raw, raw.astype(np.float32))
            kept = one_target(raw, rec)
            preds = predictions(rec, schemas, kept, threshold)
            truth = [{tuple(k) for k in s["fk"]} for s in schemas]
            per = np.array([[len(t & {(a, c, g) for (a, c), g in p.items()}),
                             len({(a, c, g) for (a, c), g in p.items()} - t),
                             len(t - {(a, c, g) for (a, c), g in p.items()})] for t, p in zip(truth, preds)])
            tables = np.array([len(s["tables"]) for s in schemas])
            entry = {"per database": per.tolist()}
            print(f"== {name}")
            base = None
            for fm in FROZEN:
                if (OUT / "scores" / fm / f"{whole.slug(name)}.pkl").exists():
                    b = frozen_counts(fm, name)
                    entry[fm] = whole.measure(b, tables)
                    print(whole.line(fm, entry[fm]))
                    if fm == "nine-mended":
                        base = b
            entry["measures"] = whole.measure(per, tables, base)
            print(whole.line(label, entry["measures"]), flush=True)
            out["sets"][name] = entry
    (OUT / "scored").mkdir(exist_ok=True)
    (OUT / "scored" / f"{label}.json").write_text(json.dumps(out) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, first, kind, *rest = sys.argv[1:]
    if cmd == "fit":
        fit(first, kind, *rest[:1])
    else:
        score(first, kind, *rest[:2])
