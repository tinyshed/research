# The compact blocks made fast on MS MARCO, over ordinary SQLite tables — 1 November 2026

**The harness of this round,
[bench/search-msmarco/fast](../bench/search-msmarco/fast/README.md) with
`src/fast.rs` and `src/fast2.rs`, is at research commit `19e6458`.** Its
specification was written after the counts and a profile on a busy machine
and before any time on a quiet one.
[search-msmarco-timing-2026-11-01](search-msmarco-timing-2026-11-01.md)
found the compact blocks' p99 3.0–3.5 times Tantivy's in WSL2's containers.
This round finds where a query's time goes, takes it away a change at a
time, every answer still exact, and lays the same blocks out again for
reading, in SQLite tables a reader opens as it opens any other. Every time
below is this machine's in WSL2's VM, as the earlier round's were: an order
of engines and their ratios in one run, not Linux's latencies.

| Question | Answer | What follows |
|---|---|---|
| Where did the compact blocks' time go? | In the reader and in the layout both: a bound found again by binary search for every candidate and every passive list; a SQL statement for every one to eight blocks; varints for blocks and directories, and a row and its key for every block; four fields' lengths interleaved in 70 MB | — |
| How near Tantivy does it come? | **At its p99, and 1.6 times its p50**: `fast3`, the same blocks bit-packed in one row a key, p99 26.6–28.6 ms against Tantivy's 26.9–28.0, p50 4.1–4.2 ms against 2.3, from compact's 89.7–95.3 and 10.9–11.3 in the same run; with directories kept between queries p50 3.6–3.7 | the directories kept between queries: p50 3.6–3.7 ms, 1.6 times Tantivy's, p99 unchanged |
| H2.1, within two times Tantivy at p99? | Met on this machine: `fast3` 0.96–1.12 of Tantivy's p99 over runs 2 and 3, the highest interval bound 1.31 | bare Linux |
| Against PISA and Lucene? | Still behind: their p99 14.7–15.7 and 12.1–12.4 ms, their p50 2.0–2.1 and 1.6–1.7 | the per-term and per-posting costs below |
| What does the layout cost in space? | Less than compact's: what a query reads is 1.99 bytes a posting against compact's 3.18, Tantivy's 1.70, Lucene's 1.61 and PISA's 2.11 | no part of the speed came from a larger index |
| Are the answers exact? | Every arm on the sample's 1,097 queries: scores within 1e-9 of compact's, documents differing in 6 queries and only where scores tie | — |

## What was built

Both readers read the same `compact.db` the earlier round built and answer
exactly as compact1's reader; neither changes the lab's sources.

**`fast`**, compact1's reader with switches. With every switch off it opens
compact1's blocks query for query: 4,503,341 of 4,503,341 on the sample.

- `bounds=front`: a block's bound from its Pareto front of (frequency,
  length), its exact best under any average length, kept in `fast.db`
  (36 MB; 10,652,177 points for 2,752,140 blocks, 3.9 a block). compact's own
  bound joins the largest frequency and the shortest field, which may be two
  postings.
- `seed`: θ starts at the best of the query's keys' tenth-best scores, from
  each key's ten best postings kept in `fast.db`; ten documents score at
  least that, so a document whose bound is below it cannot be among the ten.
- `norms=flat`: the field's lengths as one u16 array.
- `read=blob`: a block through one incremental-blob handle moved from row to
  row, from a rowid copy of the blocks, instead of a statement.

**`fast2`**, the same blocks laid out for reading in `fast2.db`, ordinary
tables a reader attaches:

