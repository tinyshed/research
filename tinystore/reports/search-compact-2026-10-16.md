# Search from compact blocks, under a long mix of writes — 16 October 2026

[search-blocks-2026-10-16](search-blocks-2026-10-16.md) answered a million
documents exactly from blocks of our own in SQLite, its window executor
measured apart from Tantivy, its store larger than FTS5's and its head left
to grow. This round makes the blocks smaller and cheaper to read, packs the
head into them, and measures five readers in one session and two long mixes
of searches, sets and deletes, every answer of the blocks checked against
scoring every posting. Nothing here is a product promise.

| Question | Answer |
|---|---|
| How near Tantivy, in one session? | The compact blocks read ahead answer `or` at p99 5.9–6.4 ms against 3.6–4.0, 1.5–1.8 times; at the median 2.5–4 times; FTS5 22–90 times later than them |
| At what memory? | A reader 71 MiB at its peak, 58 of them the process's own, against Tantivy's 309–323 |
| How small? | Postings, directories and lengths 375 MiB, against 646 packed, 725 as rows and FTS5's 529 with positions; positions would add 283 |
| Is the answer exact? | Yes: every executor, with and without reading ahead, and 1,398 checks among 15,038 writes, 0 differ |
| Does packing the head hold the tail? | Yes: packed every 1,000 writes, `or`'s p99 stays 14–16 ms over 50,000 operations; never packed, it climbs from 13 to 37 ms |
| What does packing cost? | 0.86 s a thousand writes on average, 1.04 s at most, the writer held meanwhile |

## The compact format

The blocks of the earlier round, with four changes:

- A field's length is kept once a document, in rows of 65,536 documents, not
  in every posting; a block holds documents and frequencies alone. A reader
  keeps the rows it reads, 8 MiB for a million documents.
- A key in no more than 16 documents keeps its postings, lengths and all, in
  its own row with no block or directory: 3,167,989 of 3,581,369 keys.
- The whole collection is built in one pass, so a key has no short block but
  its last: 882,828 blocks for 87,964,148 postings, against 5,735,478. The
  pass held 2.9 GiB at its peak; a store would build in segments and merge.
- A set gives its document a new number and marks the old one gone, as
  Lucene does: the head's documents always follow every block's, and packing
  the head appends to each key's list, filling its last block first.

LAB_AHEAD=8 reads up to eight of a list's blocks in one statement once the
list is being read in order. LAB_POSITIONS=1 keeps each posting's positions
beside its block, to weigh them; no query reads them.

## Reads, five arms in one session

Rounds A B C D E then E D C B A, each process after a warm pass of its own;
`compact` and `compact-ahead` are the window executor over the compact blocks,
`packed` the window executor over the packed blocks of search-blocks.

| | `or` p50 ms | `or` p99 ms | `pruned-or` 5 % p50 | p99 | `pruned-or` 2 % p50 | p99 |
|---|---|---|---|---|---|---|
| FTS5 | 76.26; 77.30 | 496.75; 497.63 | 23.09; 24.10 | 85.42; 89.39 | 12.60; 12.60 | 48.20; 48.12 |
| packed | 1.17; 1.16 | 7.76; 7.69 | 0.93; 0.94 | 3.86; 4.10 | 0.72; 0.74 | 2.74; 2.83 |
| compact | 1.00; 1.02 | 6.86; 7.41 | 0.79; 0.81 | 3.66; 3.68 | 0.63; 0.76 | 2.46; 2.95 |
| compact-ahead | 0.85; 0.84 | 6.42; 5.88 | 0.70; 0.70 | 3.08; 3.02 | 0.58; 0.58 | 2.22; 2.15 |
| Tantivy | 0.33; 0.33 | 3.96; 3.55 | 0.22; 0.22 | 1.41; 1.46 | 0.14; 0.16 | 0.88; 0.96 |

nDCG@10 is the blocks' 0.4318, 0.4312 and 0.4259 in every arm of them,
Tantivy's 0.4353, 0.4370 and 0.4298, FTS5's 0.4177, 0.4177 and 0.4065.
Readers held at their peak: FTS5 and packed 68 MiB, compact 71, Tantivy
309–323.

Means of the 600 questions in the check, `or`:

| compact | mean ms | blocks opened | statements | directories ms | blocks ms | every posting ms |
|---|---|---|---|---|---|---|
| cursor | 2.57 | 212.4 | 212.4 | 0.26 | 0.87 | 17.10 |
| cursor, ahead 8 | 2.22 | 252.0 | 70.1 | 0.25 | 0.55 | 12.73 |
| window | 1.77 | 213.0 | 213.0 | 0.25 | 0.86 | 16.98 |
| window, ahead 8 | 1.45 | 256.5 | 71.2 | 0.25 | 0.55 | 12.48 |

Against search-blocks' window executor over the packed blocks, 0.61 ms on
directories and 1.34 ms on blocks, the compact format reads its directories
in 0.25 ms, a key's directory one row read with its number; reading ahead
opens a fifth more blocks in a third of the statements. Every arm and plan
of the check differed in no question.

