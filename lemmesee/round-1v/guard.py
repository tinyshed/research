"""Round 1v's guard: F1 on SchemaPile's test schemas whose every rule-named
column is declared (round 1b's `complete`), by size, beside the whole split.

    python -I guard.py <scored model> [<scored model> ...]

A model taught to draw keys nobody declared loses F1 over every declared key
where the declarations are thin, for that alone; on the schemas that declare
every key the name rules guess, a key drawn and not declared is more likely
a wrong one. The frozen models are read beside the round's.
"""

import json
import pickle
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import numpy as np  # noqa: E402

import whole  # noqa: E402

NAME = "schemapile test"


def main(names):
    data = pickle.load(open(whole.OUT / "sets" / f"{whole.slug(NAME)}.pkl", "rb"))
    schemas = data["schemas"]
    truth = [{tuple(k) for k in s["fk"]} for s in schemas]
    by = {}
    for (sid, t, c), g in data["rules"].items():
        by.setdefault(sid, []).append((t, c, g))
    complete = np.array([all(k in truth[i] for k in by.get(s["id"], ())) for i, s in enumerate(schemas)])
    tables = np.array([len(s["tables"]) for s in schemas])
    rows = {}
    for fm in ("1s", "seven-bounded", "nine-mended"):
        sc = whole.Scored(fm, NAME)
        rows[fm] = sc.counts(sc.baseline())
    for n in names:
        r = json.load(open(whole.OUT / "scored" / f"{n}.json", encoding="utf-8"))
        rows[n] = np.array(r["sets"][NAME]["per database"])
    out = {"schemas": int(len(schemas)), "complete": int(complete.sum())}
    print(f"{NAME}: {len(schemas)} schemas, {int(complete.sum())} of them complete by the name rules")
    for n, per in rows.items():
        m_all = whole.measure(per, tables)
        m_c = whole.measure(per[complete], tables[complete])
        m_r = whole.measure(per[~complete], tables[~complete])
        out[n] = {"all": m_all, "complete": m_c, "the rest": m_r}
        print(f"{n:30} all {m_all['f1']:.3f} | complete {m_c['f1']:.3f} (P {m_c['precision']:.3f} R {m_c['recall']:.3f}) "
              + " ".join(f"{k}:{v['f1']:.3f}" for k, v in m_c["by tables"].items())
              + f" | the rest {m_r['f1']:.3f} (P {m_r['precision']:.3f} R {m_r['recall']:.3f})")
    return out


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(sys.argv[1:])
