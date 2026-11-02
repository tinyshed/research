# TinyStore's searches on MS MARCO made fast, and a live index timed — 2 November 2026

**The harness is [bench/search-msmarco](../bench/search-msmarco/) with
`tiny/` beside it; TinyStore at `60a5903c`, its `main`, against
`d2e65efd` before it.** [search-tinystore-2026-11-02](search-tinystore-2026-11-02.md)
built TinyStore's own engine on the 8,841,823 passages and found its searches
1.4 times as slow as `fast4-cache`, the prototype the engine was built from,
at the median, and 2.1–2.5 times Tantivy's. This round times the index as a
program has it, its segments as background work left them, then finds where
a search's time goes beside the prototype's and takes it away a change at a
time, every answer the same.

| Question | Answer | What follows |
|---|---|---|
| Does a live index search slower than one merged into one segment? | **Little**: at `d2e65efd`, 22 segments in four levels took p50 5.51–5.89 ms against 5.04–5.72, p99 33.5–35.9 against 33.7–35.2; at `60a5903c`, p50 3.43–3.81 against 3.26–3.51, p99 21.9–25.0 against 21.0–22.7 | — |
| Where did a search's time go beside the prototype's? | Around the search: a block's values read through a bit reader that refilled a value at a time, 2.5 times the prototype's unpacking; every posting of every block opened given its length, though a query scores about a quarter of them, in a fetch that took 21 % of the time; a window's bounds and starts searched for, its documents sorted, its buffers allocated | — |
| How fast now? | **p50 3.26–3.51 ms, p99 21.0–22.7**: 1.37–1.49 times Tantivy's p50 and 0.74–0.82 times its p99, from 2.16–2.49 and 1.22–1.32; under `fast4-cache`'s 3.82–3.94 and 27.7–29.0 in the same run | the per-candidate work below |
| The same answers? | **Yes**: 1,093 of 1,097 sample queries the same ten as compact's in the same order, 10,967 of 10,970 documents, as at `d2e65efd`; every query's ten the same as `d2e65efd`'s, in order, merged and live | — |
| Against PISA and Lucene? | Still behind at both: PISA's p50 2.33–2.70 and p99 15.2–15.8, Lucene's 1.70–1.85 and 11.9–13.3 | — |

## A live index

`tiny/round.sh live` builds the index as `build` does, two sets in flight
while background work merges, and leaves it there, `TINY_MAINTAIN=0`, rather
than merging it into one segment
([build-tinystore-live.jsonl](data/search-tinystore-queries-2026-11-02/logs/build-tinystore-live.jsonl)):
272 s, the segments two of level 0 with 1,823 passages, four of level 1, eight
of level 2 and eight of level 3. Its pieces take 232.5 MB of pages rather than
the merged index's 135.1 MB; its runs 528.7 MB rather than 520.6.

A search reads a key's piece in each of the 22 segments rather than in one,
and joins their blocks into one list. The timing below,
`timing/live-before` and `timing/after`, puts the live index's p50 at 1.03–1.13
times the merged one's round by round, and its p99 the same within the
intervals; its ten are the merged index's for every query.

## Where the time went

`perf` in a container of the same shape over a warm pass of the sample
([profile-d2e65efd.txt](data/search-tinystore-queries-2026-11-02/logs/profile-d2e65efd.txt),
[profile-fast4-cache.txt](data/search-tinystore-queries-2026-11-02/logs/profile-fast4-cache.txt)),
in samples of each, the two runs at the same rate:

| | TinyStore `d2e65efd` | `fast4-cache` |
|---|---|---|
| the search's loop, scoring inlined | 13,042 | 15,642 |
| a block fetched: its postings given their lengths | 11,793 | 679 |
| a block unpacked | 5,574 | 2,267 |
| the kernel's copies of SQLite's pages: `_copy_to_iter`, `filemap_*`, `xas_load` | 3,537 | 3,288 |
| all | 57,053 | 30,025 |

The engine's search was the prototype's; what it did around it was not. A
block's values went through `compress`'s bit reader, which refills a word a
value; every block a list opened had each posting's length looked up and
written, though a passive list opens a block for one document; each window
searched every list's blocks for its bounds and its next start, sorted the
documents it touched and allocated two vectors.

Counted in a build with counters added, per query over the sample at the
step before the last ([counts.txt](data/search-tinystore-queries-2026-11-02/logs/counts.txt),
columns in this order): 62,946 blocks in the query's lists on average, 2,159
windows, 2,862 blocks unpacked, 91,950 postings scored a window at a time,
52,233 documents completed from the passive lists, 45,351 of their
postings sought, 5.8 lists; building the lists took 4.4 % of the time.

## The changes

Each a build of the engine with the change, timed by passes of `msmarco time
tinystore` over the sample, warm, a 2-CPU container each, the variants
alternating, two rounds unless said; quick runs on this machine, p50 and p99
in ms, each against its predecessor in the same run:

| change | p50 | p99 |
|---|---|---|
| `d2e65efd` | 5.23–5.61 | 34.3–34.5 |
| a block's values read as one 8-byte load each, a shift and a mask; a list's block read into the last one's room; a window's bounds and start stepped to from the last window's; the documents it touched marked in a bitset and read in their order, not sorted; points rounded by a cast rather than libm's `round` | 4.36–4.60 | 26.5–27.1 |
| a posting's length read only when it is scored: a block a window scores whole at once, a document a passive list is asked for alone | 3.27–3.57 | 24.1–24.6 |
| a block unpacked straight into postings | 3.21–3.44 | 23.7–23.9 |
| a candidate completed as Lucene does, the window's most of the lists not asked yet and the block's bound of the one asked, rather than every list's block at the document first; four rounds | 2.89–3.24 | 20.2–21.6 |

