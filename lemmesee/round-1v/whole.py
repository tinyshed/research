"""Round 1v: a set as a frozen model scored it (dump.py), read schema by
schema, and the round's measures: precision, recall and F1 over every
declared one-column key, pooled, as the mean over databases, and by the
number of tables of a schema.

A prediction is {(table, column): target} a schema, by name; a key is right
when its column and its table are, as round 1t counts it.
"""

import pickle
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent / "corpus"
OUT = ROOT / "round-1v"
sys.path.insert(0, str(HERE.parent / "round-1b"))

import numpy as np  # noqa: E402

SEED = 20261101  # round 1p's resampling of databases
BINS = [(0, 5), (5, 10), (10, 20), (20, 40), (40, 100), (100, 10 ** 6)]
SETS = ["schemapile validation", "schemapile validation, renamed", "schemapile test", "schemapile test, renamed",
        "spider", "bird", "ctu", "livesqlbench (development)", "github 2025", "github 2025, rest", "github style",
        "github fresh"]


def slug(name: str) -> str:
    return name.replace(",", "").replace("'", "").replace("(", "").replace(")", "").replace(" ", "-")


def bin_name(b) -> str:
    return f"{b[0]}-{b[1] - 1}" if b[1] < 10 ** 6 else f"{b[0]}+"


class Scored:
    """One model's scores on one set."""

    def __init__(self, model: str, name: str):
        data = pickle.load(open(OUT / "sets" / f"{slug(name)}.pkl", "rb"))
        rec = pickle.load(open(OUT / "scores" / model / f"{slug(name)}.pkl", "rb"))
        self.model, self.name = model, name
        self.schemas = data["schemas"]
        self.threshold = rec["threshold"]
        for k in ("schema", "table", "column", "target", "key", "label", "rules", "first", "second"):
            setattr(self, k, rec[k])
        # a schema's pairs lie together
        self.spans = {}
        n = len(self.schema)
        start = 0
        while start < n:
            end = start
            si = self.schema[start]
            while end < n and self.schema[end] == si:
                end += 1
            assert si not in self.spans
            self.spans[int(si)] = (start, end)
            start = end
        self.truth = [{tuple(k) for k in s["fk"]} for s in self.schemas]
        self.tables = np.array([len(s["tables"]) for s in self.schemas])

    def names(self, i: int):
        s = self.schemas[self.schema[i]]
        t = s["tables"][self.table[i]]
        return t["name"], t["columns"][self.column[i]]["name"], s["tables"][self.target[i]]["name"]

    def baseline(self, scores=None, threshold=None) -> list[dict]:
        """One target a column, the best scored, drawn at the threshold."""
        scores = self.second if scores is None else scores
        threshold = self.threshold if threshold is None else threshold
        out = [dict() for _ in self.schemas]
        for si, (a, b) in self.spans.items():
            best = {}
            for i in range(a, b):
                k = (int(self.table[i]), int(self.column[i]))
                if k not in best or scores[i] > scores[best[k]]:
                    best[k] = i
            s = self.schemas[si]
            for (ti, ci), i in best.items():
                if scores[i] >= threshold:
                    t = s["tables"][ti]
                    out[si][(t["name"], t["columns"][ci]["name"])] = s["tables"][self.target[i]]["name"]
        return out

    def counts(self, preds: list[dict]) -> np.ndarray:
        """(databases, 3): true, false and missed keys."""
        per = np.zeros((len(self.schemas), 3), dtype=np.int64)
        for si, p in enumerate(preds):
            drawn = {(t, c, g) for (t, c), g in p.items()}
            truth = self.truth[si]
            per[si] = [len(truth & drawn), len(drawn - truth), len(truth - drawn)]
        return per


def column_keys(rec: dict) -> np.ndarray:
    """A number a column: its schema, table and column together."""
    return (rec["schema"].astype(np.int64) << 32) | (rec["table"].astype(np.int64) << 16) | rec["column"].astype(np.int64)


def one_target(scores: np.ndarray, rec: dict) -> np.ndarray:
    """Each column keeps its best-scored target; the others score nothing (keys.one_target on indices)."""
    ck = column_keys(rec)
    order = np.lexsort((-scores, ck))
    firsts = np.ones(len(order), dtype=bool)
    firsts[1:] = ck[order][1:] != ck[order][:-1]
    kept = np.zeros_like(scores)
    idx = order[firsts]
    kept[idx] = scores[idx]
    return kept


