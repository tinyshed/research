# A page model for SQLite files — 1 November 2026

**Its harness, [bench/page-model](../bench/page-model/page_model.py), is at research commit `b146e7d`; the codec round [codec-pages-2026-11-01](codec-pages-2026-11-01.md) imports it unchanged.** Every figure is a page count, which no other job on the machine moves; nothing here was timed.

A codec result is reported as payload bytes. SQLite stores each payload in a
cell on a fixed-size page, so the file the payload lands in moves in steps,
and a step can be larger than the payload change or absent altogether. This
note derives those steps from SQLite's documented file format and its b-tree
code, predicts the pages a file takes from its rows' payload sizes, and checks
the prediction on 1,190 files built by SQLite itself on two operating systems.

**Result.** For every file in the model's domain — a fresh file, rows written
in key order, no deletes — the model predicted the page count exactly: 1,184
of 1,184 files, leaf, interior and overflow pages each exact, on SQLite 3.50.4
(Windows) and 3.46.1 (Linux) alike. Six files written in random rowid order,
outside the domain, took 0–13 % more pages than the append-order prediction.
A second round, [codec-pages-2026-11-01](codec-pages-2026-11-01.md), fed it the
stored row sizes of 24,576 more files, the blocks of seven published codecs
and TinyStore's on 32 datasets at 1, 4, 8 and 16 KiB: every one exact.
Fed only the mean body size a report gives, the model reproduces to the page
the two payload b-trees of
[metrics-payload-2026-10-09](metrics-payload-2026-10-09.md)'s
nonsparse fixture, and it explains three earlier observations in
[measurements.md](../measurements.md) and
[grid-2026-09-21](grid-2026-09-21.md).

## Sources of the rules

- *Database File Format*, <https://www.sqlite.org/fileformat2.html>, opened on
  2026-11-01 (HTTP Last-Modified 2026-10-28): section 1.6 "B-tree Pages"
  (page header 8 bytes on a leaf and 12 on an interior page, 2-byte cell
  pointers, cell layouts, and the spill rule below), 1.7 "Cell Payload
  Overflow Pages" (a 4-byte link, then U−4 bytes of content), 2.1 "Record
  Format" (header of varints, serial types; a BLOB of n bytes is type 12+2n),
  2.4 "Representation of WITHOUT ROWID Tables" (an index b-tree keyed by the
  whole record).
- SQLite's source, official GitHub mirror `sqlite/sqlite` at
  `cb62289207c58596ee8d63b853b3b8a5db7ed323` (VERSION 3.55.0, 2026-10-31):
  `src/btree.c` lines 3474–3477 (the local-size limits), `balance()` at
  9179 (a page is rebalanced if it overflows or is less than a third full)
  and 9220–9242 (when `balance_quick` applies), `balance_quick` at 8040,
  `balance_nonroot` at 8278 (sibling choice 8344–8357, packing 8576–8640,
  evening from the right 8641–8671); `src/insert.c` line 3414 (the transfer
  that VACUUM uses opens index b-trees with `OPFLAG_BULKCSR`, which is
  `BTREE_BULKLOAD`).

The file format fixes how many bytes a row costs on a page and in overflow.
It does not fix how full pages are left; that is the b-tree code's choice, and
the model takes it from the source above.

## The model

Let U be the usable page size (page size less reserved bytes, 0 here) and P a
row's payload, its record: a header of varints (its own length, then one serial
type a column) and the column bytes. An `integer primary key` is stored as a
NULL in the record and as the cell's key.

**What a row costs (format, section 1.6).** With X the most a cell keeps on its
page and M the least a spilling cell keeps,

```text
X = U − 35                      table b-tree leaf (a rowid table)
X = (U − 12)·64/255 − 23        index b-tree (an index, or a WITHOUT ROWID table)
M = (U − 12)·32/255 − 23        both
K = M + (P − M) mod (U − 4)

local = P   if P ≤ X;   K if P > X and K ≤ X;   M otherwise
overflow pages = ⌈(P − local) / (U − 4)⌉
cell = varint(P) [+ varint(rowid), table leaf] + local [+ 4, if it spills]
       [+ 4-byte child pointer, interior index cell];  at least 4 bytes
```