| Table | What it holds | MiB | Bytes a posting |
|---|---|---|---|
| `lists` | one row a key, its 128-posting blocks one after another, each bit-packed at its own widths of gap and frequency | 491.5 | 1.492 |
| `dirs` | one row a key, 16 fixed-width bytes a block: first, last, the best saturation at the build's average rounded up to an f32, offset | 46.0 | 0.140 |
| `lengths` | one byte a document; 17 documents of 255 tokens or more in `long` | 8.5 | 0.026 |
| compact's `ckeys`, read for the dictionary and the 2,322,700 keys too short for blocks | | 110.5 | 0.335 |
| **what a query reads** | | **656.5** | **1.992** |

The seed's ten postings a key are read from `fast.db`'s `fdir`, 34.5 MiB
with the fronts `fast2` does not read; the seeds alone, ten varint triples
for each of 143,271 keys, are about 8 MiB by their count, not measured
apart.

against compact's 1,048.1 MiB and 3.180 bytes a posting, the same
345,553,319 postings ([fast2-build.jsonl](data/search-fast-2026-11-01/logs/fast2-build.jsonl)).
The bit-packed blocks are 497,948,833 bytes of payload against compact's
772,967,147 of varints, and their table 515 MB of pages against `cblob`'s
912: one row a key leaves no b-tree of block keys and fills its overflow
pages. `ckeys` still carries compact's own directories, 15.0 MiB of it,
which `fast2` does not read.

A block's bound is kept for the build's average length; at a larger average
it is multiplied by the new over the old, which BM25's saturation never
exceeds, so the bound stays above every score as the collection grows.
`fast2` reads one blob handle a list, at each block's offset; no statement
runs a block. It reads no head and no gone: it measures reads of a settled
index, as the timing harness does.

Its switches, each measured: `table`, scores of frequency below 16 and
length below 255 tabled when a list opens; `words`, a block unpacked eight
bytes at a time; `walk`, the bound of each passive list's block for a
window's candidates found by stepping forward, since candidates come in
order, where compact's executor searches the directory twice a candidate;
`direct`, an essential list's postings added from its decoded block without
the cursor's checks; `windows`, each list's blocks for a window found by
stepping forward instead of a search a window; `cache`, each key's directory
kept decoded between queries. `fast3` is `words`, `walk` and `direct`.

## Counted, on the sample

Blocks each reader opened over the 1,097 queries, deterministic
([answers/](data/search-fast-2026-11-01/answers/)):

| | blocks | against compact |
|---|---|---|
| compact1, and `fast` with every switch off | 4,503,341 | 1 |
| `fast`, front bounds | 3,916,373 | 0.87 |
| `fast`, the seed | 4,252,359 | 0.94 |
| `fast`, both and flat lengths | 3,709,659 | 0.82 |
| `fast2`, every switch | 3,072,852 | 0.68 |

`fast2` counts every block it decodes; compact1 and `fast` count those a
statement returned, read ahead included.

## Timed, two runs

Each run: every arm a process a pass, one query at a time, warm (every query
once untimed, then once timed), the sample's 1,097 queries, each container 2
CPUs and 6 GiB; four rounds, the arms in one order and then the other. p50
and p99 in ms, the four rounds as `a; b; c; d`, and the ratio to Tantivy's
of the same round with the first round's interval
([run 1](data/search-fast-2026-11-01/timing/20261101T132605Z/summary.md),
[run 2](data/search-fast-2026-11-01/timing/20261101T134909Z/summary.md)).

**Run 1**, 13:26–13:47 UTC:

