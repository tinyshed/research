# Search from blocks of our own over SQLite's pages — 16 October 2026

[search-1m-2026-10-16](search-1m-2026-10-16.md) left a trade between two
engines at a million documents: FTS5 inside the store's file, its reads
tens of milliseconds, and Tantivy beside it, a hundred times faster to read
and twenty times slower to set. This round asks whether a store can keep
FTS5's side — one SQLite file, its transactions, a reader's memory bounded by
its cache, a set in milliseconds — and still take a share of Tantivy's reads,
by keeping postings of its own as blocks in SQLite tables and answering a
query's top ten by block-max MaxScore. The same million documents, 600
questions, tokens, fields and plans as before; every answer checked against
scoring every posting. Nothing here is a product promise.

| Question | Answer |
|---|---|
| Is a top ten from the blocks exact? | Yes: on all 600 questions, every plan, both executors, both formats and after sets, its ten scores equal those of every posting scored |
| How fast, in the same session as FTS5 and Tantivy? | `or` p50 2.5–2.7 ms and p99 24–25 ms, against FTS5's 102–107 and 674–677 ms and Tantivy's 0.44–0.48 and 4.6–5.1 ms |
| Does executing a window at a time matter? | Yes: the same blocks opened, and p99 halves, `or` 25–27 → 11 ms, `pruned-or` 11–12 → 5.2–5.4 ms |
| Does a packed directory? | A little: `or`'s p50 1.85 → 1.4–1.6 ms, the pruned plans the same, the postings' tables 11 % smaller |
| Does a map of the file? | Its p50 7–13 % lower, its p99 not, and a reader holding 675–697 MiB against 68 |
| What does a set cost? | 4.1–4.8 ms at p50, FTS5 3.6–3.8 and Tantivy 64–67; a hundred a commit 43–86 ms |
| What do the sets cost the reads? | 600 documents in the head lengthen `or`'s p99 from 24 to 60 ms |

## The blocks

A key is a field and a term, `b|война`; its documents are blocks of up to
128 postings, each a document's delta, the term's frequency and the field's
length as varints, with a directory beside them of each block's first and
last document, largest frequency and shortest field. BM25's saturated
frequency rises with the frequency and falls with the length, so a block's
largest frequency over its shortest field bounds what the key adds to any of
its documents; a query computes that bound from the idf and the average
lengths it reads at that moment, so a set that changes the collection's
statistics leaves every bound an upper bound. This holds for this formula,
non-negative weights and a directory that is exact for the postings it
describes; another ranking needs its own argument.

The documents are written in four chunks of 250,000, a key's last block in a
chunk possibly short: 87,964,148 postings in 5,735,478 blocks. A set writes
the document's postings to a head and marks its rowid gone from the blocks,
in one transaction; a query skips gone postings as it opens a block and
reads the head as one more list a key. Positions are not kept, so no phrase
can be asked of these blocks.

Two executors answer from the same blocks. **cursor** is block-max MaxScore
a candidate at a time: lists ordered by their largest bound, the prefix
whose bounds sum to no more than the tenth score θ non-essential, candidates
from the rest, a non-essential list asked about a candidate while the bounds
of the blocks that could hold it might still lift it past θ. **window**
splits the documents into windows of 4,096, as Lucene's MaxScoreBulkScorer
and turbopuffer's FTS v2 do: lists are split into essential and not by their
bounds inside the window, a window no list can lift past θ is passed unread,
the essential lists add their postings into an array one list after another,
and the documents it holds are completed from the others in order.

Two formats hold the same blocks. **rows** keys by text, a directory row a
block; **packed** numbers the keys and keeps a key's directory one row a
chunk.

## Reads, three engines in one session

Rounds A B C then C B A, each process after a warm pass of its own.

