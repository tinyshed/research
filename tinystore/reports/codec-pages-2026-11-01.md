# Codecs measured in the file, not the payload — 1 November 2026

**The harness of this round, [bench/codec-pages](../bench/codec-pages/README.md),
is at research commit `9f7ed11`.** Every figure below
is a byte or page count, which no other job on the machine moves; nothing
here was timed.

A ranking of TinyStore's paper candidates, kept outside this repository, puts first a paper on
codec results measured in the file rather than the payload, with
[an exact page model](page-model-2026-11-01.md) of SQLite's table
b-trees as its tool, and lists five hypotheses for it. This round tests them
with the published codecs' own code: Gorilla, Chimp, Chimp128 and Elf from
their authors' Java, ALP from its authors' C++, zstd at levels 3 and 19 per
block, and TinyStore's adaptive codec, on ALP's 30 public datasets, TSBS long
and NAB. Each block is one row of `(id integer primary key, body blob)`.

| Question | Answer | What follows |
|---|---|---|
| H1.2: does the page model predict every such file from its row sizes? | Yes, all of them. 24,576 SQLite files, 66,696,672 rows, 32,974,878 pages: page count, leaf, interior and overflow pages each exact in every file, and `dbstat` agrees with the model's own walk of the bytes in every file | The model is the tool the paper claims; no miss to report in its domain |
| H1.1: do codec rankings reverse once payloads sit in pages? | Not falsified, but rarely by much. Of 384 cells (dataset × block length × page size), Kendall's τ-b between payload order and file order is below 1 in 210, with at least one strictly reversed pair of codecs in 90; 114 of 10,752 pairs reverse and 824 more become ties; the best codec changes in 18 cells. τ-b averages 0.937, at least 0.5. A reversed pair's payloads differ by 1.2 % at the median and by up to 38 % | The rank reversal is real and small. What is large is how much of the file each codec's rows waste: within one cell the file-to-payload ratio of the worst codec is 1.13–1.21 times the best's at the median, up to 2.37 times |
| H1.3: does the block length minimising the file differ from the one minimising the payload? | Yes, in 543 of 1,024 curves (dataset × codec × page size, 24 lengths from 120 to 8,192): the payload's best length leaves a file 5.5 % larger than the file's best at the median, more than 5 % larger in 284 curves, up to 74.9 % on a dataset of 100,000 values or more | Choose a block length for the file; for TinyStore, whose payload is smallest at its 240-value cap, the file prefers several blocks a row |
| H1.4: is it a page-store effect? | Yes. PostgreSQL 18 on the same rows reorders more than SQLite at 8 KiB: τ-b below 1 in 84 and 85 of 96 cells (EXTERNAL, EXTENDED), 81 and 93 reversed pairs against SQLite's 28, the best codec changing in 11 cells. A flat file of length-prefixed bodies keeps the payload order in all 96: τ-b 1, no reversal, no tie | The effect belongs to pages, and PostgreSQL adds two of its own: TOAST's quarter-page threshold and the pages bulk extension leaves empty |
| H1.4: does the model extend to PostgreSQL? | The space half exactly, the placement half nearly. Every heap and TOAST page of all 1,536 loads holds exactly the bytes the tuple rules give; every row went out of line exactly when the 2,032-byte threshold says; every value's chunks are as predicted. A sequential fill predicts the heap's used pages exactly in 1,048 loads and TOAST's in 1,039, 1.8 % and 2.8 % above in total, where the free-space map back-fills earlier pages | Model the free-space map's search to close the gap; count bulk extension's empty pages, 6.3 % of the heap here, as part of the file |
| H1.5: can a layout be chosen offline from row sizes? | For rowid against WITHOUT ROWID, yes on both corpora. TSBS from its rows: +20,480 bytes, +0.45 %, to the page, as metrics-layout measured. Alibaba from that round's own histograms: −21.6 % to −22.2 % against −21.40 % measured. The inline threshold: the right pick in three of four layouts, wrong for Alibaba's rowid table (inline 64 predicted, 32 measured) | Use the model to choose; give it rows, not histograms, where order matters |
| Was Alibaba measured? | No. Its archive is reachable only from Alibaba's Beijing storage, which the local network blocks and the proxy cannot reach; every attempt from 00:16 to 03:58 UTC fetched nothing | Fetch it on another network, then run it through `run.sh` and `h15.py` |

## Environment and reproduction