| arm | p50 | p99 | p99 / Tantivy |
|---|---|---|---|
| compact | 11.82; 11.17; 11.14; 11.09 | 98.6; 96.0; 90.5; 92.0 | 3.55 [2.91, 4.06]; 3.19; 3.32; 3.45 |
| `fast`, flat lengths | 12.15; 12.22; 11.07; 11.05 | 86.6; 89.6; 84.2; 87.3 | 3.12 [2.63, 3.58]; 2.98; 3.09; 3.27 |
| `fast`, bounds, seed, flat | 11.12; 11.13; 10.32; 10.06 | 73.0; 73.3; 68.6; 66.7 | 2.63 [2.23, 3.06]; 2.44; 2.52; 2.50 |
| the same, blob reads | 9.16; 8.81; 8.82; 8.95 | 64.1; 61.8; 61.5; 61.7 | 2.31 [1.88, 2.74]; 2.05; 2.26; 2.31 |
| `fast2`, no table | 5.52; 5.20; 5.23; 5.31 | 51.8; 49.8; 49.8; 50.6 | 1.87 [1.56, 2.19]; 1.65; 1.83; 1.90 |
| `fast2` | 5.39; 5.14; 5.29; 5.33 | 52.2; 50.0; 49.5; 53.0 | 1.88 [1.59, 2.23]; 1.66; 1.82; 1.99 |
| Tantivy | 2.35; 2.43; 2.32; 2.43 | 27.8; 30.1; 27.2; 26.7 | 1 |
| PISA `block_max_maxscore` | 2.46; 2.32; 2.14; 2.23 | 16.2; 15.1; 14.6; 15.8 | 0.58 [0.48, 0.67]; 0.50; 0.54; 0.59 |
| Lucene | 1.66; 1.74; 1.71; 1.78 | 13.0; 12.9; 12.4; 12.7 | 0.47 [0.39, 0.54]; 0.43; 0.46; 0.48 |

**Run 2**, 13:49–14:07 UTC:

| arm | p50 | p99 | p99 / Tantivy |
|---|---|---|---|
| compact | 11.33; 10.92; 10.95; 11.25 | 95.3; 89.7; 90.5; 92.1 | 3.43 [2.89, 3.87]; 3.33; 3.23; 3.41 |
| `fast2` | 4.79; 4.82; 4.77; 4.77 | 49.5; 50.1; 50.6; 49.1 | 1.78 [1.57, 2.17]; 1.86; 1.81; 1.82 |
| `fast2`, words | 5.07; 4.95; 4.96; 5.02 | 49.7; 50.3; 47.9; 49.3 | 1.79 [1.56, 2.22]; 1.87; 1.71; 1.83 |
| `fast2`, walk | 4.66; 4.67; 4.69; 4.67 | 29.2; 29.2; 30.1; 31.4 | 1.05 [0.96, 1.26]; 1.08; 1.07; 1.16 |
| `fast2`, direct | 4.73; 4.61; 4.76; 4.75 | 51.1; 49.3; 48.7; 49.3 | 1.84 [1.58, 2.23]; 1.83; 1.74; 1.83 |
| **`fast3`** | **4.12; 4.11; 4.21; 4.17** | **26.6; 28.5; 28.6; 28.6** | **0.96 [0.89, 1.15]; 1.06; 1.02; 1.06** |
| Tantivy | 2.31; 2.31; 2.38; 2.33 | 27.8; 26.9; 28.0; 27.0 | 1 |
| PISA `block_max_maxscore` | 2.07; 2.08; 2.04; 2.11 | 15.6; 15.7; 14.7; 14.8 | 0.56 [0.48, 0.66]; 0.58; 0.52; 0.55 |
| Lucene | 1.68; 1.60; 1.65; 1.61 | 12.1; 12.2; 12.4; 12.3 | 0.44 [0.38, 0.53]; 0.45; 0.44; 0.45 |

**Run 3**, 14:11–14:24 UTC, the window walk and the directory cache
([run 3](data/search-fast-2026-11-01/timing/20261101T141139Z/summary.md)):

