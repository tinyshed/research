"""H1.5: choose the groups table's layout offline from a TinyStore file's rows.

    python3 -I h15.py --model <repo>/tinystore/bench/page-model/page_model.py \
        --db <metrics.db> --work <dir> --out <h15.json>

From a closed TinyStore metrics file (built at e307c48 by the
metrics-layout round's Go builder), this

1. divides the file by object with `dbstat`;
2. reads the `groups` rows in primary-key order and the size of each record,
   and gives their histogram against the page model's spill limits;
3. predicts with page_model.py, before any rebuild, the pages of the groups
   object as WITHOUT ROWID (an index b-tree written in key order, as the
   round's fresh control copies it) and as a rowid table with its
   `primary key (series_id, start_ts)` become a UNIQUE autoindex, the
   round's `rowid` variant;
4. only then rebuilds both as the round's harness does (`fresh()` in
   metrics-layout-bench/rust/src/main.rs: tables from the original DDL, the
   groups DDL without `without rowid` for the variant, every table copied by
   `insert ... select *`, explicit indexes created after, one transaction,
   WAL, checkpoint) with this Python's SQLite, and divides them.
"""

import argparse
import importlib.util
import json
import os
import sqlite3

TABLE_ORDER_LAST = ("groups", "clocks", "payloads")


