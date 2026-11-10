"""Round 1v: a name read once for the tables that carry it, after the keys are drawn.

    python -I complete.py <first> <second> [<floor> <share> <least>]

A second stage draws a column's key when its score passes the threshold, a
column at a time. Here a schema's columns of one name (letters and digits,
lower case) are read together: where at least `least` of them are drawn into
table t, and those are at least `share` of the columns of that name whose
best target is t, the others whose best target is t are drawn too if they
score `floor` or more. The same name into the same table is one decision.

Prints, a set, F1 as the stage draws and with the name completed, by size;
development sets only (dump.py's).
"""

import json
import pickle
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "round-1b"))

import numpy as np  # noqa: E402

import whole  # noqa: E402
from train import norm  # noqa: E402

OUT = whole.OUT


def complete(schemas, rec, raw, threshold, floor=0.3, share=0.5, least=1) -> list[dict]:
    """{(table, column): target} a schema: the stage's keys and the completed ones."""
    kept = whole.one_target(raw, rec)
    out = [dict() for _ in schemas]
    groups = defaultdict(list)  # (schema, name, target) -> [(row, drawn)]
    for i in np.flatnonzero(kept > 0):
        si = int(rec["schema"][i])
        s = schemas[si]
        t = s["tables"][rec["table"][i]]
        c = t["columns"][rec["column"][i]]["name"]
        g = s["tables"][rec["target"][i]]["name"]
        drawn = kept[i] >= threshold
        if drawn:
            out[si][(t["name"], c)] = g
        groups[(si, norm(c), g)].append((i, drawn, t["name"], c))
    for (si, name, g), members in groups.items():
        d = sum(1 for m in members if m[1])
        if len(members) < 2 or d < least or d < share * len(members):
            continue
        for i, drawn, t, c in members:
            if not drawn and kept[i] >= floor:
                out[si][(t, c)] = g
    return out


def counts(schemas, preds) -> np.ndarray:
    per = np.zeros((len(schemas), 3), dtype=np.int64)
    for si, (s, p) in enumerate(zip(schemas, preds)):
        truth = {tuple(k) for k in s["fk"]}
        drawn = {(t, c, g) for (t, c), g in p.items()}
        per[si] = [len(truth & drawn), len(drawn - truth), len(truth - drawn)]
    return per


def load(first: str, second: str, name: str):
    data = pickle.load(open(OUT / "sets" / f"{whole.slug(name)}.pkl", "rb"))
    rec = pickle.load(open(OUT / "first" / first / f"{whole.slug(name)}.pkl", "rb"))["pairs"]
    d = OUT / "second" / f"{first}--{second}"
    raw = np.load(d / "sets" / f"{whole.slug(name)}.npy")
    return data["schemas"], rec, raw, json.load(open(d / "second.json", encoding="utf-8"))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    first, second = sys.argv[1:3]
    settings = [tuple(float(x) for x in sys.argv[3:6])] if len(sys.argv) > 5 else \
        [(0.5, 0.5, 1), (0.3, 0.5, 1), (0.3, 0.3, 1), (0.1, 0.5, 1), (0.3, 0.5, 2), (0.1, 0.3, 2)]
    rows = defaultdict(list)
    for name in whole.SETS:
        if not (OUT / "second" / f"{first}--{second}" / "sets" / f"{whole.slug(name)}.npy").exists():
            continue
        schemas, rec, raw, result = load(first, second, name)
        tables = np.array([len(s["tables"]) for s in schemas])
        base = counts(schemas, complete(schemas, rec, raw, result["threshold"], floor=2.0))
        rows["as drawn"].append((name, whole.measure(base, tables)))
        for floor, share, least in settings:
            per = counts(schemas, complete(schemas, rec, raw, result["threshold"], floor, share, int(least)))
            rows[f"floor {floor}, share {share}, least {int(least)}"].append((name, whole.measure(per, tables, base)))
    names = [n for n, _ in rows["as drawn"]]
    print(f"{first}--{second}: F1 over every declared key")
    print(f"{'':32}" + "".join(f"{n[:9]:>10}" for n in names))
    for k, v in rows.items():
        print(f"{k:32}" + "".join(f"{m['f1']:>10.3f}" for _, m in v))
    print("precision")
    for k, v in rows.items():
        print(f"{k:32}" + "".join(f"{m['precision']:>10.3f}" for _, m in v))
