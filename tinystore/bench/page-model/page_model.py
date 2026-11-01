#!/usr/bin/env python3
"""A page model for SQLite files, and the experiment that checks it.

The model predicts how many pages a freshly written SQLite file takes for N
rows of a given layout, from the rows' payload sizes alone:

* how many bytes a cell takes on its page and how many spill to overflow
  pages, from the database file format
  (https://www.sqlite.org/fileformat2.html, sections 1.6 "B-tree Pages",
  1.7 "Cell Payload Overflow Pages" and 2.1 "Record Format");
* how full each b-tree page is left when rows arrive in key order, from
  SQLite's balancing code (src/btree.c: balance_quick, balance_nonroot,
  balance_deeper), which the file format leaves to the implementation.

The experiment builds the same files with the standard library's sqlite3,
checkpoints them, reads page_count and freelist_count, and divides every file
by walking each b-tree page by page (and through dbstat where the SQLite
build has it). It measures layout, not time: the result depends only on the
SQLite file format and b-tree code, not on the machine.

Standard library only. Reproduce with:

    python <repo>/tinystore/bench/page-model/page_model.py --out <repo>/tinystore/reports/data/page-model-2026-11-01
"""

import argparse
import json
import os
import platform
import random
import sqlite3
import struct
import sys
import tempfile
import time

# --------------------------------------------------------------------------
# The file format: record, cell and overflow sizes (fileformat2 1.6, 1.7, 2.1)
# --------------------------------------------------------------------------


def varint_len(v):
    """Bytes of SQLite's big-endian varint: 7 bits a byte, 8 in the ninth."""
    if v < 0:
        return 9
    for n, bits in enumerate((7, 14, 21, 28, 35, 42, 49, 56), start=1):
        if v < (1 << bits):
            return n
    return 9


def int_serial(v):
    """Serial type and content bytes of an integer (schema format 4)."""
    if v == 0:
        return 8, 0
    if v == 1:
        return 9, 0
    for st, nbytes in ((1, 1), (2, 2), (3, 3), (4, 4), (5, 6)):
        lim = 1 << (8 * nbytes - 1)
        if -lim <= v < lim:
            return st, nbytes
    return 6, 8


def record_len(cols):
    """Bytes of a record: header (its own size varint and one serial type a
    column) and body. A column is ('null',), ('int', v) or ('blob', n)."""
    types, body = [], 0
    for c in cols:
        if c[0] == "null":
            st, ln = 0, 0
        elif c[0] == "int":
            st, ln = int_serial(c[1])
        elif c[0] == "blob":
            st, ln = 12 + 2 * c[1], c[1]
        else:
            raise ValueError(c)
        types.append(st)
        body += ln
    tl = sum(varint_len(t) for t in types)
    h = tl + 1
    while varint_len(h) + tl != h:
        h = varint_len(h) + tl
    return h + body


