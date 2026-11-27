"""Round 1y: the label frequency, the weights of the unlabelled pairs and
the weighted measures (README.md has the formulas and their sources).

    python -I pu.py weights     the weights of both seeds, kept, and their table by size
"""

import json
import pickle
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1v"))

import numpy as np  # noqa: E402

import whole  # noqa: E402  (round 1v's measures)

V = whole.OUT                       # corpus/round-1v, read only
OUT = whole.ROOT / "round-1y"
SEEDS = {"": 20261027, "-seed2": 20261028}
FLOOR = 0.01                        # an unlabelled pair enters as a positive from this weight on

# the label frequency by the number of tables: round 1v's thinning.json and its teachers' table
EDGES = np.array([5, 10, 20, 40])
E_RULES = np.array([0.953, 0.867, 0.793, 0.736, 0.659])
E_TEACHERS = np.array([0.953, 0.867, 0.69, 0.62, 0.71])


def e_of(tables, which: str = "rules") -> np.ndarray:
    return (E_RULES if which == "rules" else E_TEACHERS)[np.searchsorted(EDGES, np.asarray(tables), side="right")]


def size_bin(tables) -> np.ndarray:
    return np.searchsorted(EDGES, np.asarray(tables), side="right")


BIN_NAMES = ["under 5", "5 to 9", "10 to 19", "20 to 39", "40 or more"]


def frozen_first(twin: str) -> str:
    return "eleven" + twin


def calibration(twin: str):
    """g = Pr(s = 1 | score) over every training pair: the isotonic regression
    of the declared label on the frozen model's score out of fold. Kept as its knots."""
    path = OUT / f"calibration{twin}.npz"
    if path.exists():
        z = np.load(path)
        return z["x"], z["y"]
    from sklearn.isotonic import IsotonicRegression
    rec = pickle.load(open(V / "first" / frozen_first(twin) / "train.pkl", "rb"))["pairs"]
    s = np.load(V / "second" / f"{frozen_first(twin)}--wide" / "train-oof.npy").astype(np.float64)
    iso = IsotonicRegression(y_min=0.0, y_max=1.0, out_of_bounds="clip").fit(s, rec["label"].astype(np.float64))
    OUT.mkdir(parents=True, exist_ok=True)
    np.savez(path, x=iso.X_thresholds_, y=iso.y_thresholds_)
    return iso.X_thresholds_, iso.y_thresholds_


def g_of(scores, knots) -> np.ndarray:
    return np.interp(np.asarray(scores, dtype=np.float64), knots[0], knots[1])


def score_at(g_value: float, knots) -> float:
    """The lowest score whose g reaches g_value."""
    x, y = knots
    i = int(np.searchsorted(y, g_value, side="left"))
    if i >= len(y):
        return float("inf")
    if i == 0 or y[i] == y[i - 1]:
        return float(x[i])
    return float(x[i - 1] + (g_value - y[i - 1]) / (y[i] - y[i - 1]) * (x[i] - x[i - 1]))


SHRINK = 5.0   # a schema's own share of declared `table_id` columns counts beside this many at its size's frequency


def schema_e() -> dict:
    """A label frequency for each training schema (README.md, "Added": the SAR fit read beside the rule):
    its own share of rule-named columns declared, shrunk towards its size's e(n). Read as thin.py reads it."""
    from collections import defaultdict
    root = whole.ROOT / "round-1b"
    named = defaultdict(dict)
    for line in open(root / "guesses.jsonl", encoding="utf-8"):
        g = json.loads(line)
        if g.get("fk"):
            named[g["schema"]][(g["table"], g["column"])] = g["fk"]
    out = {}
    with open(root / "hidden.jsonl", encoding="utf-8") as f:
        for line in f:
            s = json.loads(line)
            if s["split"] != "train" or not s["fk"]:
                continue
            declared = {(a, c): g for a, c, g in s["fk"]}
            n = named.get(s["id"], {})
            so = sum(1 for k, g in n.items() if declared.get(k) == g)
            e = float(e_of([len(s["tables"])])[0])
            out[s["id"]] = (so + SHRINK * e) / (len(n) + SHRINK)
    return out