| Page size | X, table leaf | X, index | M | Overflow content a page |
|---:|---:|---:|---:|---:|
| 1,024 | 989 | 230 | 103 | 1,020 |
| 4,096 | 4,061 | 1,002 | 489 | 4,092 |
| 8,192 | 8,157 | 2,030 | 1,003 | 8,188 |
| 16,384 | 16,349 | 4,086 | 2,031 | 16,380 |

The rule for K is why a long payload wastes little: when K ≤ X the local part
takes the remainder and every overflow page is full. A payload just past X is
the expensive case: K exceeds X, the cell keeps M bytes on its page, and one
overflow page holds the rest, mostly empty.

**What a page holds (format, 1.6).** A leaf holds cells while
8 + Σ(2 + cell) ≤ U; an interior page while 12 + Σ(2 + cell) ≤ U. A table
interior cell is a 4-byte child pointer and the varint of a rowid; an index
interior cell is the leaf cell with a child pointer in front.

**How full pages are left (btree.c).**

- Rows appended to a rowid table in rowid order meet `balance_quick`: the
  cell that does not fit starts a new page on its own. Every leaf is filled
  greedily, so for a row layout of constant cell size c,

  ```text
  rows a leaf = ⌊(U − 8) / (c + 2)⌋        leaves = ⌈N / rows a leaf⌉
  ```

- Interior pages, and every page of an index b-tree, are split by
  `balance_nonroot`: the right-most page and up to two left siblings are
  repacked left first, then evened out from the right. VACUUM rebuilds index
  b-trees with `BTREE_BULKLOAD`, which takes one left sibling and skips the
  evening, leaving pages full. A page less than a third full is rebalanced
  too, and the root deepens or shallows.

These last rules have no short closed form for cells of varying size, so
[page_model.py](../bench/page-model/page_model.py) simulates them cell by
cell (class `BTree`, about 200 lines ported from the functions named above)
and counts the pages: file pages = 1 (page 1, the schema) + each b-tree's
leaf, interior and overflow pages + the freelist. For rowid tables the
simulation's leaf count equalled the greedy closed form in all 597 files where
both apply.

**Largest body that still fits r rows on a leaf**, for TinyStore's payload
table `(id integer primary key, body blob not null) strict`, rowid 5000:

| Page size | r = 1, unspilled | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1,024 | 985 | 498 | 328 | 244 | 193 | 159 | 135 | 118 |
| 4,096 | 4,057 | 2,034 | 1,352 | 1,012 | 807 | 671 | 574 | 501 |
| 8,192 | 8,153 | 4,082 | 2,718 | 2,036 | 1,626 | 1,354 | 1,159 | 1,013 |
| 16,384 | 16,344 | 8,178 | 5,448 | 4,084 | 3,265 | 2,719 | 2,329 | 2,037 |

A body above the first column spills; its local part is then small enough
that several such rows share a leaf again.

## The experiment

[page_model.py](../bench/page-model/page_model.py) builds each file with
Python's `sqlite3`: `page_size` set, `auto_vacuum=none`, WAL, all rows in one
transaction, then `wal_checkpoint(truncate)`; it reads `page_count` and
`freelist_count`, and divides the file by walking every b-tree from the bytes
alone. On every page the walk checks that header + cell pointers + cells
(sized by the rules above) + free space (from the page header's freeblocks and
fragment count) equal U, and that every overflow chain has the length the
rules give; where the build has `dbstat`, its per-object page counts are
compared with the walk. Bodies are seeded random bytes; SQLite does not compress,
so only their lengths matter.

| Group | Layout | Page sizes | Files | Page count exact | Leaf, overflow exact | Pages, predicted / measured |
|---|---|---|---:|---:|---:|---:|
| sweep | payload table, rowid; bodies 0 to 2.25 U in steps of U/64 | 1, 4, 8, 16 KiB | 580 | 580 | 580, 580 | 246,362 / 246,362 |
| without_rowid | `(series, start, body)` WITHOUT ROWID, same sweep | 1, 4 KiB | 290 | 290 | 290, 290 | 214,133 / 214,133 |
| without_rowid_vacuum | the same, after VACUUM | 1, 4 KiB | 290 | 290 | 290, 290 | 212,431 / 212,431 |
| meta | rowid, three integer columns and a body | 4 KiB | 10 | 10 | 10, 10 | 1,708 / 1,708 |
| mixed | rowid, 3,000 bodies uniform in mean ± spread | 4 KiB | 7 | 7 | 7, 7 | 11,646 / 11,646 |
| mixed_without_rowid | the same, WITHOUT ROWID | 4 KiB | 7 | 7 | 7, 7 | 19,224 / 19,224 |
| random_order | rowid, rows in random rowid order (outside the model) | 4 KiB | 6 | 1 | 1, 6 | 742 / 795 |