Everything ran on 1 November 2026 on the AMD Ryzen 7 7700 (8 cores, 16
logical processors) of the earlier rounds, in Docker Desktop 29.6.2 on WSL2,
Linux 6.18.33.2-microsoft-standard-WSL2, 16 CPUs and 15.2 GiB visible to
the containers. Sizes are deterministic: the Java and the ALP encoders were
each rebuilt once from empty volumes during their checks and wrote
byte-identical bodies of the harness's test file.

| What | Version |
|---|---|
| TinyStore | `e307c48a40126aad0e2873b6bf3aaedef8115483`, the submodule's pinned commit (`v0.1.0-rc.5`), klauspost/compress v1.20.1 |
| Gorilla, Chimp, Chimp128 | panagiotisl/chimp `320d397157c7e0696b3c64dc1711fc17a3add3da` |
| Elf | Spatio-Temporal-Lab/elf `386d69348e6c30475761d3519ee0bfe10ed473d3`, branch `vldb2023-release`, the VLDB 2023 paper's |
| their bit streams' libraries | vigna/dsiutils `838c5070b74382a652649f24042c19e4f492dab5`, vigna/fastutil `dc909bb15d7cb0b9d40d7382bf7bb0a5c6c517e0`, built from source |
| ALP | cwida/ALP `31ca0ed11c93c99d3f5b5c30e01a3e1c3832d3ce`, unmodified, `-O2`, no `-march` |
| zstd | 1.5.7 from its source release, SHA-256 `eb33e51f49a15e023950cd7825ca74a4a2b43db8354825ac24fc1b7ee09e6fa3` |
| Go, Python, SQLite, g++ | `golang:1.27@sha256:2920bb124f1a2e12a1d39f0cab756ae0068441ee1da7655dec7f5e374944095b`: Go 1.27.3, Python 3.13.5 with SQLite 3.46.1 and `dbstat`, g++ 14.2.0, Debian 13 |
| Java | `eclipse-temurin:21-jdk@sha256:ebfbb18d11ff163ee83b21258acf12248af48d22a0fdfdc68fa1a1516c05976a`, Temurin 21.0.13+9 |
| PostgreSQL | `postgres:18@sha256:edb0c08dc33befd0262ab3b5cf1c4f3f8cfd8b09e88c05b45adee29d6761ddb6`, 18.6, 8 KiB blocks, `autovacuum=off`, `fsync=off`, `full_page_writes=off` |
| the page model | [page_model.py](../../tinystore/bench/page-model/page_model.py), unchanged, imported by `store.py` |

The binaries' hashes are in [binaries.txt](data/codec-pages-2026-11-01/binaries.txt).

| Run | UTC | Log |
|---|---|---|
| NAB through every encoder and into SQLite | 01:09:47–01:10:41 | the first lines of `nab.encode.jsonl` |
| ALP's datasets in four lanes, TSBS long in a fifth | 01:11:05–02:03:28 | [runs/lane-*.txt](data/codec-pages-2026-11-01/runs/) |
| TinyStore's TSBS metrics file built, H1.5 | 01:12–01:13, then `h15.py` and `h15_hist.py` | `tsbs-h15.json`, `h15-hist.json` |
| PostgreSQL: NAB, then 14 datasets | 01:19:33, 01:30:50–01:33:06 | [runs/pg-a.txt](data/codec-pages-2026-11-01/runs/pg-a.txt) |
| Docker Desktop stopped and was restarted; nothing was lost | | |
| PostgreSQL: the other 17 datasets | 07:12:09–07:13:12 | [runs/pg-b.txt](data/codec-pages-2026-11-01/runs/pg-b.txt) |
| the analyses | 07:12–07:15 | |