def row_e(train: dict, which: str) -> np.ndarray:
    rec = train["pairs"]
    if which == "schema":
        by = schema_e()
        return np.array([by[s["id"]] for s in train["schemas"]])[rec["schema"]]
    tables = np.array([len(s["tables"]) for s in train["schemas"]])[rec["schema"]]
    return e_of(tables, which)


def weights(twin: str, which: str = "rules") -> np.ndarray:
    """A training pair's weight as a positive, 0 for a pair of a declared column.
    `which`: `rules` and `teachers` by the number of tables, `schema` a schema's own."""
    path = OUT / f"weights-{which}{twin}.npy"
    if path.exists():
        return np.load(path)
    train = pickle.load(open(V / "first" / frozen_first(twin) / "train.pkl", "rb"))
    rec = train["pairs"]
    s = np.load(V / "second" / f"{frozen_first(twin)}--wide" / "train-oof.npy").astype(np.float64)
    g = np.minimum(g_of(s, calibration(twin)), 1 - 1e-9)
    e = row_e(train, which)
    w = np.minimum(1.0, (1 - e) / e * g / (1 - g))
    ck = whole.column_keys(rec)
    uniq, inv = np.unique(ck, return_inverse=True)
    declared = np.zeros(len(uniq), dtype=bool)
    np.logical_or.at(declared, inv, rec["label"])
    w[declared[inv]] = 0.0
    total = np.bincount(inv, weights=w, minlength=len(uniq))
    w /= np.maximum(total, 1.0)[inv]
    w = w.astype(np.float32)
    np.save(path, w)
    return w