Rows a file: enough for about 60 leaves, at least 300 and at most 20,000.
Every walk found zero format errors and no unaccounted page; `dbstat` agreed
with the walk on all 1,190 Linux files; the two platforms produced identical
page counts and identical per-object divisions for every file.

| Run | Python | SQLite | dbstat | Files | Wall time |
|---|---|---|---|---:|---:|
| Windows 11 Pro 10.0.26300, native | 3.13.15 (MSC v.1944, AMD64) | 3.50.4 | not compiled in | 1,190 | 73 s |
| Ubuntu 26.04 under WSL2, Linux 6.18.33.2-microsoft-standard-WSL2 | 3.14.4 (GCC 15.2.0) | 3.46.1 | yes | 1,190 | 34 s |

Machine: AMD Ryzen 7 7700, 31.1 GiB, the one the reports name. Neither run is
in a container; this is a deterministic layout measurement with no timing in
its result, and the identical output of two SQLite versions on two systems is
its check. The wall times are not measurements. Raw output, one JSON line a
file with the prediction, `pragma` readings, the walk and `dbstat`:
[results-windows.jsonl](data/page-model-2026-11-01/results-windows.jsonl),
[results-linux.jsonl](data/page-model-2026-11-01/results-linux.jsonl),
[summary-windows.json](data/page-model-2026-11-01/summary-windows.json),
[summary-linux.json](data/page-model-2026-11-01/summary-linux.json), and the runs'
consoles [run-windows.txt](data/page-model-2026-11-01/run-windows.txt),
[run-linux.txt](data/page-model-2026-11-01/run-linux.txt).

```sh
python -I <repo>/tinystore/bench/page-model/page_model.py --out <repo>/tinystore/reports/data/page-model-2026-11-01 --label windows
python3 -I <repo>/tinystore/bench/page-model/page_model.py --out <repo>/tinystore/reports/data/page-model-2026-11-01 --label linux
python -I <repo>/tinystore/bench/page-model/applications.py > <repo>/tinystore/reports/data/page-model-2026-11-01/applications.txt
```

## What the model shows

**The file is a staircase over the payload.** The payload table at 4 KiB pages,
measured (the [sweep](data/page-model-2026-11-01/results-windows.jsonl), table b-tree
pages only):

| Body bytes | Rows a leaf | Pages per 1,000 rows | Table bytes per body byte |
|---:|---:|---:|---:|
| 512 | 8 | 145.2 | 1.162 |
| 640 | 6 | 169.4 | 1.084 |
| 1,024 | 3 | 336.7 | 1.347 |
| 1,344 | 3 | 336.7 | 1.026 |
| 1,408 | 2 | 503.3 | 1.464 |
| 1,792 | 2 | 503.3 | 1.150 |
| 2,048 | 1 | 1,003.3 | 2.007 |
| 4,032 | 1 | 1,003.3 | 1.019 |
| 4,096 | 8, each + 1 overflow page | 1,127.1 | 1.127 |
| 8,192 | 8, each + 2 overflow pages | 2,127.1 | 1.064 |

A codec that shrinks a 1,792-byte body by 21 % to 1,408 saves no page; one
that shrinks a 1,408-byte body by 4.5 % to 1,344 removes a third of the leaves.
Just past a step the file holds twice the payload (2,048-byte bodies, one a
page); just below it, 2–3 % more than the payload.

**WITHOUT ROWID spills four times sooner.** An index b-tree keeps at most
X = 1,002 bytes a cell at 4 KiB, against a rowid table's 4,061. A WITHOUT ROWID
row of payload 1,003 to 4,581 bytes keeps 489 on its page and spills the rest
into one overflow page of its own. 2,000 rows of 980-byte bodies take 505
pages; 1,100-byte bodies, 12 % more payload, take 2,287, of which 2,000 are
overflow pages mostly empty. As a rowid table the same rows take 502 and 671
([applications.txt](data/page-model-2026-11-01/applications.txt), A4).