## Size

| Store | MiB | postings and what finds them | positions | build s | build HWM MiB |
|---|---|---|---|---|---|
| compact | 1,344 | blocks 218, keys and directories 149, lengths 8: 375 | 283 | 83 | 2,935 |
| packed (search-blocks) | 1,332 | blocks 457, directories 103, keys 86: 646 | — | 156 | 1,747 |
| rows (search-blocks) | 1,411 | blocks 495, directories 163, keys 67: 725 | — | 131 | 1,497 |
| FTS5 | 1,240 | postings with positions 511, sizes 16, index 2: 529 | in 511 | 94 | 728 |
| Tantivy, stored fields compressed | 706 | — | in 706 | 57 | 1,780 |

Every SQLite store holds the documents as written, 666 MiB, and an index of
their keys, 20; the compact store's build time includes its positions.

## Churn

Two copies of the compact index, each 50,000 operations from one seed: in a
hundred, 70 searches of the 600 questions, `or` and `pruned-or` 5 % in turn,
25 sets of a judged passage with its body changed and 5 deletes; every 25th
search checked against scoring every posting. The window executor, no
reading ahead. The reader is told the documents a set marked gone, as a
store's readers would share its writer's memory.

| packed | operations | `or` p50 | `or` p99 | `pruned-or` p50 | p99 | set p50 | set p99 | head rows |
|---|---|---|---|---|---|---|---|---|
| every 1,000 writes | 5,000 | 1.78 | 12.58 | 1.33 | 5.90 | 3.56 | 11.19 | 51,018 |
| | 25,000 | 2.28 | 13.95 | 1.85 | 7.51 | 3.48 | 11.23 | 52,355 |
| | 50,000 | 2.56 | 14.40 | 1.91 | 7.97 | 3.60 | 12.05 | 3,868 |
| never | 5,000 | 1.76 | 13.16 | 1.26 | 6.12 | 3.57 | 12.01 | 145,232 |
| | 25,000 | 4.04 | 24.92 | 2.47 | 9.23 | 3.99 | 17.58 | 701,094 |
| | 50,000 | 6.22 | 36.68 | 3.12 | 12.37 | 4.03 | 17.28 | 1,339,556 |

A row is 5,000 operations; every row is in [the measurements](search-compact-measurements-2026-10-16.md).
Both mixes made 34,962 searches and 15,038 writes, checked 1,398 searches
and found none that differed; 14,845 documents were gone at the end. The
packed mix packed 15 times, 856 ms on average and 1,038 at most, and its
store grew 15 MiB; the other's head grew it 71 MiB. Even packed, a search
in the mix answers at 2.3–2.5 times the median of the reads above: the head
between packs, up to 57,000 rows, and the gone documents the blocks still
hold, which nothing yet removes.

## What this round does not show

- Merges that remove gone documents from blocks, and a build in segments: the
  gone only accumulate, and the build held the collection in memory.
- A pack that does not hold the writer for a second: packing every thousand
  writes keeps a set waiting up to that long.
- Phrases: positions are weighed, never read.
- Readers at once, a second corpus, a second machine; no significance test.
- This round's per-question answers stayed on the volume; its summaries are
  in [data/search-compact-2026-10-16](data/search-compact-2026-10-16/).

## What follows

One session now sets the compact blocks read ahead at 1.5–2.5 times
Tantivy's p99 and 2.5–4 times its median, 22–90 times FTS5, with a reader of
71 MiB, sets in 3.5 ms and every answer exact; packing the head keeps the
tail flat through 15,000 writes. What a book would still need measured:
merges that purge the gone, packing in small steps, phrases read from their
positions, vectors beside the words, and all of it again on a second corpus
and machine.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop's WSL2 VM on an AMD
Ryzen 7 7700, the container limited to 2 CPUs and 6 GiB; Rust 1.99.0;
rusqlite 0.40.2 with SQLite 3.53.2, every SQLite file opened as TinyStore
opens one (4 KiB pages, WAL, `synchronous=full`, a 1 MiB cache); Tantivy
0.26.2. The corpus of search-1m-2026-10-16, checked against
[corpus.sha256](../search-plan-lab/corpus.sha256) and
[miracl.sha256](../search-plan-lab/miracl.sha256). The packed index is the
one run-packed.sh built at `4e94f8e`.

Harness `798638e` ran [run-compact.sh](../search-plan-lab/run-compact.sh)
from 10:55:13 to 11:16:57 UTC; `summarize-compact.py` writes every table here
from the raw lines.

```sh
MSYS_NO_PATHCONV=1 docker run --rm --cpus 2 --memory 6g -v <repo>:/src \
  -v search-plan-cargo:/usr/local/cargo/registry -v search-plan-target:/target \
  -v search-plan-perf:/perf -e CARGO_TARGET_DIR=/target rust:1.99 \
  sh -c 'cd /src/tinystore/search-plan-lab && sh run-packed.sh && sh run-compact.sh'
python tinystore/search-plan-lab/summarize-compact.py tinystore/reports/data/search-compact-2026-10-16
```
