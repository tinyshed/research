"""Round 1v: two second stages' scores averaged, a pair at a time.

    python -I blend.py <first>--<kind> <first>--<kind> [<weight of the first>] [write]

Both models score the same pairs of a set (first.py's kept pairs), so their
raw scores are averaged pair by pair, one target a column kept after. The
threshold is the best F1 over every declared key of SchemaPile's validation
split read beside its renamed copies (dump.py's two validation sets), as
every fit's is. `write` keeps the blend's scoring as
corpus/round-1v/scored/<a>+<b>.json for table.py and choose.py.
"""

import json
import pickle
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import numpy as np  # noqa: E402

import whole  # noqa: E402

OUT = whole.OUT
VALIDATION = ("schemapile validation", "schemapile validation, renamed")


def scored(model: str, name: str):
    """A model's rows of a set and its raw scores of them, in the order its first stage scored them."""
    rec = pickle.load(open(OUT / "first" / model.split("--")[0] / f"{whole.slug(name)}.pkl", "rb"))["pairs"]
    raw = np.load(OUT / "second" / model / "sets" / f"{whole.slug(name)}.npy")
    assert len(raw) == len(rec["schema"]), (model, name)
    return rec, raw


def row_keys(rec: dict) -> np.ndarray:
    """A number a pair: its schema, table, column and target."""
    return ((rec["schema"].astype(np.int64) << 40) | (rec["table"].astype(np.int64) << 28)
            | (rec["column"].astype(np.int64) << 14) | rec["target"].astype(np.int64))


def counts_at(schemas, rec, kept, threshold) -> np.ndarray:
    per = np.zeros((len(schemas), 3), dtype=np.int64)
    truth = [{tuple(k) for k in s["fk"]} for s in schemas]
    drawn = [set() for _ in schemas]
    for i in np.flatnonzero(kept >= threshold):
        s = schemas[rec["schema"][i]]
        t = s["tables"][rec["table"][i]]
        drawn[rec["schema"][i]].add((t["name"], t["columns"][rec["column"][i]]["name"], s["tables"][rec["target"][i]]["name"]))
    for si in range(len(schemas)):
        per[si] = [len(truth[si] & drawn[si]), len(drawn[si] - truth[si]), len(truth[si] - drawn[si])]
    return per


def best_threshold(sets: list) -> float:
    """The threshold of the best F1 over every declared key of the sets together."""
    scores, labels, missing = [], [], 0
    for schemas, rec, kept in sets:
        m = kept > 0
        scores.append(kept[m])
        labels.append(rec["label"][m])
        missing += sum(len({tuple(k) for k in s["fk"]}) for s in schemas) - int(rec["label"][m].sum())
    s = np.concatenate(scores)
    y = np.concatenate(labels)
    order = np.argsort(-s)
    tp = np.cumsum(y[order])
    drawn = np.arange(1, len(order) + 1)
    total = int(y.sum()) + missing
    f1 = 2 * tp / (drawn + total)
    return float(s[order][int(np.argmax(f1))])


def kept_of(a: str, b: str, w: float, name: str):
    """The two models' scores of a set averaged a pair. Their rows are the
    same pairs but need not lie in the same order (a set's pairs are shared
    out over the workers that build them), so the second model's are put in
    the first's order by the pair itself."""
    schemas = pickle.load(open(OUT / "sets" / f"{whole.slug(name)}.pkl", "rb"))["schemas"]
    rec, raw = scored(a, name)
    if w < 1:
        rec_b, raw_b = scored(b, name)
        ka, kb = row_keys(rec), row_keys(rec_b)
        oa, ob = np.argsort(ka, kind="stable"), np.argsort(kb, kind="stable")
        assert len(ka) == len(kb) and (ka[oa] == kb[ob]).all(), "the two models scored different pairs"
        other = np.empty_like(raw_b)
        other[oa] = raw_b[ob]
        raw = w * raw + (1 - w) * other
    return schemas, rec, whole.one_target(raw.astype(np.float64), rec)


def main(a: str, b: str, w: float, write: bool) -> None:
    threshold = best_threshold([kept_of(a, b, w, n) for n in VALIDATION])
    label = f"{a}+{b}" + (f"@{w}" if w != 0.5 else "")
    out = {"model": label, "threshold": threshold, "weight of the first": w, "sets": {}}
    print(f"{label}: threshold {threshold:.3f}")
    for name in whole.SETS:  # a set at a time: all of them at once do not fit beside a fit
        if not (OUT / "second" / a / "sets" / f"{whole.slug(name)}.npy").exists():
            continue
        schemas, rec, k = kept_of(a, b, w, name)
        per = counts_at(schemas, rec, k, threshold)
        tables = np.array([len(s["tables"]) for s in schemas])
        m = whole.measure(per, tables)
        out["sets"][name] = {"per database": per.tolist(), "measures": m}
        print(whole.line(name[:32], m))
    if write:
        (OUT / "scored" / f"{label}.json").write_text(json.dumps(out) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    args = [x for x in sys.argv[1:] if x != "write"]
    main(args[0], args[1], float(args[2]) if len(args) > 2 else 0.5, "write" in sys.argv)
