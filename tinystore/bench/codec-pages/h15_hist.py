"""H1.5 from histograms: metrics-layout-2026-10-09's layouts predicted from the
row-size histograms its own raw output keeps, with no file rebuilt.

    python3 -I h15_hist.py --model <repo>/tinystore/bench/page-model/page_model.py \
        --data <repo>/tinystore/reports/data/metrics-layout-2026-10-09 --out <h15-hist.json>

Each variant's JSON in that round's data holds `directory_size_histogram`
(the length of every groups row's directory) and `body_size_histogram` (every
block body), the row counts of each table, and the file `dbstat` divided after
the rebuild. From the histograms alone this predicts the objects a layout
changes, the `groups` table (WITHOUT ROWID, or rowid with the UNIQUE autoindex
its primary key becomes) and `payloads` (every body longer than the inline
threshold), and from them the file as the fresh control's measured file plus
the predicted change. A histogram has no order, so each prediction is made
for 25 seeded permutations of the rows and given as its range; the integer
columns take sizes from evenly spread ids (series, clock) and six-byte
millisecond timestamps. Every other object is the control's.

Only the layout half of a choice is predicted: the directories of an inline
threshold other than 16 are the round's re-encoded ones, whose histogram is
in that variant's JSON.
"""

import argparse
import importlib.util
import json
import os
import random

VARIANTS = ("exact", "rowid", "inline0", "inline32", "inline64", "inline128",
            "rowid_inline0", "rowid_inline32", "rowid_inline64", "rowid_inline128")
TS = 1_767_225_600_000  # a six-byte millisecond time, as every corpus timestamp is
PERMUTATIONS = 25