| | p50 ms | p99 ms | nDCG@10 | Recall@10 | MRR@10 |
|---|---|---|---|---|---|
| FTS5, `or` | 102.55; 107.16 | 674.26; 676.72 | 0.4177 | 0.5662 | 0.4326 |
| blocks, cursor, `or` | 2.45; 2.65 | 23.99; 24.61 | 0.4318 | 0.5518 | 0.4720 |
| Tantivy, `or` | 0.44; 0.48 | 4.55; 5.07 | 0.4353 | 0.5580 | 0.4760 |
| FTS5, `pruned-or`, 5 % | 32.44; 33.11 | 118.04; 120.60 | 0.4177 | 0.5656 | 0.4336 |
| blocks, cursor, `pruned-or`, 5 % | 1.72; 1.73 | 11.49; 10.62 | 0.4312 | 0.5541 | 0.4709 |
| Tantivy, `pruned-or`, 5 % | 0.30; 0.30 | 1.84; 1.87 | 0.4370 | 0.5642 | 0.4755 |
| FTS5, `pruned-or`, 2 % | 17.37; 16.45 | 63.90; 65.54 | 0.4065 | 0.5541 | 0.4163 |
| blocks, cursor, `pruned-or`, 2 % | 1.24; 1.29 | 6.71; 6.62 | 0.4258 | 0.5488 | 0.4666 |
| Tantivy, `pruned-or`, 2 % | 0.20; 0.19 | 1.23; 1.17 | 0.4298 | 0.5538 | 0.4706 |

Every reader held 68 MiB at its peak, 59 of them the questions and every
document's id before the index opened, but Tantivy's, 309–323 MiB. FTS5 read
more slowly here than in search-1m-2026-10-16 an hour before on the same
machine (p50 102–107 against 74–80 ms, `or`), with three indexes rather than
two sharing the kernel's page cache; only numbers of one session are compared
with one another. The blocks score as Tantivy does, a field's BM25 a key
summed with its boost, and their nDCG@10 is Tantivy's within 1.5 %; FTS5's
bm25 weighs columns otherwise.

## Executors, formats and a map

Each pair alternated A B B A on one index.

| Blocks, `or` | p50 ms | p99 ms |
|---|---|---|
| rows, cursor | 2.65; 2.62 | 26.56; 24.64 |
| rows, window | 1.68; 1.68 | 11.43; 11.17 |
| rows, window, again | 1.86; 1.85 | 11.77; 11.88 |
| packed, window | 1.40; 1.57 | 9.41; 10.78 |
| packed, window, again | 1.59; 1.57 | 9.95; 9.95 |
| packed, window, 4 GiB map | 1.39; 1.46 | 11.11; 10.72 |

| Blocks, `pruned-or`, 5 % | p50 ms | p99 ms |
|---|---|---|
| rows, cursor | 1.88; 1.77 | 11.56; 11.19 |
| rows, window | 1.27; 1.28 | 5.16; 5.38 |
| packed, window | 1.16; 1.28 | 5.41; 5.49 |
| packed, window, 4 GiB map | 1.15; 1.15 | 5.42; 5.98 |

`rows, window, again` and `packed, window, again` are the arms of the next
pair, a minute later. The window executor opens as many blocks as the
cursor, 232 against 229 for `or`; what it saves is the work between them.
Where the window executor's time went, mean of 600 questions, `or`:

| | mean ms | reading directories | finding and unpacking blocks | the rest |
|---|---|---|---|---|
| rows, window | 3.04 | 0.87 | 1.29 | 0.88 |
| packed, window | 2.84 | 0.61 | 1.34 | 0.89 |
| every posting | 31.1–32.1 | | 8.3–8.4 | |

A block is found and unpacked in about 5.6–5.8 µs, a lookup in SQLite's b-tree
through its 1 MiB cache and the kernel's page cache. The map shortens that
little and makes the reader hold the pages it touched, ten times the memory.

## Sets

| | one set p50 ms | p99 ms | 100 a commit, ms |
|---|---|---|---|
| blocks, round 1 | 4.13 | 9.64 | 43; 47; 62; 56; 78 |
| blocks, round 2 | 4.78 | 15.02 | 72; 77; 86; 61; 79 |
| FTS5, round 1 | 3.80 | 10.03 | 21; 30; 24; 43; 26 |
| FTS5, round 2 | 3.60 | 9.44 | 33; 48; 40; 29; 51 |
| Tantivy, round 1 | 66.55 | 117.23 | 91; 81; 80; 82; 80 |
| Tantivy, round 2 | 64.30 | 95.05 | 97; 77; 76; 76; 76 |

After the sets, 600 documents twice, the blocks' `or` read at p50 4.34 ms and
p99 59.64 ms, against 2.45–2.65 and 24–25 before: a head list holds a key's
new postings unblocked under one bound, the largest among them. Their
answers stayed exact.

