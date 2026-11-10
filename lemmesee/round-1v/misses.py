"""Round 1v: a model's declared keys on a set, by what became of each, and its misses listed.

    python -I misses.py <first> <kind>[-<tag>] <set> [<lines>]
"""
import json
import pickle
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import numpy as np  # noqa: E402
import whole  # noqa: E402

OUT = whole.OUT


def load(first, second, name):
    data = pickle.load(open(OUT / "sets" / f"{whole.slug(name)}.pkl", "rb"))
    rec = pickle.load(open(OUT / "first" / first / f"{whole.slug(name)}.pkl", "rb"))["pairs"]
    d = OUT / "second" / f"{first}--{second}"
    raw = np.load(d / "sets" / f"{whole.slug(name)}.npy")
    threshold = json.load(open(d / "second.json", encoding="utf-8"))["threshold"]
    return data["schemas"], rec, raw, threshold


def fates(schemas, rec, raw, threshold):
    """(schema index, table, column, target) -> (fate, its score, the winner, the winner's score)."""
    ck = whole.column_keys(rec)
    order = np.lexsort((-raw, ck))
    out = {}
    start = 0
    n = len(order)
    while start < n:
        end = start
        while end < n and ck[order[end]] == ck[order[start]]:
            end += 1
        rows = order[start:end]
        lab = [i for i in rows if rec["label"][i]]
        top = rows[0]
        s = schemas[rec["schema"][top]]
        t = s["tables"][rec["table"][top]]
        col = t["columns"][rec["column"][top]]["name"]
        winner = s["tables"][rec["target"][top]]["name"]
        if lab:
            i = lab[0]
            g = s["tables"][rec["target"][i]]["name"]
            if i == top:
                fate = "found" if raw[i] >= threshold else "first, below the threshold"
            else:
                fate = "another target first, drawn" if raw[top] >= threshold else "another target first, none drawn"
            out[(int(rec["schema"][top]), t["name"], col, g)] = (fate, float(raw[i]), winner, float(raw[top]))
        elif raw[top] >= threshold:
            out[(int(rec["schema"][top]), t["name"], col, winner)] = ("drawn, no key declared", float(raw[top]), winner, float(raw[top]))
        start = end
    for si, s in enumerate(schemas):
        for a, c, g in s["fk"]:
            if (si, a, c, g) not in out:
                out[(si, a, c, g)] = ("no candidate", 0.0, "", 0.0)
    return out


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    first, second, name = sys.argv[1:4]
    lines = int(sys.argv[4]) if len(sys.argv) > 4 else 60
    schemas, rec, raw, threshold = load(first, second, name)
    f = fates(schemas, rec, raw, threshold)
    tables = [len(s["tables"]) for s in schemas]
    by = defaultdict(Counter)
    for (si, a, c, g), (fate, *_ ) in f.items():
        b = next(whole.bin_name(x) for x in whole.BINS if x[0] <= tables[si] < x[1])
        by[b][fate] += 1
        by["all"][fate] += 1
    kinds = ["found", "first, below the threshold", "another target first, drawn", "another target first, none drawn",
             "no candidate", "drawn, no key declared"]
    print(f"{first}--{second} on {name}, threshold {threshold:.3f}")
    for b in [whole.bin_name(x) for x in whole.BINS] + ["all"]:
        if b not in by:
            continue
        k = sum(by[b][x] for x in kinds[:5])
        print(f"  tables {b:>6}: keys {k:5d} | " + " | ".join(f"{x} {by[b][x] / max(k, 1):.2f}" for x in kinds))
    # the largest groups of misses: one column name into one target, a database
    groups = defaultdict(list)
    for (si, a, c, g), v in f.items():
        if v[0] not in ("found", "drawn, no key declared"):
            groups[(si, c.lower(), g)].append((a, v))
    print(f"  misses: {sum(len(v) for v in groups.values())} in {len(groups)} (database, column name, target) groups")
    for (si, c, g), v in sorted(groups.items(), key=lambda kv: -len(kv[1]))[:lines]:
        fate = Counter(x[1][0] for x in v).most_common(1)[0][0]
        sc = np.mean([x[1][1] for x in v])
        win = Counter(x[1][2] for x in v).most_common(1)[0][0]
        ws = np.mean([x[1][3] for x in v])
        print(f"   {len(v):3d} {schemas[si]['id'].split('__')[-1][:22]:22} ({tables[si]:3d} t) {c[:26]:26} -> {g[:28]:28} "
              f"{sc:.2f} | {fate[:28]:28} | first: {win[:24]:24} {ws:.2f}")
