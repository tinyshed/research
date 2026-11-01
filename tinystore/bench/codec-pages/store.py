"""Store each bodies file in SQLite at several page sizes, divide the file,
and predict it with the page model from the rows' actual sizes.

    python3 -I store.py --model <repo>/tinystore/bench/page-model/page_model.py \
        --dataset <name> --pages 1024,4096,8192,16384 --tmp <dir> --out <results.jsonl> \
        <bodies file> [<bodies file> ...]

Each block is one row of TinyStore's payload table shape,
`(id integer primary key, body blob not null) strict`, ids 1..N in block
order, written as page_model.py writes its files: a fresh file, `page_size`
set, `auto_vacuum=none`, WAL, every row in one transaction, then
`wal_checkpoint(truncate)`. The file is divided by `dbstat` and by
page_model.py's own walk of every page, and predicted by page_model.py's
`predict` from `length(body)` of every row, read back from the file.

One JSON line a bodies file and page size.
"""

import argparse
import importlib.util
import json
import os
import re
import sqlite3
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cpfiles import read_bodies  # noqa: E402


def load_model(path):
    spec = importlib.util.spec_from_file_location("page_model", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def build(pm, path, U, bodies):
    for suffix in ("", "-wal", "-shm"):
        if os.path.exists(path + suffix):
            os.remove(path + suffix)
    create, insert, _ = pm.SCHEMAS["blob_rowid"]
    con = sqlite3.connect(path, isolation_level=None)
    con.execute(f"pragma page_size={U}")
    con.execute("pragma auto_vacuum=none")
    con.execute("pragma journal_mode=wal")
    con.execute("pragma synchronous=off")
    con.execute(create)
    con.execute("begin")
    con.executemany(insert, ((i + 1, b) for i, b in enumerate(bodies)))
    con.execute("commit")
    con.execute("pragma wal_checkpoint(truncate)")
    out = {
        "page_count": con.execute("pragma page_count").fetchone()[0],
        "freelist": con.execute("pragma freelist_count").fetchone()[0],
        "page_size": con.execute("pragma page_size").fetchone()[0],
        "dbstat": pm.dbstat_census(con),
    }
    # the model is fed the stored rows' sizes, read back, not the input's
    out["sizes"] = [r[0] for r in con.execute("select length(body) from t order by id")]
    con.close()
    out["walk"] = pm.walk_file(path)
    for suffix in ("", "-wal", "-shm"):
        if os.path.exists(path + suffix):
            os.remove(path + suffix)
    return out


def one(pm, ds, bodies_path, pages, tmp):
    m = re.match(r"(.+)\.L(\d+)\.bodies$", os.path.basename(bodies_path))
    codec, L = m.group(1), int(m.group(2))
    recs = list(read_bodies(bodies_path, with_bodies=True))
    bodies = [r[3] for r in recs]
    failed = sum(1 for r in recs if r[2] != 0)
    values = sum(r[1] for r in recs)
    payload = sum(len(b) for b in bodies)
    out = []
    for U in pages:
        t0 = time.time()
        meas = build(pm, os.path.join(tmp, f"{ds}-{codec}-{L}-{U}.db"), U, bodies)
        sizes = meas.pop("sizes")
        assert sizes == [len(b) for b in bodies]
        pred = pm.predict("blob_rowid", U, sizes, list(range(1, len(sizes) + 1)))
        w = meas["walk"]["objects"]["t"]
        ds_t = (meas["dbstat"] or {}).get("t", {})
        exact = {
            "page_count": pred["page_count"] == meas["page_count"],
            "leaf": pred["leaf"] == w["leaf"],
            "interior": pred["interior"] == w["interior"],
            "overflow": pred["overflow"] == w["overflow"],
            "dbstat_agrees_with_walk": (ds_t.get("leaf", 0) == w["leaf"] and ds_t.get("internal", 0) == w["interior"]
                                        and ds_t.get("overflow", 0) == w["overflow"]),
        }
        out.append({
            "dataset": ds, "codec": codec, "block": L, "page_size": U, "blocks": len(bodies), "values": values,
            "failed_blocks": failed, "payload_bytes": payload, "record_bytes": pred["record_bytes"],
            "file_bytes": meas["page_count"] * U, "measured": meas, "predicted": pred, "exact": exact,
            "seconds": round(time.time() - t0, 2),
        })
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True)
    ap.add_argument("--dataset", required=True)
    ap.add_argument("--pages", default="1024,4096,8192,16384")
    ap.add_argument("--tmp", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("bodies", nargs="+")
    a = ap.parse_args()
    pm = load_model(a.model)
    os.makedirs(a.tmp, exist_ok=True)
    pages = [int(p) for p in a.pages.split(",")]
    with open(a.out, "a") as f:
        for path in a.bodies:
            for rec in one(pm, a.dataset, path, pages, a.tmp):
                f.write(json.dumps(rec, sort_keys=True) + "\n")
                f.flush()
                e = rec["exact"]
                print(f"{rec['dataset']} {rec['codec']} L={rec['block']} U={rec['page_size']} "
                      f"payload={rec['payload_bytes']} file={rec['file_bytes']} "
                      f"exact={all(e.values())} {rec['seconds']}s", flush=True)


if __name__ == "__main__":
    main()