`fast4-cache` in the same runs as the second and third: 3.62–3.72 and
3.60–3.66 at p50, 28.5–29.0 and 27.3–27.6 at p99.

Tried and not taken, each within the runs' spread of what it replaced:

| variant | against | result, four rounds |
|---|---|---|
| windows of 8,192, 16,384 or 32,768 documents rather than 4,096 (two rounds) | 4,096 | p50 3.21–3.28, 3.00, 3.08–3.42 against 3.09–3.15 |
| windows of 2,048; of 1,024 | 4,096 | 3.01–3.25 against 3.06–3.30; 1,024 slower, 3.57–4.27 |
| a block unpacked by a function a width, its places and mask constants | 8-byte loads | mean 4.37–4.66 against 4.50–4.63 ms |
| a document sought in its block by bisection | stepping | mean 4.83–5.27 against 4.84–5.10 |
| readers' page cache of 16 MiB rather than 1 | 1 MiB | mean 4.87–5.05 against 4.84–5.10 |
| search.db mapped, `mmap_size` 8 GiB | read | mean 4.68–4.90 against 4.84–5.10 |

Mapping the file may take 3 % off the mean, and is a choice of how the store
holds memory rather than of search; it was not taken.

## Timing

`round.sh timing`, the arms in one order and then the other, four rounds,
each pass a container of 2 CPUs and 6 GiB, warm, one query at a time, from
08:20 to 08:27 UTC
([summary.md](data/search-tinystore-queries-2026-11-02/timing/after/summary.md)):

| arm | p50 | p99 | p50 / Tantivy | p99 / Tantivy |
|---|---|---|---|---|
| TinyStore `60a5903c` | 3.26–3.51 | 21.0–22.7 | 1.37–1.49 | 0.74–0.82 |
| TinyStore `60a5903c`, live | 3.43–3.81 | 21.9–25.0 | 1.47–1.61 | 0.73–0.92 |
| `fast4-cache` | 3.82–3.94 | 27.7–29.0 | 1.60–1.66 | 0.92–1.02 |
| compact blocks | 11.83–12.29 | 92.1–98.1 | 4.94–5.08 | 3.25–3.48 |
| Tantivy | 2.34–2.47 | 27.3–30.2 | 1 | 1 |
| PISA `block_max_maxscore` | 2.33–2.70 | 15.2–15.8 | 0.98–1.14 | 0.51–0.57 |
| Lucene | 1.70–1.85 | 11.9–13.3 | 0.72–0.79 | 0.42–0.49 |

And at `d2e65efd`, from 07:27 to 07:31 UTC
([summary.md](data/search-tinystore-queries-2026-11-02/timing/live-before/summary.md)):

| arm | p50 | p99 | p50 / Tantivy | p99 / Tantivy |
|---|---|---|---|---|
| TinyStore `d2e65efd` | 5.04–5.72 | 33.7–35.2 | 2.16–2.49 | 1.22–1.32 |
| TinyStore `d2e65efd`, live | 5.51–5.89 | 33.5–35.9 | 2.38–2.56 | 1.21–1.36 |
| Tantivy | 2.30–2.33 | 26.4–27.7 | 1 | 1 |

Every arm's ids were the same in all four of its passes. TinyStore reads
each hit's value too, which no other arm does.

## What is left

The counts put most of a query in its candidates: 52,233 documents
completed and 45,351 postings sought in passive lists, against 91,950 scored
a window at a time. Lucene answers the same queries by the same algorithm in
0.52–0.53 of the time at p50 and 0.56–0.59 at p99, round by round; its postings are decoded
by the JDK's vector API here and its lengths are a byte a document, and how
much of the gap each holds is not measured.

## Limits

- One machine, in WSL2's VM: a time is this machine's, and the ratios in one
  run are what the round can say. `bootstrap.py` heads each summary "not a
  measurement" for the label `quiet machine, WSL2 container`, as in the
  rounds before.
- The changes' and the variants' times are quick runs of two or four rounds,
  each against the build before it in the same run; only the two timings
  above are the harness's.
- The live index is one build's segments; another build, or a program that
  sets at another pace, leaves others.
- The counts come from a build with counters added, at the step before the
  last change, and the counters' own cost is in its times.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop 29.6.2's WSL2 VM,
16 CPUs and 15.2 GiB, on an AMD Ryzen 7 7700 under Windows 11 Pro
10.0.26300; `rust:1.99`, each container 2 CPUs and 6 GiB; TinyStore's
bundled SQLite without its `LIBSQLITE3_FLAGS`, as every SQLite arm here.

```sh
# from <repo>, after search-tinystore-2026-11-02's build
export DATA=tinystore/reports/data/search-tinystore-queries-2026-11-02
sh tinystore/bench/search-msmarco/tiny/round.sh tools
sh tinystore/bench/search-msmarco/tiny/round.sh live
sh tinystore/bench/search-msmarco/tiny/round.sh answers
STAMP=after ROUNDS=4 ARMS="compact fast4-cache tinystore tinystore-live tantivy pisa-block_max_maxscore lucene" \
  LABEL="quiet machine, WSL2 container" sh tinystore/bench/search-msmarco/tiny/round.sh timing
```

`timing/live-before` ran the same at `d2e65efd` with `ARMS="tinystore
tinystore-live tantivy"`. Raw output, every pass's query ids and
nanoseconds, is in [timing/](data/search-tinystore-queries-2026-11-02/timing/).