**How the rows arrived changes the fill.** WITHOUT ROWID tables written in key
order took 10.5 % more b-tree pages (leaf and interior) than the same tables
after VACUUM over the 4 KiB sweep and 8.2 % over the 1 KiB one, up to 14 % for
one size: the evening in `balance_nonroot` leaves
pages part-empty, the bulk rebuild does not. Rowid tables written in random
rowid order took up to 13 % more pages than appended ones (70 against 62 for
16-byte bodies, 170 against 152 for 1,780-byte ones; equal at one row a page).

## Codec bodies, PostgreSQL and layouts (codec-pages-2026-11-01)

[codec-pages-2026-11-01](codec-pages-2026-11-01.md) ran the model, unchanged and imported
by its harness, on the files its hypotheses needed:

- **Codec bodies in SQLite** (H1.2): each block of Gorilla, Chimp, Chimp128,
  Elf, ALP, zstd -3, zstd -19 and TinyStore's codec, 24 block lengths from 120
  to 8,192 values, on ALP's 30 datasets, TSBS long and NAB, one row of the
  payload table each, at four page sizes. 24,576 files, 66,696,672 rows,
  32,974,878 pages, SQLite 3.46.1: page count, leaf, interior and overflow
  pages exact in every file, `dbstat` and the walk agreeing in every file. Rows
  per file from 1 to 218,160; bodies of very different spreads, which the
  earlier groups only sampled as uniform bands.
- **PostgreSQL 18's heap and TOAST** (H1.4), 8 KiB, the same rows by `COPY`,
  storage EXTERNAL and EXTENDED, 1,536 loads. The format half extends exactly:
  a heap tuple is 24 bytes of header, 4 of the id and the body's datum (1 + n
  up to 127 bytes, else 4 + n; an 18-byte pointer out of line), MAXALIGNed,
  with a 4-byte line pointer after a 24-byte page header; a value goes out of
  line when its tuple would pass 2,032 bytes, in chunks of 1,996, whose tuples
  are 36 + l bytes because a TOAST table's `chunk_data` is PLAIN and keeps a
  4-byte header. Every heap and TOAST page's `lower` and `upper` matched its
  tuples on all 1,536 loads. The fill half does not, yet: a sequential fill
  gives the pages that hold tuples exactly in 1,048 and 1,039 loads (heap,
  TOAST) and 1.8 % and 2.8 % too many in total, because the free-space map
  sends later small tuples back to earlier pages; and `COPY`'s bulk extension
  allocates up to 64 pages at a time, leaving 6.3 % of the heap's pages empty
  here, which the file counts and a model of tuples does not.
- **TinyStore's groups table** (H1.5): from TinyStore's own TSBS file, built
  at `e307c48`, the model gives 127 pages as WITHOUT ROWID, 121 as a rowid
  table and 11 for its UNIQUE index, the rebuilt pages exactly and the
  metrics-layout round's +20,480 bytes to the byte. From that round's
  histograms alone, which carry no order, overflow pages come out exactly or
  within three pages and leaves within the range of 25 random orders for
  every WITHOUT ROWID variant, but 7 to 30 pages under four rowid variants:
  a greedy fill depends on the order the histogram drops.

The domain is unchanged: everything inside it was exact. The two misses the
round found in SQLite sit outside it: the `inline 0` variant's payloads
table, which the metrics-layout harness fills out of key order (776 and 884
pages measured against 749–757 and 807–809 predicted from histograms), and an
index filled out of key order, `sqlite_autoindex_series_1`, 28 pages in
SQLite 3.46.1 against 27 in 3.53.4.

## Against the reports

The cases are in [applications.py](../bench/page-model/applications.py); each builds
the rows with SQLite as well as predicting them, and every prediction below
matched its own SQLite file
([applications.txt](data/page-model-2026-11-01/applications.txt)).

