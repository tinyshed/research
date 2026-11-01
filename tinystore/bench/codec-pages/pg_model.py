"""The page model extended to PostgreSQL's heap and TOAST, checked on every load.

    python3 -I pg_model.py <pg out dir> <results.jsonl> <dataset> [<dataset> ...]

For each load pg_load.sh made (a bodies file, storage EXTERNAL or EXTENDED),
from the rows' stored sizes (`pg_column_size(body)`, and whether the value
went out of line) and PostgreSQL 18's rules for 8 KiB pages:

  heap tuple    t_len = 24 (header, MAXALIGN(23), no null bitmap)
                        + 4 (the integer id)
                        + the body's datum: 1 + n bytes up to 127 (short
                          varlena), else 4 + n (no padding: offset 4 is
                          int-aligned); inline compressed, its stored size;
                          out of line, an 18-byte TOAST pointer
  on a page     MAXALIGN(t_len) + a 4-byte line pointer, after a 24-byte
                page header; a tuple goes on the page while
                pd_upper - pd_lower >= MAXALIGN(t_len) + 4 (heap_multi_insert
                and PageGetHeapFreeSpace), at most 291 a page
  out of line   when t_len > TOAST_TUPLE_THRESHOLD = 2032 (a quarter page);
                the value is cut into chunks of TOAST_MAX_CHUNK_SIZE = 1996
                bytes, each a TOAST tuple of t_len = 24 + 4 (chunk_id)
                + 4 (chunk_seq) + 4 + l: a TOAST table's chunk_data is
                PLAIN, so it keeps a 4-byte header even when short

It checks each heap and TOAST page's header (pageinspect's lower and upper)
against the tuples the page holds, predicts the pages a sequential fill
takes, and reports the relation sizes PostgreSQL measured beside it,
including the pages its bulk extension allocated and left empty.
"""

import json
import os
import sys
from collections import defaultdict

PAGE, HEADER, LP, MAXALIGN, MAX_TUPLES = 8192, 24, 4, 8, 291
THRESHOLD, CHUNK = 2032, 1996


def align(n):
    return (n + MAXALIGN - 1) // MAXALIGN * MAXALIGN


def datum_inline(n):
    return 1 + n if n + 1 <= 127 else 4 + n


def chunk_tlen(l):
    # chunk_data's storage is PLAIN in a TOAST table, so it keeps its 4-byte header even when short
    return 24 + 4 + 4 + 4 + l


def greedy(tlens):
    """Pages and each tuple's page for a sequential fill."""
    pages, free, count, where = 0, 0, MAX_TUPLES, []
    for t in tlens:
        need = align(t) + LP
        if pages == 0 or free < need or count >= MAX_TUPLES:
            pages += 1
            free, count = PAGE - HEADER, 0
        free -= need
        count += 1
        where.append(pages - 1)
    return pages, where


def read_tsv(path):
    with open(path) as f:
        return [line.rstrip("\n").split("\t") for line in f if line.strip()]


def page_check(pages, items):
    """items: page -> [t_len]. Every page with tuples must have
    lower = 24 + 4k and upper = 8192 - sum MAXALIGN(t_len)."""
    bad, empty, used = 0, 0, 0
    for blk, (lower, upper) in pages.items():
        tl = items.get(blk, [])
        if not tl:
            empty += 1
            if not (lower == 0 and upper == 0) and not (lower == HEADER and upper == PAGE):
                bad += 1
            continue
        used += 1
        if lower != HEADER + LP * len(tl) or upper != PAGE - sum(align(t) for t in tl):
            bad += 1
    bad += sum(1 for b in items if b not in pages)
    return bad, empty, used


def one(out_dir, name, storage, sizes):
    rows = read_tsv(os.path.join(out_dir, f"{name}.{storage}.rows"))
    chunks = read_tsv(os.path.join(out_dir, f"{name}.{storage}.chunks"))
    pages = defaultdict(dict)
    for kind, blk, lower, upper, _ in read_tsv(os.path.join(out_dir, f"{name}.{storage}.pages")):
        pages[kind][int(blk)] = (int(lower), int(upper))
    heap_items, tlens, external_ok, ext_sizes = defaultdict(list), [], 0, {}
    for i, (rid, colsize, comp, ext, blk) in enumerate(rows):
        colsize, ext = int(colsize), ext == "t"
        t = 24 + 4 + (18 if ext else colsize)
        tlens.append(t)
        heap_items[int(blk)].append(t)
        # the threshold rule: out of line exactly when the tuple with this datum inline exceeds 2032;
        # an inline datum is its stored size, an out-of-line one its stored bytes and a 4-byte header
        inline_t = 24 + 4 + (colsize + 4 if ext else colsize)
        external_ok += (inline_t > THRESHOLD) == ext
        if ext:
            ext_sizes[int(rid)] = colsize
    toast_items, chunk_ok, nchunks = defaultdict(list), 0, defaultdict(int)
    by_value = defaultdict(list)
    for cid, seq, length, blk in chunks:
        length = int(length)
        toast_items[int(blk)].append(chunk_tlen(length))
        by_value[int(cid)].append(length)
    # each out-of-line value must be ceil(size / 1996) chunks, all full but the last
    predicted_chunks = []
    for rid in sorted(ext_sizes):
        s = ext_sizes[rid]
        k, r = divmod(s, CHUNK)
        predicted_chunks.append([CHUNK] * k + ([r] if r else []))
    measured_chunks = [by_value[c] for c in sorted(by_value)]
    chunk_ok = predicted_chunks == measured_chunks
    heap_bad, heap_empty, heap_used = page_check(pages["heap"], heap_items)
    toast_bad, toast_empty, toast_used = page_check(pages["toast"], toast_items)
    heap_pred, heap_where = greedy(tlens)
    toast_tl = [chunk_tlen(l) for c in predicted_chunks for l in c]
    toast_pred, _ = greedy(toast_tl)
    measured_where = [int(r[4]) for r in rows]
    s = dict(zip(("heap_main", "heap_fsm", "heap_vm", "toast_main", "toast_fsm", "toast_vm", "toast_index",
                  "pkey_index", "total", "heap_pages_used", "toast_pages_used", "toast_chunks", "rows"),
                 map(int, sizes)))
    return {
        "name": name, "storage": storage, "sizes": s,
        "rows_external": len(ext_sizes), "rows_compressed": sum(1 for r in rows if r[2]),
        "external_rule_holds": external_ok == len(rows),
        "chunks_as_predicted": chunk_ok,
        "heap_pages_bad": heap_bad, "heap_pages_empty": heap_empty, "heap_pages_with_tuples": heap_used,
        "toast_pages_bad": toast_bad, "toast_pages_empty": toast_empty, "toast_pages_with_tuples": toast_used,
        "heap_pages_predicted": heap_pred, "heap_placement_exact": heap_where == measured_where,
        "toast_pages_predicted": toast_pred,
        "stored_value_bytes": sum(int(r[1]) for r in rows),
    }


def main():
    out_dir, results = sys.argv[1], sys.argv[2]
    with open(results, "a") as f:
        for ds in sys.argv[3:]:
            for line in read_tsv(os.path.join(out_dir, f"{ds}.sizes")):
                name, storage, sizes = line[0], line[1], line[2:]
                _, codec, L = name.split("__")
                rec = one(out_dir, name, storage, sizes)
                rec.update({"dataset": ds, "codec": codec, "block": int(L[1:])})
                f.write(json.dumps(rec, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