def max_local(U, index):
    """X: the most payload a cell keeps on its page (1.6)."""
    return ((U - 12) * 64 // 255 - 23) if index else U - 35


def min_local(U):
    """M: the least payload a spilling cell keeps on its page (1.6)."""
    return (U - 12) * 32 // 255 - 23


def local_len(P, U, index):
    X = max_local(U, index)
    if P <= X:
        return P
    M = min_local(U)
    K = M + (P - M) % (U - 4)
    return K if K <= X else M


def overflow_pages(P, U, index):
    """Pages in a cell's overflow chain: U-4 content bytes each (1.7)."""
    loc = local_len(P, U, index)
    if loc == P:
        return 0
    return -(-(P - loc) // (U - 4))


def table_leaf_cell(P, rowid, U):
    loc = local_len(P, U, False)
    sz = varint_len(P) + varint_len(rowid) + loc + (4 if loc < P else 0)
    return max(sz, 4)


def index_leaf_cell(P, U):
    loc = local_len(P, U, True)
    sz = varint_len(P) + loc + (4 if loc < P else 0)
    return max(sz, 4)


# --------------------------------------------------------------------------
# The balancing: how full pages are left (src/btree.c, appends at the right)
# --------------------------------------------------------------------------


class Node:
    __slots__ = ("leaf", "cells", "kids")

    def __init__(self, leaf):
        self.leaf = leaf
        self.cells = []  # (base_size, key); see BTree.size
        self.kids = []  # interior: len(kids) == len(cells) + 1


class BTree:
    """Simulates one b-tree receiving cells in increasing key order.

    kind 'table' is a rowid table (intkey, data in leaves); 'index' is an
    index b-tree, which is also how a WITHOUT ROWID table is stored. bulk
    reproduces BTREE_BULKLOAD, which VACUUM's transfer and CREATE INDEX set
    on index cursors. The code paths followed are balance() with its
    one-third-full rule, balance_deeper, balance_quick (table leaves at the
    right edge only) and balance_nonroot: siblings chosen around the right-most
    child, cells packed left first, then evened from the right unless bulk.
    """

    def __init__(self, U, kind, bulk=False):
        self.U, self.kind, self.bulk = U, kind, bulk
        self.root = Node(True)
        self.alloc = 1  # the root page
        self.free = 0  # pages on the freelist
        self.hw = 1  # high-water mark of pages ever allocated
        self.overflow = 0

    # page accounting -------------------------------------------------------
    def _new_page(self):
        if self.free:
            self.free -= 1
        else:
            self.hw += 1
        self.alloc += 1

    def _free_page(self):
        self.free += 1
        self.alloc -= 1

    def size(self, cell, leaf):
        base, key = cell
        if leaf:
            return base
        if self.kind == "index":
            return base + 4  # interior index cell: child pointer + leaf cell
        return 4 + varint_len(key)  # interior table cell: child pointer + key

    def used(self, node):
        return sum(2 + self.size(c, node.leaf) for c in node.cells)

    def room(self, node):
        return self.U - (8 if node.leaf else 12)

    # insertion -------------------------------------------------------------
    def append(self, base, key, ovfl_pages):
        for _ in range(ovfl_pages):
            self._new_page()
        self.overflow += ovfl_pages
        path = [self.root]
        while not path[-1].leaf:
            path.append(path[-1].kids[-1])
        leaf = path[-1]
        leaf.cells.append((base, key))
        if self.used(leaf) > self.room(leaf):
            self._balance(path)

    def _balance(self, path):
        lvl = len(path) - 1
        while True:
            node = path[lvl]
            over = self.used(node) > self.room(node)
            free = self.room(node) - self.used(node)
            if not over and free * 3 <= self.U * 2:
                return
            if lvl == 0:
                if not over:
                    return
                child = Node(node.leaf)
                child.cells, child.kids = node.cells, node.kids
                node.leaf, node.cells, node.kids = False, [], [child]
                self._new_page()
                path.insert(1, child)
                lvl = 1
                continue
            parent = path[lvl - 1]
            if self.kind == "table" and node.leaf and over:
                self._balance_quick(parent, node)
            else:
                self._balance_nonroot(parent, is_root=(lvl - 1 == 0))
            lvl -= 1

    def _balance_quick(self, parent, node):
        cell = node.cells.pop()
        new = Node(True)
        new.cells.append(cell)
        self._new_page()
        parent.cells.append((None, node.cells[-1][1]))
        parent.kids.append(new)

    def _balance_nonroot(self, parent, is_root):
        bulk = 1 if self.bulk else 0
        nc = len(parent.cells)
        if nc < 2:
            nx, n_old = 0, nc + 1
        else:
            nx, n_old = nc - 2 + bulk, 2 - bulk + 1
        sibs = parent.kids[nx:nx + n_old]
        divs = parent.cells[nx:nx + n_old - 1]
        leaf = sibs[0].leaf
        leaf_data = self.kind == "table" and leaf
        usable = self.U - 12 + (4 if leaf else 0)

        cells, kids, sz_new, cnt_new = [], [], [], []
        for j, s in enumerate(sibs):
            sz_new.append(sum(2 + self.size(c, leaf) for c in s.cells))
            cells.extend(s.cells)
            if not leaf:
                kids.extend(s.kids[:-1])
            cnt_new.append(len(cells))
            if j < n_old - 1 and not leaf_data:
                cells.append(divs[j])
                if not leaf:
                    kids.append(s.kids[-1])
        right_kid = None if leaf else sibs[-1].kids[-1]
        n = len(cells)
        sz = [self.size(c, leaf) for c in cells]

        k = n_old
        i = 0
        while i < k:
            while sz_new[i] > usable:
                if i + 1 >= k:
                    k = i + 2
                    if len(sz_new) < k:
                        sz_new.append(0)
                        cnt_new.append(n)
                    else:
                        sz_new[k - 1], cnt_new[k - 1] = 0, n
                s = 2 + sz[cnt_new[i] - 1]
                sz_new[i] -= s
                if not leaf_data:
                    s = 2 + sz[cnt_new[i]] if cnt_new[i] < n else 0
                sz_new[i + 1] += s
                cnt_new[i] -= 1
            while cnt_new[i] < n:
                s = 2 + sz[cnt_new[i]]
                if sz_new[i] + s > usable:
                    break
                sz_new[i] += s
                cnt_new[i] += 1
                if not leaf_data:
                    s = 2 + sz[cnt_new[i]] if cnt_new[i] < n else 0
                sz_new[i + 1] -= s
            if cnt_new[i] >= n:
                k = i + 1
            i += 1
        del sz_new[k:], cnt_new[k:]

        for i in range(k - 1, 0, -1):
            sz_right, sz_left = sz_new[i], sz_new[i - 1]
            r = cnt_new[i - 1] - 1
            d = r + 1 - (1 if leaf_data else 0)
            while True:
                sz_r, sz_d = sz[r], sz[d]
                if sz_right != 0 and (
                    bulk or sz_right + sz_d + 2 > sz_left - (sz_r + (0 if i == k - 1 else 2))
                ):
                    break
                sz_right += sz_d + 2
                sz_left -= sz_r + 2
                cnt_new[i - 1] = r
                r -= 1
                d -= 1
                if r < 0:
                    break
            sz_new[i], sz_new[i - 1] = sz_right, sz_left

        new_pages, new_divs = [], []
        start = 0
        for p in range(k):
            end = cnt_new[p]
            node = Node(leaf)
            node.cells = cells[start:end]
            if not leaf:
                node.kids = kids[start:end] + [kids[end] if p < k - 1 else right_kid]
            new_pages.append(node)
            if p < k - 1:
                if leaf_data:
                    new_divs.append((None, cells[end - 1][1]))
                    start = end
                else:
                    new_divs.append(cells[end])
                    start = end + 1
        for _ in range(k - n_old):
            self._new_page()
        for _ in range(n_old - k):
            self._free_page()
        parent.cells = parent.cells[:nx] + new_divs + parent.cells[nx + n_old - 1:]
        parent.kids = parent.kids[:nx] + new_pages + parent.kids[nx + n_old:]
        if is_root and not parent.cells and len(parent.kids) == 1:
            only = parent.kids[0]  # balance_shallower: the root takes its child
            parent.leaf, parent.cells, parent.kids = only.leaf, only.cells, only.kids
            self._free_page()

    # results ---------------------------------------------------------------
    def census(self):
        leaf = interior = 0
        stack = [self.root]
        while stack:
            nd = stack.pop()
            if nd.leaf:
                leaf += 1
            else:
                interior += 1
                stack.extend(nd.kids)
        return {"leaf": leaf, "interior": interior, "overflow": self.overflow,
                "freelist": self.free, "pages": self.hw}


def greedy_leaves(cell_sizes, U):
    """The closed form for appends to a rowid table: each leaf takes cells
    until the next one does not fit in its U-8 bytes (balance_quick)."""
    pages, used = 1, 0
    for c in cell_sizes:
        if used + 2 + c > U - 8:
            pages += 1
            used = 0
        used += 2 + c
    return pages


# --------------------------------------------------------------------------
# Row layouts of the experiment
# --------------------------------------------------------------------------

SCHEMAS = {
    # TinyStore's payload table in the 21 September spikes
    "blob_rowid": ("create table t (id integer primary key, body blob not null) strict",
                   "insert into t (id, body) values (?, ?)", "table"),
    # a payload row with the metadata a block directory carries
    "meta_rowid": ("create table t (id integer primary key, series integer not null, "
                   "start integer not null, count integer not null, body blob not null) strict",
                   "insert into t (id, series, start, count, body) values (?, ?, ?, ?, ?)", "table"),
    # TinyStore's groups table shape: a composite key, WITHOUT ROWID
    "blob_without_rowid": ("create table t (series integer not null, start integer not null, "
                           "body blob not null, primary key (series, start)) strict, without rowid",
                           "insert into t (series, start, body) values (?, ?, ?)", "index"),
}


def row_values(schema, i, body):
    if schema == "blob_rowid":
        return (i, body), [("null",), ("blob", len(body))], i
    if schema == "meta_rowid":
        series, start, count = i // 32, 1_700_000_000_000 + i * 2_400_000, 240
        cols = [("null",), ("int", series), ("int", start), ("int", count), ("blob", len(body))]
        return (i, series, start, count, body), cols, i
    if schema == "blob_without_rowid":
        series, start = 1 + i // 1000, i
        return (series, start, body), [("int", series), ("int", start), ("blob", len(body))], i
    raise ValueError(schema)


def predict(schema, U, sizes, order, bulk=False):
    kind = SCHEMAS[schema][2]
    tree = BTree(U, kind, bulk=bulk)
    cells, payload, record_bytes = [], 0, 0
    keyed = []
    for i, n in zip(order, sizes):
        _, cols, key = row_values(schema, i, b"\0" * n)
        P = record_len(cols)
        payload += n
        record_bytes += P
        if kind == "table":
            c = table_leaf_cell(P, key, U)
        else:
            c = index_leaf_cell(P, U)
        keyed.append((key, c, overflow_pages(P, U, kind == "index")))
    if list(order) != sorted(order):
        return None  # the model covers key-ordered appends only
    for key, c, ov in keyed:
        tree.append(c, key, ov)
        cells.append(c)
    out = tree.census()
    out["page_count"] = 1 + out["pages"]  # page 1 holds the schema
    out["payload_bytes"] = payload
    out["record_bytes"] = record_bytes
    if kind == "table":
        out["greedy_leaves"] = greedy_leaves(cells, U)
    return out


# --------------------------------------------------------------------------
# Measurement: build the file, checkpoint, divide it page by page
# --------------------------------------------------------------------------


def read_varint(b, off):
    v = 0
    for i in range(8):
        x = b[off + i]
        v = (v << 7) | (x & 0x7F)
        if x < 0x80:
            return v, i + 1
    return (v << 8) | b[off + 8], 9


def walk_file(path):
    """Divides a database file by b-tree from the bytes alone, checking on
    every page that header + pointers + cells (sized by the format's rules)
    + free space equal the usable size, and that every overflow chain has
    the length the rules give."""
    with open(path, "rb") as f:
        data = f.read()
    ps = struct.unpack(">H", data[16:18])[0]
    ps = 65536 if ps == 1 else ps
    U = ps - data[20]
    npages = len(data) // ps
    hdr_count = struct.unpack(">I", data[28:32])[0]
    free_count = struct.unpack(">I", data[36:40])[0]
    con = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    roots = con.execute("select name, rootpage from sqlite_schema where rootpage > 0").fetchall()
    con.close()
    roots = [("sqlite_schema", 1)] + roots
    objects, errors = {}, 0
    seen = set()
    for name, root in roots:
        o = {"interior": 0, "leaf": 0, "overflow": 0, "cells": 0, "unused": 0}
        stack = [root]
        while stack:
            pg = stack.pop()
            seen.add(pg)
            base = (pg - 1) * ps
            ho = 100 if pg == 1 else 0
            p = data[base:base + ps]
            t = p[ho]
            interior = t in (2, 5)
            hsize = 12 if interior else 8
            ncell = struct.unpack(">H", p[ho + 3:ho + 5])[0]
            content = struct.unpack(">H", p[ho + 5:ho + 7])[0] or 65536
            frag = p[ho + 7]
            fb = struct.unpack(">H", p[ho + 1:ho + 3])[0]
            freeblocks = 0
            while fb:
                nxt, fsz = struct.unpack(">HH", p[fb:fb + 4])
                freeblocks += fsz
                fb = nxt
            unused = content - (ho + hsize + 2 * ncell) + freeblocks + frag
            o["interior" if interior else "leaf"] += 1
            o["cells"] += ncell
            o["unused"] += unused
            cell_bytes = 0
            for c in range(ncell):
                off = struct.unpack(">H", p[ho + hsize + 2 * c:ho + hsize + 2 * c + 2])[0]
                q = off
                if interior:
                    stack.append(struct.unpack(">I", p[q:q + 4])[0])
                    q += 4
                if t == 5:  # table interior: key only
                    _, ln = read_varint(p, q)
                    cell_bytes += 4 + ln
                    continue
                P, ln = read_varint(p, q)
                q += ln
                if t == 13:
                    _, ln = read_varint(p, q)
                    q += ln
                loc = local_len(P, U, t != 13)
                q += loc
                if loc < P:
                    first = struct.unpack(">I", p[q:q + 4])[0]
                    q += 4
                    chain = 0
                    while first:
                        seen.add(first)
                        chain += 1
                        ob = (first - 1) * ps
                        first = struct.unpack(">I", data[ob:ob + 4])[0]
                    o["overflow"] += chain
                    if chain != overflow_pages(P, U, t != 13):
                        errors += 1
                cell_bytes += max(q - off, 4)
            if interior:
                stack.append(struct.unpack(">I", p[ho + 8:ho + 12])[0])
            if ho + hsize + 2 * ncell + cell_bytes + unused != U:
                errors += 1
        objects[name] = o
    return {"page_size": ps, "usable": U, "file_pages": npages, "header_pages": hdr_count,
            "freelist_header": free_count, "objects": objects, "format_errors": errors,
            "unaccounted_pages": npages - len(seen) - free_count}


def dbstat_census(con):
    try:
        rows = con.execute("select name, pagetype, count(*), sum(unused) from dbstat group by name, pagetype").fetchall()
    except sqlite3.Error:
        return None
    out = {}
    for name, ptype, cnt, unused in rows:
        o = out.setdefault(name, {})
        o[ptype] = cnt
        o["unused_" + ptype] = unused
    return out


def measure(schema, U, sizes, order, workdir, tag, vacuum=False, seed=1):
    create, insert, _ = SCHEMAS[schema]
    path = os.path.join(workdir, f"{tag}.db")
    for suffix in ("", "-wal", "-shm"):
        if os.path.exists(path + suffix):
            os.remove(path + suffix)
    rnd = random.Random(seed)
    con = sqlite3.connect(path, isolation_level=None)
    con.execute(f"pragma page_size={U}")
    con.execute("pragma auto_vacuum=none")
    con.execute("pragma journal_mode=wal")
    con.execute("pragma synchronous=off")
    con.execute(create)
    con.execute("begin")
    con.executemany(insert, (row_values(schema, i, rnd.randbytes(n))[0] for i, n in zip(order, sizes)))
    con.execute("commit")
    if vacuum:
        con.execute("vacuum")
    con.execute("pragma wal_checkpoint(truncate)")
    out = {
        "page_count": con.execute("pragma page_count").fetchone()[0],
        "freelist": con.execute("pragma freelist_count").fetchone()[0],
        "page_size": con.execute("pragma page_size").fetchone()[0],
        "auto_vacuum": con.execute("pragma auto_vacuum").fetchone()[0],
        "dbstat": dbstat_census(con),
    }
    con.close()
    out["walk"] = walk_file(path)
    for suffix in ("", "-wal", "-shm"):
        if os.path.exists(path + suffix):
            os.remove(path + suffix)
    return out


# --------------------------------------------------------------------------
# The experiment
# --------------------------------------------------------------------------


def rows_for(schema, U, n):
    """Enough rows for about 60 leaf pages, at least 300 and at most 20,000."""
    kind = SCHEMAS[schema][2]
    P = record_len(row_values(schema, 5000, b"\0" * n)[1])
    c = table_leaf_cell(P, 5000, U) if kind == "table" else index_leaf_cell(P, U)
    per_page = max(1, (U - 8) // (c + 2))
    return max(300, min(20000, 60 * per_page))


def configs(quick):
    out = []
    pages = [1024, 4096] if quick else [1024, 4096, 8192, 16384]
    for U in pages:
        step = U // (16 if quick else 64)
        for n in range(0, int(2.25 * U) + 1, step):
            out.append(dict(group="sweep", schema="blob_rowid", U=U, n=n))
    for U in ([4096] if quick else [1024, 4096]):
        step = U // (16 if quick else 64)
        for n in range(0, int(2.25 * U) + 1, step):
            out.append(dict(group="without_rowid", schema="blob_without_rowid", U=U, n=n))
            out.append(dict(group="without_rowid_vacuum", schema="blob_without_rowid", U=U, n=n, vacuum=True))
    for n in (0, 16, 100, 560, 1300, 1600, 1780, 2500, 4000, 6000):
        out.append(dict(group="meta", schema="meta_rowid", U=4096, n=n))
    # rows of varying size around a mean, seeded: uniform within +-spread
    for mean, spread in ((300, 100), (560, 120), (1300, 200), (1595, 150), (1780, 150), (3000, 500), (5000, 800)):
        out.append(dict(group="mixed", schema="blob_rowid", U=4096, mean=mean, spread=spread))
        out.append(dict(group="mixed_without_rowid", schema="blob_without_rowid", U=4096, mean=mean, spread=spread))
    # key order the model does not cover: rows written in random rowid order
    for n in (16, 100, 560, 1300, 1780, 3000):
        out.append(dict(group="random_order", schema="blob_rowid", U=4096, n=n, shuffle=True))
    return out


def run(cfg, workdir, idx):
    schema, U = cfg["schema"], cfg["U"]
    if "mean" in cfg:
        rnd = random.Random(1000 + idx)
        N = 3000
        sizes = [rnd.randint(cfg["mean"] - cfg["spread"], cfg["mean"] + cfg["spread"]) for _ in range(N)]
    else:
        N = rows_for(schema, U, cfg["n"])
        sizes = [cfg["n"]] * N
    order = list(range(1, N + 1))
    if cfg.get("shuffle"):
        random.Random(7 + idx).shuffle(order)
    vac = cfg.get("vacuum", False)
    # after VACUUM the transfer rebuilds index b-trees with BTREE_BULKLOAD
    pred = predict(schema, U, sizes, order, bulk=vac and SCHEMAS[schema][2] == "index")
    if pred is None:  # out of the model's domain: predict the same rows appended in order
        pred = predict(schema, U, sizes, sorted(order))
        pred["note"] = "append-order prediction for rows written in random order"
    meas = measure(schema, U, sizes, order, workdir, f"c{idx}", vacuum=vac)
    rec = dict(cfg)
    rec.update({"rows": N, "predicted": pred, "measured": meas})
    return rec


def environment():
    con = sqlite3.connect(":memory:")
    opts = [r[0] for r in con.execute("pragma compile_options")]
    has_dbstat = dbstat_census(con) is not None
    con.close()
    return {
        "python": sys.version.split()[0],
        "sqlite": sqlite3.sqlite_version,
        "sqlite_compile_options": opts,
        "dbstat": has_dbstat,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }


def summarize(records):
    groups = {}
    for r in records:
        g = groups.setdefault(r["group"], {"configs": 0, "exact_page_count": 0, "max_abs_page_diff": 0,
                                           "sum_pred": 0, "sum_meas": 0, "overflow_exact": 0,
                                           "leaf_exact": 0, "format_errors": 0, "dbstat_agrees": 0,
                                           "dbstat_checked": 0})
        p, m = r["predicted"], r["measured"]
        w = m["walk"]["objects"]["t"]
        d = p["page_count"] - m["page_count"]
        g["configs"] += 1
        g["exact_page_count"] += d == 0
        g["max_abs_page_diff"] = max(g["max_abs_page_diff"], abs(d))
        g["sum_pred"] += p["page_count"]
        g["sum_meas"] += m["page_count"]
        g["overflow_exact"] += p["overflow"] == w["overflow"]
        g["leaf_exact"] += p["leaf"] == w["leaf"]
        g["format_errors"] += m["walk"]["format_errors"] + abs(m["walk"]["unaccounted_pages"])
        ds = m["dbstat"]
        if ds is not None:
            g["dbstat_checked"] += 1
            t = ds.get("t", {})
            g["dbstat_agrees"] += (t.get("leaf", 0) == w["leaf"] and t.get("internal", 0) == w["interior"]
                                   and t.get("overflow", 0) == w["overflow"])
    return groups


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--out", required=True, help="directory for results")
    ap.add_argument("--workdir", help="where the temporary databases go (default: a new temp directory)")
    ap.add_argument("--quick", action="store_true", help="a small subset, for a smoke run")
    ap.add_argument("--label", default=sys.platform, help="name of this run's output files")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    env = environment()
    cfgs = configs(a.quick)
    records = []
    with tempfile.TemporaryDirectory(dir=a.workdir) as wd:
        t0 = time.time()
        for i, cfg in enumerate(cfgs):
            records.append(run(cfg, wd, i))
            if (i + 1) % 100 == 0:
                print(f"{i + 1}/{len(cfgs)} configs, {time.time() - t0:.0f} s", flush=True)
    env["configs"] = len(cfgs)
    env["finished_utc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    summary = summarize(records)
    with open(os.path.join(a.out, f"results-{a.label}.jsonl"), "w", encoding="utf-8") as f:
        for r in records:
            f.write(json.dumps(r, sort_keys=True) + "\n")
    with open(os.path.join(a.out, f"summary-{a.label}.json"), "w", encoding="utf-8") as f:
        json.dump({"environment": env, "groups": summary}, f, indent=1, sort_keys=True)
    print(json.dumps({"environment": {k: v for k, v in env.items() if k != "sqlite_compile_options"},
                      "groups": summary}, indent=1, sort_keys=True))


if __name__ == "__main__":
    main()