| Report and what it says | What the model says | Verdict |
|---|---|---|
| [measurements.md](../measurements.md), "The adaptive codec against the one it replaced": the noisy payload shrank from 7.415 to 6.644 B/sample, 9.124 B/sample in the file both times, "both sizes still occupy the same number of SQLite pages" | 240 samples a block are 1,779.6 and 1,594.6 bytes a body; two rows a 4 KiB leaf for bodies of 1,353–2,034 bytes; 10,000 rows take 5,000 leaves and 12 interior pages either way. With bodies spread ±150 bytes around either mean, 3,000 rows take 1,505 pages both times | Predicted |
| measurements.md, "The whole file, after the night of 21 September": float blocks closed "at about 1300 bytes, where three rows fill a 4 KiB page instead of two" | three rows a leaf for 1,013–1,352-byte bodies; 10,000 of 1,300 bytes take 3,334 leaves against 5,000 at 1,780 | Predicted; the band is a property of 4 KiB pages and this schema, which is why [review-2026-09-21](review-2026-09-21.md) found the 1,300-byte target not universal |
| [grid-2026-09-21](grid-2026-09-21.md): payload 3.9 % smaller, file 9.6 % smaller, "the smaller rows crossed a step in how many fit on a 4096-byte page"; rows average 560 bytes, 580 of them | the step from six to seven rows a leaf is at 574/575 bytes; the two runs' mean rows, about 583 (560 + 13,100/580) and 560, lie on either side. As uniform rows they take 98 and 84 payload-table pages | Mechanism predicted; the uniform model gives 14 pages where the whole file moved 11 and the payload object measured 88 pages, because the rows vary. Not an exact prediction |
| [metrics-layout-2026-10-09](metrics-layout-2026-10-09.md): Alibaba's WITHOUT ROWID groups own 1,904,640 bytes of overflow pages, 312,228 of them payload and 1,590,552 unused; rowid groups have none | 465 pages × 4,096 = 312,228 + 1,590,552 + 465 × 4-byte links, exactly the format's accounting. Directory rows above 1,002 bytes spill at 4 KiB in an index b-tree and only above 4,061 in a rowid table; a row of up to 4,581 bytes spills into one page, so the 465 pages hold rows averaging 489 + 312,228/465 = 1,160.5 bytes if every chain is one page | Mechanism and accounting predicted; the 1,449,984-byte saving needs the rows' sizes, which the report does not list |
| [metrics-payload-2026-10-09](metrics-payload-2026-10-09.md), nonsparse fixture: rowid-baseline payload b-tree 4,206,592 bytes with no overflow; rowid-compact-count8 3,325,952 bytes with 768 overflow pages | from the mean body only, 3,295,691 bytes over 2,048 blocks ([metrics-layout](metrics-layout-2026-10-09.md)'s baseline table) = 1,609: 2,048 bodies take 1,024 leaves + 3 interior = 1,027 pages = 4,206,592 bytes; 256 packs of 8 × 1,609 take 43 leaves + 1 interior + 768 overflow = 812 pages = 3,325,952 bytes | Both equal to the byte. The real bodies vary and packs carry a header, so this shows the result is insensitive to those details within these bands, not that the rows are uniform |

What the model does not do: it predicts pages from row sizes, so where a report
gives no row sizes — the release round's 18.4 MiB of payload in 20.2 MiB of
sample pages ([release-2026-10-23](release-2026-10-23.md),
metrics), or TSBS's 0.5764 B/sample of envelope beyond a 0.5775 B/sample payload
([tsbs-2026-09-21](tsbs-2026-09-21.md)) — it names the
mechanisms but predicts no number.

## Domain and limits

- **Inside:** a fresh file; rows inserted in increasing key order by one
  writer; no update, delete or overwrite; `auto_vacuum=none`; no reserved bytes;
  page 1 holding only the schema; one transaction a file; and, as a separate
  case, the file after VACUUM.
- **Measured outside, and missed:** random rowid order, 0–13 % more pages than
  the append prediction.
- **Measured beside it** (codec-pages-2026-11-01): PostgreSQL's heap and
  TOAST, exact in every page's bytes, approximate in their fill.
- **Not measured:** secondary indexes filled row by row out of key order (one
  measured in passing, above); updates that rewrite
  a row in place or move it (TinyStore's heads, whose free pages the release
  round found holding 1.9 MiB of pages for 0.3 MiB of rows); deletes, the
  freelist and its reuse; commits of one row each; text columns and REAL
  values; reserved bytes (checksum or encryption extensions); page 1 tables;
  SQLite versions other than 3.46.1 and 3.50.4.
- The format's rules are frozen by its compatibility promise; the fill rules are
  implementation and could change in a later SQLite. They were read from a
  3.55.0 development commit and matched 3.46.1 and 3.50.4.
- [open-questions.md](../open-questions.md) asked for a page
  sweep that finds "where the packing cliffs are between one payload size and
  the next". For the payload table and the WITHOUT ROWID group shape this
  experiment answers it: the cliffs are the table above, computed rather than
  swept, and the sweep confirms them.