def load_model(path):
    spec = importlib.util.spec_from_file_location("page_model", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def divide(path):
    con = sqlite3.connect(f"file:{path}?mode=ro&immutable=1", uri=True)
    out = {"page_size": con.execute("pragma page_size").fetchone()[0],
           "page_count": con.execute("pragma page_count").fetchone()[0],
           "freelist": con.execute("pragma freelist_count").fetchone()[0], "objects": {}}
    for name, ptype, n, unused in con.execute(
            "select name, pagetype, count(*), sum(unused) from dbstat group by name, pagetype"):
        o = out["objects"].setdefault(name, {"pages": 0})
        o[ptype] = n
        o["unused_" + ptype] = unused
        o["pages"] += n
    con.close()
    out["file_bytes"] = out["page_count"] * out["page_size"]
    return out


def group_rows(path):
    con = sqlite3.connect(f"file:{path}?mode=ro&immutable=1", uri=True)
    rows = con.execute("select series_id, start_ts, end_ts, length(directory), clock_id from groups "
                       "order by series_id, start_ts").fetchall()
    con.close()
    return rows


def predict(pm, rows, U):
    wr = pm.BTree(U, "index")
    tb = pm.BTree(U, "table")
    ix = pm.BTree(U, "index")
    records = []
    for rowid, (sid, start, end, dlen, cid) in enumerate(rows, start=1):
        cols = [("int", sid), ("int", start), ("int", end), ("blob", dlen), ("int", cid)]
        P = pm.record_len(cols)
        records.append(P)
        wr.append(pm.index_leaf_cell(P, U), (sid, start), pm.overflow_pages(P, U, True))
        tb.append(pm.table_leaf_cell(P, rowid, U), rowid, pm.overflow_pages(P, U, False))
        Q = pm.record_len([("int", sid), ("int", start), ("int", rowid)])
        ix.append(pm.index_leaf_cell(Q, U), (sid, start), pm.overflow_pages(Q, U, True))
    cw, ct, ci = wr.census(), tb.census(), ix.census()
    pages = lambda c: c["leaf"] + c["interior"] + c["overflow"]  # noqa: E731
    return records, {
        "without_rowid": cw | {"pages": pages(cw), "bytes": pages(cw) * U},
        "rowid_table": ct | {"pages": pages(ct), "bytes": pages(ct) * U},
        "rowid_unique_index": ci | {"pages": pages(ci), "bytes": pages(ci) * U},
        "delta_bytes": (pages(ct) + pages(ci) - pages(cw)) * U,
    }


def histogram(pm, records, U):
    X = pm.max_local(U, True)
    bins = {}
    for P in records:
        b = P // 128 * 128
        bins[b] = bins.get(b, 0) + 1
    return {"bin_bytes": 128, "counts": {str(k): v for k, v in sorted(bins.items())},
            "index_max_local_X": X, "table_max_local_X": pm.max_local(U, False),
            "records_over_index_X": sum(1 for P in records if P > X),
            "records_over_table_X": sum(1 for P in records if P > pm.max_local(U, False)),
            "records": len(records), "record_bytes": sum(records),
            "max_record": max(records), "mean_record": round(sum(records) / len(records), 1)}


def rebuild(src, dest, rowid):
    for s in ("", "-wal", "-shm"):
        if os.path.exists(dest + s):
            os.remove(dest + s)
    srccon = sqlite3.connect(f"file:{src}?mode=ro&immutable=1", uri=True)
    page = srccon.execute("pragma page_size").fetchone()[0]
    schema = srccon.execute("select type, name, sql from sqlite_schema where sql is not null order by rowid").fetchall()
    srccon.close()
    c = sqlite3.connect(dest, isolation_level=None)
    c.execute(f"pragma page_size={page}")
    c.execute("pragma foreign_keys=1")
    c.execute("pragma journal_mode=wal")
    c.execute("attach database ? as src", (f"file:{src}?mode=ro&immutable=1",))
    c.execute("begin immediate")
    for kind, name, sql in schema:
        if kind == "table":
            if name == "groups" and rowid:
                sql = sql.replace(", without rowid", "").replace(", WITHOUT ROWID", "")
            c.execute(sql)
    for kind, name, _ in schema:
        if kind == "table" and name not in TABLE_ORDER_LAST:
            c.execute(f"insert into main.{name} select * from src.{name}")
    for name in TABLE_ORDER_LAST:
        c.execute(f"insert into main.{name} select * from src.{name}")
    for kind, _, sql in schema:
        if kind == "index":
            c.execute(sql)
    c.execute("commit")
    c.execute("pragma wal_checkpoint(truncate)")
    c.execute("detach database src")
    c.close()
    return divide(dest)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True)
    ap.add_argument("--db", required=True)
    ap.add_argument("--work", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    pm = load_model(a.model)
    os.makedirs(a.work, exist_ok=True)
    original = divide(a.db)
    U = original["page_size"]
    rows = group_rows(a.db)
    records, pred = predict(pm, rows, U)
    result = {"sqlite": sqlite3.sqlite_version, "original": original,
              "groups_histogram": histogram(pm, records, U), "predicted": pred}
    # the prediction is written before anything is rebuilt
    with open(a.out, "w") as f:
        json.dump(result, f, indent=1, sort_keys=True)
    exact = rebuild(a.db, os.path.join(a.work, "exact.db"), rowid=False)
    variant = rebuild(a.db, os.path.join(a.work, "rowid.db"), rowid=True)
    result["rebuilt"] = {"exact": exact, "rowid": variant}
    ge = exact["objects"]["groups"]["pages"]
    gr = variant["objects"]["groups"]["pages"]
    gi = variant["objects"].get("sqlite_autoindex_groups_1", {}).get("pages", 0)
    result["check"] = {
        "without_rowid_pages": {"predicted": pred["without_rowid"]["pages"], "rebuilt": ge},
        "rowid_table_pages": {"predicted": pred["rowid_table"]["pages"], "rebuilt": gr},
        "rowid_index_pages": {"predicted": pred["rowid_unique_index"]["pages"], "rebuilt": gi},
        "delta_bytes": {"predicted": pred["delta_bytes"], "rebuilt": variant["file_bytes"] - exact["file_bytes"]},
        "rebuilt_exact_file": exact["file_bytes"], "rebuilt_rowid_file": variant["file_bytes"],
        "delta_percent_rebuilt": round(100 * (variant["file_bytes"] - exact["file_bytes"]) / exact["file_bytes"], 3),
        "delta_percent_predicted_on_rebuilt_exact": round(100 * pred["delta_bytes"] / exact["file_bytes"], 3),
    }
    with open(a.out, "w") as f:
        json.dump(result, f, indent=1, sort_keys=True)
    print(json.dumps(result["check"], indent=1), json.dumps(result["groups_histogram"], indent=1))


if __name__ == "__main__":
    main()
