"""Round 1v's choice, by the rule its specification wrote at 22:23 UTC on 9 November:
the fit with the highest F1 over the four GitHub development sets pooled,
if its F1 on SchemaPile's complete test schemas is not more than 0.01 under `1s`'s.

    python -I choose.py <scored model> [<scored model> ...]

A blend (`a+b`) is read beside the single models and is never the choice: it
is two models' bytes. The frozen models' rows are computed once from
dump.py's scores and kept (corpus/round-1v/choose-frozen.pkl).
"""
import json
import pickle
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import numpy as np  # noqa: E402
import whole  # noqa: E402

GITHUB = ("github 2025", "github 2025, rest", "github style", "github fresh")


def frozen_rows() -> dict:
    path = whole.OUT / "choose-frozen.pkl"
    if path.exists():
        return pickle.load(open(path, "rb"))
    data = pickle.load(open(whole.OUT / "sets" / "schemapile-test.pkl", "rb"))
    schemas = data["schemas"]
    truth = [{tuple(k) for k in s["fk"]} for s in schemas]
    by = {}
    for (sid, t, c), g in data["rules"].items():
        by.setdefault(sid, []).append((t, c, g))
    complete = np.array([all(k in truth[i] for k in by.get(s["id"], ())) for i, s in enumerate(schemas)])
    rows = {"complete": complete}
    for fm in ("1s", "nine-mended"):
        parts, tabs = [], []
        for g in GITHUB:
            sc = whole.Scored(fm, g)
            parts.append(sc.counts(sc.baseline()))
            tabs.append(sc.tables)
        sc = whole.Scored(fm, "schemapile test")
        rows[fm] = (np.vstack(parts), np.concatenate(tabs), sc.counts(sc.baseline())[complete])
    pickle.dump(rows, open(path, "wb"))
    return rows


def main(names):
    rows = frozen_rows()
    complete = rows.pop("complete")
    for n in names:
        r = json.load(open(whole.OUT / "scored" / f"{n}.json", encoding="utf-8"))["sets"]
        if not all(g in r for g in GITHUB) or "schemapile test" not in r:
            continue
        rows[n] = (np.vstack([np.array(r[g]["per database"]) for g in GITHUB]), rows["1s"][1],
                   np.array(r["schemapile test"]["per database"])[complete])
    floor = whole.prf(*rows["1s"][2].sum(axis=0))[2] - 0.01
    out = {}
    for n, (gh, tabs, comp) in rows.items():
        m = whole.measure(gh, tabs, rows["nine-mended"][0])
        c = whole.prf(*comp.sum(axis=0))[2]
        out[n] = {"github development sets, pooled": m, "schemapile test, complete, f1": c, "passes the guard": bool(c >= floor)}
        print(f"{n:34} github pooled P {m['precision']:.3f} R {m['recall']:.3f} F1 {m['f1']:.4f} macro {m['macro f1']:.3f} "
              f"minus nine-mended {m['minus base, f1']:+.3f} {[round(v, 3) for v in m['minus base, f1_95']]} | "
              + " ".join(f"{k}:{v['f1']:.3f}" for k, v in m["by tables"].items())
              + f" | complete test {c:.3f} {'ok' if c >= floor else 'UNDER THE GUARD'}")
    eligible = [n for n in names if n in out and out[n]["passes the guard"] and "+" not in n]
    choice = max(eligible, key=lambda n: out[n]["github development sets, pooled"]["f1"]) if eligible else None
    print("the rule's choice among single models:", choice)
    return out, choice


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(sys.argv[1:])