The commands are in the [harness's README](../bench/codec-pages/README.md),
"The round"; in short, from Git Bash with `MSYS_NO_PATHCONV=1`:

```sh
cd <repo>/tinystore/bench/codec-pages
java/codec-pages.sh build; sh alp/run.sh build          # and go build ./tinystore, README.md
HTTPS_PROXY=<proxy> python -I fetch_alp.py <corpus>/alp 1048576
python -I prepare.py <corpus>; (cd <corpus> && sha256sum -c <repo>/tinystore/bench/codec-pages/corpus-sha256.txt)
for d in <dataset> ...; do sh run.sh <corpus> $d; done  # 8 codecs x 24 lengths x 4 page sizes
sh pg_load.sh <dataset> ...
docker run --rm -v codec-pages-work:/work -v <repo>:/src:ro golang:1.27 sh -c '
  python3 -I /src/tinystore/bench/codec-pages/pg_model.py /work/pg/out /work/results/pg.jsonl <dataset> ... &&
  python3 -I /src/tinystore/bench/codec-pages/analyze.py /work/results /work/analysis &&
  python3 -I /src/tinystore/bench/codec-pages/pg_analyze.py /work/results/pg.jsonl /work/results /work/analysis'
```

Raw output is in [data/codec-pages-2026-11-01](data/codec-pages-2026-11-01):
every SQLite file's measurement, division and prediction
(`results/<dataset>.store.jsonl.gz`), every encoder's line
(`results/<dataset>.encode.jsonl`), PostgreSQL's size line a load
(`pg/<dataset>.sizes`) and the model's verdict a load (`pg/pg-model.jsonl.gz`),
the tables the sections below quote (`analysis/`), and H1.5's files (`h15/`).
The per-row and per-page dumps PostgreSQL wrote for the model, 129 MB, are
not kept; `pg_load.sh` writes them again.

## The corpora

| Dataset | What | Values | Pinned by |
|---|---|---:|---|
| ALP's 30 | the double datasets of ALP's `alp_dataset.hpp`, from its authors' `complete_binaries.zip` (Google Drive, 7,226,731,784 bytes): the first 2^20 values of 21 of them, the whole of the 9 smaller ones (Basel-Temp, Basel-Wind, Bird-Mig, Btc-Price, Blockchain, PM10-dust, POI-lat, POI-lon, SD-bench) | 23,597,644 | each prefix's SHA-256 in [corpus-sha256.txt](../bench/codec-pages/corpus-sha256.txt); a whole member also against the zip's CRC-32 when fetched |
| TSBS long | TSBS DevOps, 10 hosts, 10 s, 3 days, the release round's: 1,010 series | 26,179,200 | the generator at `8323e59`, seed 123; `long.lp` SHA-256 `a3e634a2…20f90c`, its conversion by `bench/tsbs` in corpus-sha256.txt |
| NAB | the 24 files of `realAWSCloudwatch` and `realKnownCause` that [nab-sha256.txt](../bench/nab-sha256.txt) pins at `ea702d75`, one series each | 137,301 | nab-sha256.txt |
| Alibaba | not measured: see the question table | | |

`fetch_alp.py` reads the zip's central directory by HTTP ranges and inflates
each member only as far as its prefix, 104,241,023 bytes of the 7 GB. A
prefix is a choice of this round: ALP's paper compresses each dataset whole.
A series
is cut into blocks of L values from its first value, the last block holding
the remainder; a block never spans two series.

## The codecs and their bodies

Every encoder reads the same series file, encodes each block alone, decodes
it with the codec's own decoder and compares every value's 64 bits; a body
holds what its codec needs to decode the block, the value count included.
Across all 6,144 encodings (32 datasets × 8 codecs × 24 lengths) not one
block failed its round trip. The [harness's README](../bench/codec-pages/README.md)
has each body's layout; what a reader must know:

- **Gorilla** is the value-only Gorilla the Chimp paper measured, from the
  Chimp authors' repository; **Chimp128** is their `ChimpN(128)`. Gorilla,
  Chimp, Chimp128 and Elf all end a stream with the NaN `0x7ff8000000000000`,
  so a block holding that NaN cannot round-trip; Elf turns every NaN into it
  and never returns on values of magnitude above about 9.22e17 or on
  subnormals. Since no block failed, no block of these corpora holds such a
  value. Chimp's, ChimpN's and Elf's fixed 8,000- and 12,000-byte output
  arrays drop bits silently past their end, which breaks blocks of 8,192
  values; the harness replaces that array by reflection before the first
  value, leaving the bytes written unchanged.
- **ALP** is a row group a block: ALP's own sampling picks ALP or ALP_RD and
  its exponent pairs on the block, then encodes vectors of 1,024. A vector
  shorter than 1,024, the whole of a block of 240 or 1,000 values, keeps only
  its values' packed bits; ALP's own benchmarks drop such a vector. The
  serialised bodies are 1.0032 times ALP's own size estimate at L = 1,000 and
  at L = 1,024 over ALP's datasets.
- **zstd** is one `ZSTD_compress` frame of the block's raw little-endian
  doubles, at level 3 or 19, default parameters.
- **TinyStore's codec** packs at most 240 values a call and keeps the first
  value and the count outside its stream. A body here holds them, 9 or 10
  bytes more than the stream; a block longer than 240 values is several of the
  codec's blocks in one row, `uvarint count | first value | uvarint length |
  stream` each, which TinyStore itself never writes.

Every SQLite file is built as page_model.py builds its own: a fresh file,
`page_size` set, `auto_vacuum=none`, WAL, every row in one transaction, ids 1
to N in block order, `wal_checkpoint(truncate)`; then divided by `dbstat` and
by the model's walk of every page, and predicted from `length(body)` of
every stored row.

## H1.2: the model predicts every file

| Page size | Files | Rows | Exact |
|---:|---:|---:|---:|
| 1,024 | 6,144 | 16,674,168 | 6,144 |
| 4,096 | 6,144 | 16,674,168 | 6,144 |
| 8,192 | 6,144 | 16,674,168 | 6,144 |
| 16,384 | 6,144 | 16,674,168 | 6,144 |
| all | 24,576 | 66,696,672 | 24,576 |

Exact means the page count, and the payload table's leaf, interior and
overflow pages, each equal to the prediction; in every file `dbstat`'s
division equals the walk's, the walk finds no format error and no page it
cannot account for, and the freelist is empty. A file holds from 1 row
(Btc-Price's 2,688 values in one block of 8,192) to 218,160 (TSBS long at 120
values a block). With the 1,184 files of
page-model.md this makes 25,760 files predicted to the page, all in the
model's domain: fresh, appended in key order, one transaction.
[h12-model.json](data/codec-pages-2026-11-01/analysis/h12-model.json) has the counts.

## H1.1: rankings in pages

A cell is a dataset, a block length (240, 1,000, 1,024) and a page size; its
eight codecs are ordered by payload bytes and by file bytes (the whole file,
page 1 included). A pair of codecs is reversed when the smaller payload makes
the larger file, and tied in the file when two different payloads take the
same pages.

| Cells | τ-b < 1 | with a reversal | reversed pairs | tied in the file only | best codec changes | τ-b mean | τ-b min |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 KiB, 96 | 36 | 26 | 34 of 2,688 | 53 | 7 | 0.964 | 0.655 |
| 4 KiB, 96 | 59 | 32 | 42 of 2,688 | 221 | 7 | 0.925 | 0.5 |
| 8 KiB, 96 | 55 | 24 | 28 of 2,688 | 290 | 4 | 0.921 | 0.5 |
| 16 KiB, 96 | 60 | 8 | 10 of 2,688 | 260 | 0 | 0.937 | 0.5 |
| L = 240, 128 | 57 | 24 | 29 of 3,584 | 279 | 5 | 0.946 | 0.5 |
| L = 1,000, 128 | 76 | 30 | 41 of 3,584 | 304 | 7 | 0.927 | 0.5 |
| L = 1,024, 128 | 77 | 36 | 44 of 3,584 | 241 | 6 | 0.937 | 0.5 |
| all 384 | 210 | 90 | 114 of 10,752 | 824 | 18 | 0.937 | 0.5 |
| the 348 of datasets of 100,000 values or more | 181 | 86 | 108 of 9,744 | 609 | 16 | 0.944 | 0.5 |

H1.1 was to be falsified if τ were 1 everywhere; it is below 1 in 210 of 384
cells. But the reversals are among near neighbours: a reversed pair's larger
payload is 1.2 % larger at the median (17 pairs more than 5 %, 10 more than
10 %), and its file 1.4 % smaller at the median (18 pairs more than 5 %, 5 more
than 10 %). The largest are on Btc-Price's 2,688 values: at L = 1,000 and
4 KiB Elf's payload is 38 % larger than ALP's and its file 20 % smaller, since
ALP's three rows of about 2,900 bytes need a leaf each and an interior page
above them, while Elf's three, averaging 4,021 bytes, share one leaf and two
overflow pages. On larger data:

| Dataset, L, page | Smaller payload | Larger payload | Payload | File |
|---|---|---|---:|---:|
| Basel-Wind, 1,000, 8 KiB | zstd -19, 3.782 B a value | TinyStore | +3.8 % | −9.7 % |
| Air-Pressure, 1,000, 16 KiB | zstd -3 | Chimp | +2.3 % | −9.2 % |
| Basel-Wind, 1,000, 4 KiB | zstd -3 | TinyStore | +2.7 % | −7.6 % |
| Basel-Temp, 240, 4 KiB | Elf | zstd -19 | +0.08 % | −6.3 % |
| Gov-26, 1,024, 4 KiB | ALP | TinyStore | +13.4 % | −6.1 % |

Where the best codec changes, the change is that of a near tie becoming a
step: on Gov-10 at L = 1,024 and 8 KiB, zstd -19's payload is 3.192 bytes a
value and TinyStore's 3.210, their files 4.070 and 3.914; on NAB at L = 1,000
and 4 KiB, zstd -19's 3.336 against ALP's 3.540, files 4.117 and 4.087. The
list is [h11-cells.csv](data/codec-pages-2026-11-01/analysis/h11-cells.csv),
every reversed pair [h11-reversals.csv](data/codec-pages-2026-11-01/analysis/h11-reversals.csv).

**Where each reversal's pages are.** Of the 114, 52 have no overflow page on
either side, so the smaller payload's rows simply fit fewer to a leaf; 50 have
overflow pages, but no more on the smaller payload's side, which loses on
leaves; 12 lose on overflow pages, rows just past the spill limit X that keep
M bytes on the leaf and one mostly empty overflow page each. At 8 and 16 KiB,
37 of the 38 reversals have no overflow at all; at 1 KiB, 31 of 34 have some.

**What is large is the spread, not the order.** The file-to-payload ratio of
one codec's rows runs from 1.008 to 4.082 over the cells, 1.149 at the
median, and within one cell it differs by codec:

| Page size | the worst codec's file/payload over the best's, median | largest |
|---:|---:|---:|
| 1 KiB | 1.158 | 1.692 |
| 4 KiB | 1.213 | 1.825 |
| 8 KiB | 1.213 | 2.092 |
| 16 KiB | 1.128 | 2.367 |

Six datasets at L = 1,000 and 4 KiB, payload and file bytes a value
([h11-values.csv](data/codec-pages-2026-11-01/analysis/h11-values.csv) has
every cell):

| Dataset | TinyStore | Gorilla | Chimp | Chimp128 | Elf | ALP | zstd -3 | zstd -19 |
|---|---|---|---|---|---|---|---|---|
| TSBS long | 0.590, 0.658 | 1.267, 1.629 | 1.566, 1.905 | 2.432, 3.188 | 1.420, 1.754 | 1.222, 1.445 | 1.108, 1.255 | 1.061, 1.198 |
| NAB | 3.943, 4.743 | 5.597, 6.772 | 5.395, 6.593 | 4.017, 4.922 | 4.232, 5.101 | 3.540, 4.087 | 3.531, 4.326 | 3.336, 4.117 |
| City-Temp | 1.117, 1.324 | 7.223, 8.023 | 5.627, 6.004 | 2.781, 4.062 | 2.189, 3.914 | 1.366, 1.727 | 2.150, 3.531 | 1.794, 2.055 |
| Basel-Wind | 3.942, 4.412 | 7.773, 8.260 | 6.838, 8.260 | 4.791, 5.075 | 4.596, 4.777 | 3.762, 4.312 | 3.840, 4.777 | 3.798, 4.611 |
| Air-Pressure | 1.428, 1.910 | 5.497, 5.934 | 5.059, 5.301 | 4.319, 4.734 | 2.422, 4.008 | 2.142, 3.594 | 4.946, 5.301 | 4.142, 4.707 |
| Medicare-1 | 4.172, 4.762 | 5.496, 6.191 | 5.283, 5.918 | 3.753, 4.598 | 3.762, 4.613 | 4.563, 5.102 | 3.760, 4.652 | 3.556, 4.453 |

On City-Temp Elf's payload is 2.189 bytes a value and its file 3.914, 79 %
more: its 1,000-value bodies average about 2,190 bytes, just past the 2,034
at which two fit a 4 KiB leaf, so most take a leaf alone, and zstd -3's,
averaging 2,150, mostly do too. ALP's average 1,366, about the 1,352 at which
three fit, and share leaves by twos and threes.

## H1.3: the block length the file wants

For each dataset, codec and page size, 24 block lengths from 120 to 8,192
(120, 160, 200, 240, 320, 400, 480, 640, 800, 960, 1,000, 1,024, 1,280, 1,600,
1,920, 2,048, 2,560, 3,200, 3,840, 4,096, 5,120, 6,144, 7,168, 8,192). A curve
differs when the length with the smallest payload leaves a file strictly
larger than the length with the smallest file; the penalty is how much
larger.

| | Curves | differ | penalty median | penalty above 5 % | largest |
|---|---:|---:|---:|---:|---:|
| 1 KiB | 256 | 111 | 10.5 % | 79 | 65.6 % |
| 4 KiB | 256 | 138 | 4.0 % | 66 | 58.4 % |
| 8 KiB | 256 | 143 | 3.4 % | 53 | 74.9 % |
| 16 KiB | 256 | 151 | 6.2 % | 86 | 100 % |
| ALP | 128 | 69 | 5.2 % | 35 | 65.6 % |
| Chimp | 128 | 66 | 1.9 % | 20 | 55.2 % |
| Chimp128 | 128 | 62 | 2.6 % | 20 | 62.5 % |
| Elf | 128 | 62 | 5.3 % | 33 | 38.1 % |
| Gorilla | 128 | 96 | 6.9 % | 63 | 74.9 % |
| TinyStore | 128 | 100 | 10.2 % | 68 | 100 % |
| zstd -3 | 128 | 45 | 4.7 % | 22 | 39.6 % |
| zstd -19 | 128 | 43 | 5.7 % | 23 | 66.7 % |
| all | 1,024 | 543 | 5.5 % | 284 | 100 % |
| datasets of 100,000 values or more | 928 | 501 | 4.8 % | 245 | 74.9 % |

The 100 % is SD-bench, 8,927 values, at 16 KiB: TinyStore's best payload, at
240, takes four pages where 480 takes two. On large data the largest is
Arade-4 under Gorilla at 8 KiB: 640 values a block give 7.24 bytes a value of
payload and 12.83 of file, bodies of about 4,630 bytes one to a leaf; 1,280
give 7.24 and 7.34. On TSBS long at 4 KiB, zstd -3's payload is smallest at
2,048 and its file at 7,168, 13.4 % apart; TinyStore's at 240 and 5,120,
10.9 %; Gorilla's at 320 and 200, 8.0 %.

The streaming codecs' payload keeps shrinking to the longest block (8,192 is
the payload's best for Chimp in 124 curves of 128, Elf 120, zstd 108), so for
them the file's best is shorter whenever a long body ends just past a page.
Gorilla's and ALP's payloads are smallest at short lengths, 120 to 320 in 72
curves of 128 each. TinyStore's payload is
smallest at its own cap, 240, in 120 curves of 128, while its file is
smallest at 8,192 in 32 and at 240 in 22: the file prefers several of the
codec's blocks a row, which is what
[metrics-payload-2026-10-09](metrics-payload-2026-10-09.md) found by packing,
here on 32 datasets. [h13-optima.csv](data/codec-pages-2026-11-01/analysis/h13-optima.csv)
has every curve.

## H1.4: PostgreSQL, and a file with no pages

The bodies of L = 240, 1,000 and 1,024 were loaded into PostgreSQL 18, one
fresh table `(id integer primary key, body bytea)` a load, by one `COPY`, the
body column's storage EXTERNAL (out of line past the threshold, never
compressed) or EXTENDED (the default: pglz first). Sizes are
`pg_relation_size` of the heap's main, free-space-map and visibility-map
forks, the TOAST table's forks, the TOAST index and the primary key, after a
checkpoint. 8 KiB is the only block size of the image; the others need a
build of their own.

| 96 cells each | τ-b < 1 | with a reversal | reversed pairs | tied only | τ-b mean | best changes |
|---|---:|---:|---:|---:|---:|---:|
| PostgreSQL EXTERNAL, every relation and fork | 84 | 37 | 81 | 338 | 0.873 | 11 |
| PostgreSQL EXTERNAL, heap and TOAST main forks | 84 | 37 | 78 | 338 | 0.875 | |
| PostgreSQL EXTERNAL, pages holding tuples | 49 | 17 | 17 | 190 | 0.953 | |
| PostgreSQL EXTENDED, every relation and fork | 85 | 39 | 93 | 341 | 0.863 | 11 |
| PostgreSQL EXTENDED, pages holding tuples | 57 | 28 | 34 | 191 | 0.940 | |
| SQLite, 8 KiB, the same rows | 55 | 24 | 28 | 290 | 0.921 | |
| flat file, 4 + body bytes a block | 0 | 0 | 0 | 0 | 1 | 0 |

The flat file is the no-padding control: at one block length every codec has
the same number of blocks, so its order is the payload's in every cell, as it
must be. PostgreSQL reorders more than SQLite, and most of the difference is
in pages that hold nothing: counting only pages with tuples, its reversals
fall from 81 to 17 (EXTERNAL). Those empty pages are bulk extension's: `COPY`
extends a relation by up to 64 pages at a time and leaves the rest of the last
extension empty, 23,800 of the 375,492 heap pages here, 6.3 %; the TOAST
table, extended a page at a time, has none. A body goes out of line above
2,000 bytes, into chunks of 1,996, four 2,032-byte chunk tuples a page.
Under EXTENDED, pglz compressed 34,469 of 2,459,880 stored values, 1.4 %.
[pg-cells.csv](data/codec-pages-2026-11-01/analysis/pg-cells.csv) has every cell,
[pg-summary.json](data/codec-pages-2026-11-01/analysis/pg-summary.json) the totals.

**The model extended to the heap and TOAST.** From PostgreSQL 18's rules for
8 KiB pages ([pg_model.py](../bench/codec-pages/pg_model.py)): a heap tuple
is 24 bytes of header, 4 of the id and the body's datum (1 + n bytes up to
127, 4 + n above, its stored size when compressed inline, an 18-byte pointer
out of line), and takes MAXALIGN of that plus a 4-byte line pointer after a
24-byte page header, while the page's free space is at least that; a value
goes out of line when its tuple would exceed 2,032 bytes; a chunk tuple is
24 + 4 + 4 + 4 + l bytes, the chunk keeping its 4-byte header because a TOAST
table's `chunk_data` is PLAIN. Checked on every load against pageinspect's
`lower` and `upper` of every page and each row's `pg_column_size` and place:

| 1,536 loads | Holds |
|---|---:|
| every heap page's bytes equal its tuples' | 1,536 |
| every TOAST page's bytes equal its chunks' | 1,536 |
| a row out of line exactly when the threshold says | 1,536 |
| every value's chunks 1,996 bytes but the last | 1,536 |
| heap pages with tuples, sequential fill exact | 1,048 (357,988 predicted, 351,692 measured) |
| TOAST pages with tuples, sequential fill exact | 1,039 (379,183 predicted, 368,881 measured) |

The first version of the chunk rule gave a short last chunk a 1-byte header
and missed 1 to 4 TOAST pages a load; the PLAIN storage of `chunk_data`
explains every one. What the sequential fill does not model is the
free-space map: when a page is full, `RelationGetBufferForTuple` records its
free space and asks the map for another, so a later small tuple, a series'
last block or a value's last chunk, goes back into an earlier page. Modelling
the map's search would close the 1.8 % and 2.8 %; it was not done tonight.

## H1.5: the groups layout chosen offline

[metrics-layout-2026-10-09](metrics-layout-2026-10-09.md) measured
TinyStore's groups table as WITHOUT ROWID and as a rowid table with its
primary key become a UNIQUE index: Alibaba 21.40 % smaller, TSBS 0.45 %
larger. Both are predicted here before any file is rebuilt.

**TSBS, from its rows.** The round's Go builder at `e307c48` rebuilt
TinyStore's TSBS file from the same corpus at the round's original size,
4,665,344 bytes (SHA-256 in binaries.txt). Its 2,020 groups records average 227.4 bytes, the
largest 653, none above the 1,002 bytes an index b-tree keeps on a 4 KiB page:
WITHOUT ROWID costs them no overflow, and a rowid table can only add its
index. [h15.py](../bench/codec-pages/h15.py) predicts, from those records in
key order, 127 pages as WITHOUT ROWID, 121 as a rowid table and 11 for its
index: +20,480 bytes, the round's measured difference to the byte
(4,554,752 − 4,534,272). Rebuilt here the same way, both are those page
counts, and the files 4,538,368 and 4,558,848, +0.451 %; the one page by
which this file exceeds the round's is `sqlite_autoindex_series_1`, an index
filled out of key order and outside the model, 28 pages in SQLite 3.46.1
against the round's 27 in 3.53.4.

**Both corpora, from histograms.** The Alibaba file could not be rebuilt
without its corpus, but metrics-layout's raw output keeps every variant's
`directory_size_histogram` and `body_size_histogram`.
[h15_hist.py](../bench/codec-pages/h15_hist.py) predicts the groups object,
its index and the payloads object from them alone, for 25 seeded orders of
the rows, the control's other objects unchanged; the change is predicted
against the control with the same order.

| Corpus | Variant | Predicted change | Measured | Groups pages, predicted and measured | of them overflow |
|---|---|---:|---:|---|---|
| TSBS | rowid | +0.27 % to +0.54 % | +0.45 % | 120–121; 121 | 0; 0 |
| Alibaba | rowid | −22.19 % to −21.64 % | −21.40 % | 367–374; 381 | 0; 0 |
| Alibaba | WITHOUT ROWID, the control | | | 745–748; 748 | 465; 465 |
| Alibaba | inline 32 | −1.75 % to −1.33 % | −1.69 % | 755–760; 757 | 469; 469 |
| Alibaba | inline 64 | +16.26 % to +16.57 % | +16.39 % | 1,313–1,318; 1,317 | 988–989; 988 |
| Alibaba | inline 128 | +25.57 % to +25.94 % | +25.70 % | 1,598–1,603; 1,601 | 1,268–1,269; 1,269 |
| Alibaba | rowid, inline 32 | −23.10 % to −22.61 % | −22.79 % | 388–395; 395 | 0; 0 |
| Alibaba | rowid, inline 64 | −23.76 % to −22.79 % | −21.10 % | 639–654; 684 | 0; 0 |
| TSBS | inline 128 | +58.81 % to +59.98 % | +58.81 % | 995–1,000; 999 | 793–795; 792 |
| TSBS | rowid, inline 128 | +0.09 % to +1.36 % | +0.99 % | 335–341; 348 | 0; 0 |

The overflow pages, which depend on each row alone, come out of a histogram
exactly or within three pages; the groups' pages are within the range of
random orders in all 10 WITHOUT ROWID variants and 6 of the 10 rowid ones,
and above it by 7 to 30 pages in the other 4, where a greedy fill of the
leaves depends on the rows' order. Their real order, by series and time, is
not random; that it explains the four is a hypothesis, not shown. `inline 0`
misses most: 776 and 884 payload pages measured against 749–757 and 807–809
predicted. In the round's code the variant gives the bodies it moves out of
the directories new payload ids above the old ones and inserts them group by
group, interleaved with old ids, so the payloads table is filled out of key
order, outside the model; that this is the whole of the difference is not
measured.

| Choice | TSBS predicted | measured | Alibaba predicted | measured |
|---|---|---|---|---|
| rowid or WITHOUT ROWID, inline 16 | WITHOUT ROWID | WITHOUT ROWID | rowid | rowid |
| inline threshold, WITHOUT ROWID | 64 | 64 | 0 | 0 |
| inline threshold, rowid | 64 | 64 | 64 | 32 |

Three of four threshold choices and both layout choices are right; the wrong
one is the rowid table on Alibaba, where the predicted ranges of 32 and 64
overlap and the leaves are under-predicted. The directories of a threshold
other than 16 are the round's own re-encoded ones, read from its histograms:
this predicts the page half of the choice, not the encoding half.

## What did not run

- **Alibaba**, for H1.1–H1.4 and the row-level half of H1.5: the archive's
  only host, Alibaba's Beijing object storage, answers this network with a
  redirect to a block page and the proxy with 502; `machine_usage.tar.gz`
  (SHA-256 `3e6ee87f…a7a`) was never fetched.
- **PostgreSQL at other block sizes** and the **TOAST threshold moved**
  (`toast_tuple_target`): the image is 8 KiB only.
- **Parquet and RocksDB** as no-padding controls: the flat file stands in
  for both, and shows the expected order exactly, but a real columnar file's
  page headers and footer are not in it.
- **Camel and DeXOR**, which candidates.md also names: not attempted.
- **The free-space map** in the PostgreSQL model, above.
- **ALP's datasets whole**: 21 of the 30 are their first 2^20 values.

## What follows

- **For the paper**: lead with the spread and the block-length optimum, not
  the rank reversal. Codec order survives pages with τ-b 0.94 on average in
  SQLite and 0.87 in PostgreSQL; what changes is how much of the file each
  codec's rows waste (the worst 13–21 % above the best at the median within a
  cell) and which block length is best (53 % of curves, 5.5 % at the median).
  The exact model is the explanation of each: 25,760 SQLite files and the
  page-level accounting of 1,536 PostgreSQL loads.
- **For TinyStore**: its codec is the smallest payload on TSBS long and
  City-Temp, and its file prefers rows of several 240-value blocks; packing,
  already measured in metrics-payload-2026-10-09, is what the model recommends
  across 32 datasets. Nothing changes in TinyStore from this round.
- **Next round**: Alibaba from another network; the free-space map in the
  PostgreSQL model; PostgreSQL built at 4 and 16 KiB; a Parquet control; and
  the H1.5 histograms replaced by rows wherever the order matters.