def weights_table() -> dict:
    out = {}
    for twin in SEEDS:
        train = pickle.load(open(V / "first" / frozen_first(twin) / "train.pkl", "rb"))
        rec = train["pairs"]
        tables = np.array([len(s["tables"]) for s in train["schemas"]])[rec["schema"]]
        b = size_bin(tables)
        s = np.load(V / "second" / f"{frozen_first(twin)}--wide" / "train-oof.npy").astype(np.float64)
        knots = calibration(twin)
        rows = {}
        ws = weights(twin, "schema")
        es = row_e(train, "schema")
        for i, name in enumerate(BIN_NAMES):
            m = b == i
            lab = m & rec["label"]
            rows[name] = {"training pairs": int(m.sum()), "declared keys": int(lab.sum()),
                          "schema: undeclared keys a schema's own frequency implies, the sum of (1-e)/e over declared keys":
                              round(float(((1 - es[lab]) / es[lab]).sum())),
                          "schema: the weights' sum": round(float(ws[m].sum()), 1),
                          f"schema: pairs entering as positives (w >= {FLOOR})": int((ws[m] >= FLOOR).sum())}
        for which in ("rules", "teachers"):
            w = weights(twin, which)
            for i, name in enumerate(BIN_NAMES):
                m = b == i
                declared = int(rec["label"][m].sum())
                e = float(e_of([[0, 5, 10, 20, 40][i]], which)[0])
                r = rows.setdefault(name, {"training pairs": int(m.sum()), "declared keys": declared})
                r[f"{which}: e"] = e
                r[f"{which}: undeclared keys the label frequency implies, (1-e)/e x declared"] = round((1 - e) / e * declared)
                r[f"{which}: the weights' sum"] = round(float(w[m].sum()), 1)
                r[f"{which}: pairs entering as positives (w >= {FLOOR})"] = int((w[m] >= FLOOR).sum())
                r[f"{which}: pairs with w >= 0.5"] = int((w[m] >= 0.5).sum())
        # how well g is calibrated: declared share by score band, over all training pairs
        bands = [0, 0.05, 0.2, 0.5, 0.7, 0.8, 0.9, 0.95, 1.0001]
        cal = []
        for lo, hi in zip(bands, bands[1:]):
            m = (s >= lo) & (s < hi)
            cal.append({"score": [lo, min(hi, 1.0)], "pairs": int(m.sum()),
                        "declared share": float(rec["label"][m].mean()) if m.any() else None,
                        "mean g": float(g_of(s[m], knots).mean()) if m.any() else None})
        out[frozen_first(twin) + "--wide"] = {"by size": rows, "g by score band": cal,
                                             "sampled pairs": int(rec["sample"].sum())}
        print(frozen_first(twin) + "--wide")
        for name, r in rows.items():
            print(f"  {name:>10}: declared {r['declared keys']:6d} | rules e {r['rules: e']:.3f} implies "
                  f"{r['rules: undeclared keys the label frequency implies, (1-e)/e x declared']:6d}, weights sum "
                  f"{r['rules: the weights\' sum']:8.1f}, positives entering {r[f'rules: pairs entering as positives (w >= {FLOOR})']:6d}"
                  f" | teachers e {r['teachers: e']:.3f} implies "
                  f"{r['teachers: undeclared keys the label frequency implies, (1-e)/e x declared']:6d}, sum "
                  f"{r['teachers: the weights\' sum']:8.1f}")
        for c in cal:
            pass
        k1 = "schema: undeclared keys a schema's own frequency implies, the sum of (1-e)/e over declared keys"
        k2 = "schema: the weights' sum"
        for name, r in rows.items():
            print(f"  {name:>10}: a schema's own e implies {r[k1]:6d}, weights sum {r[k2]:8.1f}")
        for c in cal:
            print(f"  score {c['score']}: pairs {c['pairs']:8d} declared {c['declared share']:.4f} g {c['mean g']:.4f}")
    (OUT / "weights.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    return out


# ---- the weighted measures

def ipw_counts(per: np.ndarray, tables: np.ndarray, which: str = "rules") -> np.ndarray:
    """(databases, 3) of declared found, false and missed -> weighted found, false, missed."""
    per = np.asarray(per, dtype=np.float64)
    e = e_of(tables, which)
    drawn = per[:, 0] + per[:, 1]
    tp = np.minimum(per[:, 0] / e, drawn)
    return np.stack([tp, drawn - tp, per[:, 2] / e], axis=1)


def ipw(per: np.ndarray, tables: np.ndarray, base: np.ndarray | None = None) -> dict:
    """Weighted P, R, F1 with intervals; minus a base on the same resamples."""
    w = ipw_counts(per, tables)
    p, r, f = whole.prf(*w.sum(axis=0))
    idx = np.random.default_rng(whole.SEED).integers(0, len(per), (1000, len(per)))
    boot = whole.resampled(w, idx)
    out = {"precision": p, "recall": r, "f1": f, "f1_95": whole.interval(boot[:, 2])}
    big = tables >= 40
    if big.any():
        out["40 tables or more"] = dict(zip(("precision", "recall", "f1"), whole.prf(*w[big].sum(axis=0))))
    if base is not None:
        wb = ipw_counts(base, tables)
        bb = whole.resampled(wb, idx)
        out["minus base, f1"] = f - whole.prf(*wb.sum(axis=0))[2]
        out["minus base, f1_95"] = whole.interval(boot[:, 2] - bb[:, 2])
    return out


def best_ipw_threshold(kept: np.ndarray, label: np.ndarray, tables: np.ndarray, candidates=None) -> float:
    """The threshold on `kept` (one target a column, the rest 0) with the best weighted F1.
    Counted over pairs with a database's e: found/e capped a database is the measure; here the
    cap is taken over the whole split by size, which the validation split's counts never reach."""
    live = kept > 0
    s, y, e = kept[live], label[live], e_of(tables[live])
    order = np.argsort(-s, kind="stable")
    s, y, e = s[order], y[order], e[order]
    tp = np.cumsum(y / e)
    drawn = np.arange(1, len(s) + 1)
    total = float((label / e_of(tables)).sum())   # every declared key, found or not, weighted
    tp = np.minimum(tp, drawn)
    f = 2 * tp / (drawn + total)
    last = np.r_[s[1:] != s[:-1], True]          # a threshold falls between distinct scores
    i = int(np.flatnonzero(last)[np.argmax(f[last])])
    return float(s[i])


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "weights":
        weights_table()
