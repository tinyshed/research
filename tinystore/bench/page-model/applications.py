#!/usr/bin/env python3
"""The page model applied to rows the TinyStore reports describe.

Prints, for each case, what the model predicts and what SQLite builds for the
same rows, plus the closed-form thresholds page-model.md quotes. Standard
library only; uses page_model.py beside it. Reproduce with:

    python <repo>/tinystore/bench/page-model/applications.py > <repo>/tinystore/reports/data/page-model-2026-11-01/applications.txt
"""

import os
import sqlite3
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import page_model as pm  # noqa: E402


def blob_cell(n, U, rowid=5000, index=False):
    if index:
        P = pm.record_len([("int", 1), ("int", rowid), ("blob", n)])
        return P, pm.index_leaf_cell(P, U)
    P = pm.record_len([("null",), ("blob", n)])
    return P, pm.table_leaf_cell(P, rowid, U)


def rows_per_leaf(n, U):
    _, c = blob_cell(n, U)
    return (U - 8) // (c + 2)


def thresholds(U, max_rows=8):
    """Largest body (bytes) at which r rows of (id integer primary key, body
    blob) still share one leaf page, for r = 1..max_rows (rowid 5000)."""
    out = {}
    n = 0
    for r in range(max_rows, 0, -1):
        while rows_per_leaf(n + 1, U) >= r and n < 4 * U:
            n += 1
        out[r] = n
    return out


def case(title, schema, U, sizes, workdir, tag):
    order = list(range(1, len(sizes) + 1))
    p = pm.predict(schema, U, sizes, order)
    m = pm.measure(schema, U, sizes, order, workdir, tag)
    t = m["walk"]["objects"]["t"]
    payload = sum(sizes)
    print(f"{title}")
    print(f"  rows {len(sizes)}, body bytes {payload}, page size {U}")
    print(f"  predicted: table b-tree {p['leaf']} leaf + {p['interior']} interior + {p['overflow']} overflow pages,"
          f" file {p['page_count']} pages = {p['page_count'] * U} bytes")
    print(f"  measured:  table b-tree {t['leaf']} leaf + {t['interior']} interior + {t['overflow']} overflow pages,"
          f" file {m['page_count']} pages = {m['page_count'] * U} bytes, unused in b-tree pages {t['unused']}")
    print(f"  table b-tree bytes per body byte: {(t['leaf'] + t['interior'] + t['overflow']) * U / payload:.4f}")
    return p, m


def main():
    print(f"python {sys.version.split()[0]}, sqlite {sqlite3.sqlite_version}")
    U = 4096
    print("\n== Closed-form thresholds, (id integer primary key, body blob) strict, rowid 5000")
    for ps in (1024, 4096, 8192, 16384):
        th = thresholds(ps)
        print(f"  page {ps}: largest body for r rows a leaf page: " + ", ".join(f"{r}:{th[r]}" for r in sorted(th)))
    for ps in (1024, 4096, 8192, 16384):
        print(f"  page {ps}: table leaf X = {pm.max_local(ps, False)}, index X = {pm.max_local(ps, True)},"
              f" M = {pm.min_local(ps)}, overflow page content = {ps - 4}")

    with tempfile.TemporaryDirectory() as wd:
        print("\n== A1. measurements.md, 'The adaptive codec against the one it replaced': noisy float payloads")
        print("   240 samples a block; 7.415 and 6.644 payload B/sample are 1779.6 and 1594.6 bytes a block")
        for n in (1780, 1595):
            case(f"  body {n} bytes, 10,000 rows", "blob_rowid", U, [n] * 10000, wd, f"a1_{n}")
        th = thresholds(U)
        print(f"  two rows a page for bodies {th[3] + 1}..{th[2]} bytes, three for {th[4] + 1}..{th[3]}")

        print("\n== A2. measurements.md, 'The whole file...': floats closed at about 1300 bytes, three rows a page")
        case("  body 1300 bytes, 10,000 rows", "blob_rowid", U, [1300] * 10000, wd, "a2_1300")

        print("\n== A3. grid-2026-09-21: payload rows average 560 bytes, 580 rows")
        for n in (560, 583):
            case(f"  body {n} bytes, 580 rows", "blob_rowid", U, [n] * 580, wd, f"a3_{n}")
        print(f"  seven rows a page up to {th[7]} bytes, six up to {th[6]}")
        t1 = thresholds(1024)
        print(f"  at 1024-byte pages: two rows a page up to {t1[2]} bytes, one up to {t1[1]}")

        print("\n== A4. metrics-layout-2026-10-09: Alibaba's WITHOUT ROWID groups and their overflow pages")
        X, M = pm.max_local(U, True), pm.min_local(U)
        print(f"  index b-tree: a row spills past P = {X} bytes and keeps M = {M} on its page;"
              f" one overflow page holds rows up to P = {M + U - 4}")
        pages, content, unused = 465, 312228, 1590552
        print(f"  465 overflow pages x 4096 = {pages * U}; content {content} + unused {unused} + 4-byte links {pages * 4}"
              f" = {content + unused + pages * 4}")
        print(f"  mean overflowing row if every chain is one page: P = {M} + {content}/{pages} = {M + content / pages:.1f}")
        for n in (980, 1000, 1100, 1160):
            P, c = blob_cell(n, U, index=True)
            print(f"  WITHOUT ROWID body {n}: P {P}, local {pm.local_len(P, U, True)}, cell {c},"
                  f" overflow pages {pm.overflow_pages(P, U, True)}; as a rowid table: overflow pages"
                  f" {pm.overflow_pages(pm.record_len([('null',), ('blob', n)]), U, False)}")
        for n in (980, 1100):
            for schema in ("blob_without_rowid", "blob_rowid"):
                case(f"  {schema}, body {n} bytes, 2,000 rows", schema, U, [n] * 2000, wd, f"a4_{schema}_{n}")

        print("\n== A5. metrics-payload-2026-10-09: the nonsparse fixture's payload b-tree, unpacked and packed by eight")
        print("   3,295,691 sealed body bytes over 2,048 blocks is 1,609 bytes a body (metrics-layout baseline table)")
        case("  rowid-baseline: 2,048 bodies of 1,609 bytes", "blob_rowid", U, [1609] * 2048, wd, "a5_unpacked")
        case("  count8: 256 packs of 8 x 1,609 = 12,872 bytes", "blob_rowid", U, [12872] * 256, wd, "a5_count8")
        print("   reported: rowid-baseline payload b-tree 4,206,592 bytes (1,027 pages), 0 overflow pages;"
              " rowid-compact-count8 3,325,952 bytes (812 pages), 768 overflow pages")


if __name__ == "__main__":
    main()