| Store | MiB | build s | build HWM MiB |
|---|---|---|---|
| blocks, rows: documents 666, postings 495, directory 163, terms 67 | 1,411 | 131 | 1,497 |
| blocks, packed: documents 666, postings 457, directory 103, keys 86 | 1,332 | 156 | 1,747 |
| FTS5: documents 666, postings with positions 511 | 1,240 | 122 | 728 |
| Tantivy, stored fields compressed | 706 | 86 | 1,780 |

## What this round does not show

- Phrases: the blocks keep no positions.
- A head packed into blocks while reads go on, merges of blocks, deletes
  reclaimed: the head was only written to.
- Readers at once, a reader beside a writer, keyword queries, typos.
- A second corpus, a second machine; no significance test, the answers are
  deterministic.
- Superblocks, variable blocks, tighter bounds kept beside the safe ones, and
  documents reordered: not tried.

## What follows

Blocks of our own in the store's file answer a million documents exactly,
with FTS5's memory, its file and a set in about 4 ms. In one session with
FTS5 and Tantivy the cursor executor answered 13–44 times sooner than FTS5 at
the median and 10–28 times at p99, and 5–7 times later than Tantivy; the
window executor, in an A/B of its own, then cut the cursor's p99 by a third
to a half and its median by a fifth to a third, to a p50 of 0.9–1.9 ms and a
p99 of 3.4–12 ms. Placed beside Tantivy's numbers of the other session that
would be 2–5.5 times Tantivy, an estimate across sessions that the next
round measures in one. The combination worth carrying on is the window executor over
packed blocks read through SQLite's cache; the map is not, for memory's sake.
Before a book: a head kept small by packing it into blocks in the background,
phrases or a rule that sends them to FTS5, and this round again on a second
corpus and machine.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop's WSL2 VM on an AMD
Ryzen 7 7700, the container limited to 2 CPUs and 6 GiB; Rust 1.99.0;
rusqlite 0.40.2 with SQLite 3.53.2, every SQLite file opened as TinyStore
opens one (4 KiB pages, WAL, `synchronous=full`, a 1 MiB cache); Tantivy
0.26.2. The corpus of search-1m-2026-10-16, checked against
[corpus.sha256](../search-plan-lab/corpus.sha256) and
[miracl.sha256](../search-plan-lab/miracl.sha256).

| Runs, UTC | Harness | Script | Data |
|---|---|---|---|
| 09:51:26–10:17:14 | `2b23039` | [run-blocks.sh](../search-plan-lab/run-blocks.sh) | builds, checks, queries, writes |
| 10:18:39–10:23:12 | `937b618` | [run-window.sh](../search-plan-lab/run-window.sh) | window-* |
| 10:24:41–10:28:54 | `4e94f8e` | [run-packed.sh](../search-plan-lab/run-packed.sh) | packed-builds, packed-checks |
| 10:30:24–10:31:16 | `c1a6b36` | [run-packed-reads.sh](../search-plan-lab/run-packed-reads.sh) | packed-queries |
| 10:32:16–10:33:07 | `8c20d57` | [run-mmap.sh](../search-plan-lab/run-mmap.sh) | mmap-queries |

run-packed.sh's own reads of the rows format failed on an index built before
the packed tables existed, unseen through a pipe; `c1a6b36` read both formats
again. Every table above is in [the measurements](search-blocks-measurements-2026-10-16.md),
written by `summarize-blocks.py` from [the raw lines](data/search-blocks-2026-10-16/).

```sh
MSYS_NO_PATHCONV=1 docker run --rm --cpus 2 --memory 6g -v <repo>:/src \
  -v search-plan-cargo:/usr/local/cargo/registry -v search-plan-target:/target \
  -v search-plan-perf:/perf -e CARGO_TARGET_DIR=/target rust:1.99 \
  sh -c 'cd /src/tinystore/search-plan-lab && sh run-blocks.sh && sh run-window.sh \
    && sh run-packed.sh && sh run-packed-reads.sh && sh run-mmap.sh'
python tinystore/search-plan-lab/summarize-blocks.py tinystore/reports/data/search-blocks-2026-10-16
```
