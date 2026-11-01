# search-fast: where the compact blocks' time goes on MS MARCO, and what a layout for reading buys

Written on 1 November 2026 after the counts and a profile on a busy machine,
before any time on a quiet one. The question:
[search-msmarco-timing-2026-11-01](../../../reports/search-msmarco-timing-2026-11-01.md)
found the compact blocks' p99 3.0–3.5 times Tantivy's and their p50 4.8
times. Where does a query's time go, and how much of it does each change
take away, every answer still exact?

## What is built

All in this crate, all reading the same `compact.db` the earlier round built;
nothing of the lab's sources changes.

- **`fast` (src/fast.rs)**, compact1's reader with switches, every one off
  giving compact1's own blocks opened on every query (checked: 4,503,341 of
  4,503,341 on the sample):
  - `FAST_BOUNDS=front`: a block's bound is its exact best, from the block's
    Pareto front of (frequency, length), kept in `fast.db`; compact's own
    bound takes the largest frequency and the shortest field, which may
    belong to different postings.
  - `FAST_SEED=1`: θ starts at the best of the query's keys' tenth-best
    scores, from each key's ten best postings kept in `fast.db`.
  - `FAST_NORMS=flat`: the field's lengths as one u16 array, where compact1
    keeps four fields' u16 interleaved in chunks of 65,536 documents.
  - `FAST_READ=blob`: a block read through one incremental-blob handle moved
    from row to row, from a rowid copy of the blocks, instead of a
    statement.
- **`fast2` (src/fast2.rs)**, the same blocks laid out for reading over
  ordinary tables of `fast2.db`: one row a key holding its blocks
  bit-packed one after another and read at their offsets through one
  blob handle a list; a fixed-width directory of 16 bytes a block whose
  bound is one f32 rounded up; one byte of length a document with the few
  of 255 or more apart; and, under `FAST2_TABLE=1`, each list's scores of
  frequency below 16 and length below 255 tabled when it opens.

`fast2` reads no head and no gone: it measures reads of a settled index, as
the timing harness does; the writes are compact's and `fast`'s.

## Hypotheses

- **F1, the lengths**: compact1's interleaved lengths cost more than any
  other part of decoding; the flat array takes most of it away.
- **F2, the bounds**: exact block bounds open fewer blocks. Counted on the
  sample: 13 % fewer; the seed 6 % fewer; both 18 %.
- **F3, the statements**: a blob handle costs less than a statement a block.
- **F4, the layout**: `fast2` answers the sample within two times Tantivy at
  p99 and p50 — H2.1's bar — on this machine.

A switch's effect is the difference between arms measured in the same run,
interleaved; an explanation of why is a hypothesis until the profile or a
second arm separates it.

## Exactness

Every arm's ids and scores against compact's answers on the sample, which
the earlier round found exact against scoring every posting: the scores
equal within 1e-9 and documents differing only where scores tie, as
evaluate.py defines it.

## Run

```sh
# once, after round.sh built compact.db
docker run --rm -v search-msmarco-target:/target -v search-msmarco-perf:/perf rust:1.99 \
  /target/release/msmarco fast-build /perf/msmarco/compact.db
docker run --rm -v search-msmarco-target:/target -v search-msmarco-perf:/perf rust:1.99 \
  /target/release/msmarco fast2-build /perf/msmarco/compact.db
# on a quiet machine
sh tinystore/bench/search-msmarco/fast/timing.sh
# the profile, a query's parts
docker run --rm --cpus 2 --memory 6g -v search-msmarco-target:/target -v search-msmarco-perf:/perf \
  -e FAST_BOUNDS=front -e FAST_SEED=1 -e FAST_NORMS=flat -e FAST_READ=blob rust:1.99 \
  /target/release/msmarco profile /perf/msmarco/compact.db /perf/msmarco/q/sample.tsv <out>
```