| arm | p50 | p99 | p50 / Tantivy | p99 / Tantivy |
|---|---|---|---|---|
| compact | 11.17; 11.28; 11.30; 11.29 | 92.2; 96.0; 93.2; 95.0 | 4.79 [4.53, 5.17]; 4.83; 4.92; 4.93 | 3.32 [2.81, 3.97]; 3.59; 3.53; 3.60 |
| `fast3` | 4.03; 4.06; 4.16; 4.07 | 29.9; 27.7; 29.1; 29.5 | 1.73 [1.65, 1.86]; 1.74; 1.81; 1.78 | 1.08 [0.93, 1.31]; 1.04; 1.10; 1.12 |
| `fast4`, windows walked | 3.82; 4.12; 3.88; 3.92 | 29.1; 28.9; 28.4; 29.9 | 1.64 [1.56, 1.76]; 1.76; 1.69; 1.71 | 1.05 [0.90, 1.25]; 1.08; 1.07; 1.13 |
| **`fast4`, directories kept** | **3.63; 3.73; 3.72; 3.63** | **27.3; 29.5; 28.2; 28.9** | **1.56 [1.48, 1.66]; 1.59; 1.62; 1.59** | **0.98 [0.87, 1.19]; 1.10; 1.07; 1.10** |
| Tantivy | 2.33; 2.34; 2.29; 2.29 | 27.7; 26.7; 26.4; 26.4 | 1 | 1 |
| PISA `block_max_maxscore` | 2.03; 2.34; 2.29; 2.18 | 14.6; 15.2; 14.7; 15.0 | 0.87 [0.81, 0.95]; 1.00; 1.00; 0.95 | 0.53 [0.45, 0.66]; 0.57; 0.56; 0.57 |
| Lucene | 1.60; 1.75; 1.70; 1.68 | 12.1; 12.6; 12.6; 12.6 | 0.69 [0.66, 0.74]; 0.75; 0.74; 0.73 | 0.43 [0.37, 0.55]; 0.47; 0.48; 0.48 |

The directory cache keeps a decoded directory a key for as long as the
reader lives; these 1,097 queries' keys fit in memory, a store's reader
would bound it. It is measured warm, as every arm is.

**What each change bought**, the mean of the four rounds against its
predecessor's in the same run. At p99: the flat lengths 8 %; front bounds
and the seed 19 % more; blob reads 12 % more; the layout of `fast2` 18 %
more; in run 2 the walk 40 % of what was left, `fast3` 44 %. At p50 the flat
lengths nothing (11.62 against 11.31 ms), the bounds and the seed 8 %, blob
reads 16 %, the layout 41 %, the walk 3 %, `fast3` 13 %. The score table
bought nothing measurable, nor `words` or `direct` alone; with the walk they
took p50 from 4.67 to 4.15.

Every arm's ids were the same in all four of its passes.

## Where the time goes now

A profile of `fast2`'s readers on the quiet machine
([profile/quiet-*](data/search-fast-2026-11-01/profile/)): each query's
time, the time to open its lists, the blocks it decoded and the postings it
scored, and a least-squares fit of a query's time less its lists on its
terms, blocks and postings scored:

| | mean ms | lists | per block | per posting scored | per term | R² |
|---|---|---|---|---|---|---|
| `fast2` | 8.94 | 9.3 % | 353 ns | 58 ns | 1.01 ms | 0.92 |
| `fast3` | 6.32 | 13.6 % | 374 ns | 25 ns | 0.70 ms | 0.96 |

A query of `fast3` decodes 2,801 blocks and scores 92,116 postings on
average. The walk took a posting's cost from 58 to 25 ns: the bounds
searched again for every candidate were most of it. What the fit puts on a
term, 0.70 ms, is what does not grow with blocks or postings: the windows,
each finding each list's blocks by binary search, and the directory read
and scaled whole. The fit is a hypothesis about the parts, not a
measurement of each.

## Forecasts

From the fit for `fast3`, each forecast an estimate until a run measures it:

| Change | What it removes | Forecast for `fast3`'s mean 6.3 ms, and what run 3 measured |
|---|---|---|
| windows walked, not searched (`windows`) | forecast before run 3: most of 0.70 ms a term | **measured**: p50 −3.5 %, p99 none; the fit's time a term is not the windows' search |
| directories kept between queries (`cache`) | forecast: most of 13.6 % opening lists, about −0.55 ms | **measured**: p50 −0.40 ms, −10 %; p99 −2 % |
| SIMD unpacking, PISA's SIMD-BP128 | about half of 374 ns a block | −0.5 ms |
| scoring 8 postings at once, lengths read in the block | about half of 25 ns a posting | −1.1 ms |
| documents reordered by graph bisection at build | blocks and postings decoded; it shrinks every engine's index and is reported to speed block-max processing, by how much here not measured | not estimated here |

Run 3 measured the first two: one forecast held, one did not. The two
left, on `fast4`'s p50 of 3.7 ms, forecast about 2.5–3 ms — near Tantivy's
2.3 but not Lucene's 1.6. What the fit put on a term, 0.70 ms, is still
unexplained by any measurement; the next profile times the directory's
scaling, the seeks of passive lists and the essential lists' first block
apart, before any of it is changed.

## Fields and prefixes, natively

This round measures one field. The lab's keys carry their field as a text
prefix, `b|word`, four rows a word for title, title stems, body and body
stems: a word asked costs four lookups in the dictionary, and a prefix query
four range scans. The layout above takes a list by rowid; the same tables
hold four fields as rowid = key × 4 + field under one dictionary row a word,
so a word is one lookup and a prefix one range of SQLite's own index
(`word >= 'auto' and word < 'autp'`), and a field's lengths one byte array
of its own, which a query on bodies alone never reads. That is smaller than
four keys a word, not larger; it is designed here, not measured.

## Limits

- One machine, in WSL2's VM; H2.1 asks for bare Linux too.
- `fast2` reads a settled index: no head, no gone, no writer. compact's and
  `fast`'s readers keep both.
- The seed, the fronts and `fast2.db` are built from `compact.db`; a store
  would write them with its blocks.
- The profile's parts are times taken inside the reader, which the timer
  itself slows; the fit's coefficients describe these 1,097 queries.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop 29.6.2's WSL2 VM,
16 CPUs and 15.2 GiB, on an AMD Ryzen 7 7700 with 31.1 GiB under Windows 11
Pro 10.0.26300; every engine, version and index as in
[search-msmarco-2026-11-01](search-msmarco-2026-11-01.md#environment-and-reproduction),
the readers here in the same crate, Rust 1.99.1, rusqlite 0.40.2 with its
bundled SQLite 3.53.2 and its `blob` feature. Before each run no process used
more than 0.2 % of the processors over five seconds, and nothing else ran
while it did.

| UTC | Step |
|---|---|
| 13:09–13:15 | `fast-build`, `fast2-build`; answers and a busy machine's profile, not measurements |
| 13:26–13:47 | timing, run 1 |
| 13:49–14:07 | timing, run 2 |
| 14:07–14:08 | the quiet profile |
| 14:11–14:24 | timing, run 3 |

```sh
docker run --rm -v search-msmarco-target:/target -v search-msmarco-perf:/perf rust:1.99 \
  /target/release/msmarco fast-build /perf/msmarco/compact.db
docker run --rm -v search-msmarco-target:/target -v search-msmarco-perf:/perf rust:1.99 \
  /target/release/msmarco fast2-build /perf/msmarco/compact.db
LABEL="quiet machine, WSL2 container" sh tinystore/bench/search-msmarco/fast/timing.sh
ARMS="compact fast2 fast2-words fast2-walk fast2-direct fast3 tantivy pisa-block_max_maxscore lucene" \
  LABEL="quiet machine, WSL2 container" sh tinystore/bench/search-msmarco/fast/timing.sh
ARMS="compact fast3 fast4 fast4-cache tantivy pisa-block_max_maxscore lucene" \
  LABEL="quiet machine, WSL2 container" sh tinystore/bench/search-msmarco/fast/timing.sh
```

Raw output in [data/search-fast-2026-11-01](data/search-fast-2026-11-01/).