def unlabeled(rec: dict, scores: np.ndarray, sure: float = 0.5, complete: float = 0.8) -> np.ndarray:
    """The rows of the columns most likely to be keys nobody declared: a
    column no declaration names whose best score is `sure` or more, in a
    schema that declares under `complete` of its columns scoring so. A
    schema that declares most of what scores as a key is taken at its word."""
    ck = column_keys(rec)
    uniq, inv = np.unique(ck, return_inverse=True)
    best = np.zeros(len(uniq), dtype=np.float32)
    np.maximum.at(best, inv, scores.astype(np.float32))
    declared = np.zeros(len(uniq), dtype=bool)
    np.logical_or.at(declared, inv, rec["label"])
    schema_of = (uniq >> 32).astype(np.int64)
    high = best >= sure
    n = int(schema_of.max()) + 1
    scoring = np.bincount(schema_of[high], minlength=n)
    declared_so = np.bincount(schema_of[high & declared], minlength=n)
    share = np.where(scoring > 0, declared_so / np.maximum(scoring, 1), 1.0)
    return (high & ~declared & (share[schema_of] < complete))[inv]


def prf(tp, fp, fn):
    p = tp / (tp + fp) if tp + fp else 0.0
    r = tp / (tp + fn) if tp + fn else 0.0
    return p, r, (2 * p * r / (p + r) if p + r else 0.0)


def resampled(per: np.ndarray, idx: np.ndarray) -> np.ndarray:
    s = per[idx].sum(axis=1)
    tp, fp, fn = s[:, 0], s[:, 1], s[:, 2]
    p = np.where(tp + fp > 0, tp / np.maximum(tp + fp, 1), 0.0)
    r = np.where(tp + fn > 0, tp / np.maximum(tp + fn, 1), 0.0)
    return np.stack([p, r, np.where(p + r > 0, 2 * p * r / np.maximum(p + r, 1e-12), 0.0)], axis=1)


def interval(v) -> list[float]:
    return [float(np.percentile(v, 2.5)), float(np.percentile(v, 97.5))]


def measure(per: np.ndarray, tables: np.ndarray, base: np.ndarray | None = None) -> dict:
    """Pooled and macro measures with intervals, by size, and minus a base read on the same databases."""
    p, r, f = prf(*per.sum(axis=0))
    idx = np.random.default_rng(SEED).integers(0, len(per), (1000, len(per)))
    boot = resampled(per, idx)
    macro = np.array([prf(*c)[2] for c in per])
    out = {"databases": int(len(per)), "keys": int(per[:, 0].sum() + per[:, 2].sum()),
           "precision": p, "recall": r, "f1": f, "f1_95": interval(boot[:, 2]), "macro f1": float(macro.mean()),
           "by tables": {}}
    for b in BINS:
        m = (tables >= b[0]) & (tables < b[1])
        if m.any():
            bp, br, bf = prf(*per[m].sum(axis=0))
            out["by tables"][bin_name(b)] = {"databases": int(m.sum()), "keys": int(per[m][:, 0].sum() + per[m][:, 2].sum()),
                                             "precision": bp, "recall": br, "f1": bf,
                                             "macro f1": float(macro[m].mean())}
    if base is not None:
        bb = resampled(base, idx)
        bmacro = np.array([prf(*c)[2] for c in base])
        out["minus base, f1"] = f - prf(*base.sum(axis=0))[2]
        out["minus base, f1_95"] = interval(boot[:, 2] - bb[:, 2])
        out["minus base, macro f1"] = float(macro.mean() - bmacro.mean())
        out["minus base, macro f1_95"] = interval(macro[idx].mean(axis=1) - bmacro[idx].mean(axis=1))
    return out


def line(name: str, m: dict) -> str:
    by = " ".join(f"{k}:{v['f1']:.3f}" for k, v in m["by tables"].items())
    d = f" | minus base {m['minus base, f1']:+.3f} [{m['minus base, f1_95'][0]:+.3f},{m['minus base, f1_95'][1]:+.3f}]" \
        if "minus base, f1" in m else ""
    return (f"{name:32} P {m['precision']:.3f} R {m['recall']:.3f} F1 {m['f1']:.3f} macro {m['macro f1']:.3f}"
            f"{d} | {by}")