def load_model(path):
    spec = importlib.util.spec_from_file_location("page_model", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def expand(hist):
    return [int(k) for k, v in sorted(hist.items(), key=lambda kv: int(kv[0])) for _ in range(v)]


def pages_of(c):
    return c["leaf"] + c["interior"] + c["overflow"]


def predict_groups(pm, dirs, series, clocks, rowid, U, rnd):
    order = dirs[:]
    rnd.shuffle(order)
    g = len(order)
    tb = pm.BTree(U, "table" if rowid else "index")
    ix = pm.BTree(U, "index")
    for i, dlen in enumerate(order):
        sid = 1 + i * series // g
        cid = 1 + i * clocks // g
        P = pm.record_len([("int", sid), ("int", TS), ("int", TS), ("blob", dlen), ("int", cid)])
        if rowid:
            tb.append(pm.table_leaf_cell(P, i + 1, U), i + 1, pm.overflow_pages(P, U, False))
            Q = pm.record_len([("int", sid), ("int", TS + i), ("int", i + 1)])
            ix.append(pm.index_leaf_cell(Q, U), (sid, i), pm.overflow_pages(Q, U, True))
        else:
            tb.append(pm.index_leaf_cell(P, U), (sid, i), pm.overflow_pages(P, U, True))
    return tb.census(), (ix.census() if rowid else None)


def predict_payloads(pm, bodies, inline, U, rnd, ids):
    order = [b for b in bodies if b > inline]
    rnd.shuffle(order)
    tb = pm.BTree(U, "table")
    for i, n in enumerate(order):
        rid = 1 + i * ids // max(1, len(order))
        P = pm.record_len([("null",), ("blob", n)])
        tb.append(pm.table_leaf_cell(P, rid, U), rid, pm.overflow_pages(P, U, False))
    return tb.census(), len(order)


def measured_objects(r):
    out = {}
    for o in r["physical"]["objects"]:
        d = out.setdefault(o["name"], {"leaf": 0, "internal": 0, "overflow": 0})
        d[o["type"]] = o["pages"]
    return {k: v | {"pages": v["leaf"] + v["internal"] + v["overflow"]} for k, v in out.items()}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True)
    ap.add_argument("--data", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    pm = load_model(a.model)
    result, control_runs = {}, {}
    for ds in ("tsbs", "alibaba"):
        control = json.load(open(os.path.join(a.data, f"{ds}-exact.json")))["result"]
        cobj = measured_objects(control)
        U = control["settings"]["page_size"]
        out = {}
        for v in VARIANTS:
            path = os.path.join(a.data, f"{ds}-{v}.json")
            if not os.path.exists(path):
                continue
            d = json.load(open(path))
            r, cfg = d["result"], d["config"]
            rowid, inline = bool(cfg["rowid"]), cfg["inline"]
            dirs = expand(r["directory_size_histogram"])
            bodies = expand(r["body_size_histogram"])
            assert len(dirs) == r["tables"]["groups"]
            series, clocks = r["tables"]["series"], r["tables"]["clocks"]
            ids = max(r["tables"]["payloads"], len(bodies))
            runs = []
            base = control_runs.get(ds)
            for seed in range(PERMUTATIONS):
                rnd = random.Random(seed)
                g, ix = predict_groups(pm, dirs, series, clocks, rowid, U, rnd)
                p, npay = predict_payloads(pm, bodies, inline, U, rnd, ids)
                gp, ip, pp = pages_of(g), (pages_of(ix) if ix else 0), pages_of(p)
                changed = (gp + ip + pp) - (cobj["groups"]["pages"] + cobj["payloads"]["pages"])
                total = gp + ip + pp
                run = {"groups": gp, "groups_overflow": g["overflow"], "index": ip, "payloads": pp,
                       "payload_rows": npay, "file": control["file_bytes"] + changed * U}
                if base is not None:
                    # the change against the control, both predicted with the same permutation
                    run["delta_vs_control_percent"] = round(100 * (total - base[seed]) * U / control["file_bytes"], 3)
                elif v == "exact":
                    control_runs.setdefault(ds, []).append(total)
                runs.append(run)
            mobj = measured_objects(r)
            meas = {"groups": mobj["groups"]["pages"], "groups_overflow": mobj["groups"]["overflow"],
                    "index": mobj.get("sqlite_autoindex_groups_1", {}).get("pages", 0),
                    "payloads": mobj["payloads"]["pages"], "payload_rows": r["tables"]["payloads"],
                    "file": r["file_bytes"]}
            if base is not None:
                meas["delta_vs_control_percent"] = round(100 * (r["file_bytes"] - control["file_bytes"])
                                                         / control["file_bytes"], 3)
            rng = {k: [min(x[k] for x in runs), max(x[k] for x in runs)] for k in runs[0]}
            out[v] = {"config": cfg, "predicted_range": rng, "measured": meas,
                      "within_range": {k: rng[k][0] <= meas[k] <= rng[k][1] for k in meas},
                      "file_error_bytes": [rng["file"][0] - meas["file"], rng["file"][1] - meas["file"]]}
        # the choice: rowid against WITHOUT ROWID at the production threshold, and the best threshold a layout
        def choice(names, key):
            pred = {n: sum(out[n]["predicted_range"]["file"]) / 2 for n in names if n in out}
            meas = {n: out[n]["measured"]["file"] for n in names if n in out}
            return {"predicted_best": min(pred, key=pred.get), "measured_best": min(meas, key=meas.get),
                    "predicted_order": sorted(pred, key=pred.get), "measured_order": sorted(meas, key=meas.get),
                    "key": key}
        out["choices"] = {
            "rowid_or_without_rowid": choice(("exact", "rowid"), "inline 16"),
            "threshold_without_rowid": choice(("inline0", "exact", "inline32", "inline64", "inline128"), "WITHOUT ROWID"),
            "threshold_rowid": choice(("rowid_inline0", "rowid", "rowid_inline32", "rowid_inline64", "rowid_inline128"), "rowid"),
        }
        out["control_file"] = control["file_bytes"]
        result[ds] = out
    with open(a.out, "w") as f:
        json.dump(result, f, indent=1, sort_keys=True)
    for ds, out in result.items():
        for v in VARIANTS:
            if v in out:
                o = out[v]
                print(ds, v, "pred file", o["predicted_range"]["file"], "meas", o["measured"]["file"],
                      "groups", o["predicted_range"]["groups"], o["measured"]["groups"],
                      "payloads", o["predicted_range"]["payloads"], o["measured"]["payloads"],
                      "index", o["predicted_range"]["index"], o["measured"]["index"],
                      "delta%", o["predicted_range"].get("delta_vs_control_percent"),
                      o["measured"].get("delta_vs_control_percent"))
        print(ds, json.dumps(out["choices"]))


if __name__ == "__main__":
    main()
