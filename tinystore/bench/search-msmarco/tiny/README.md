# search-tinystore: TinyStore's own search on MS MARCO

Written on 2 November 2026, before any time was taken. The rounds of
1 November measured prototypes: the lab's compact blocks, and `fast2`,
`fast3` and `fast4`, the same blocks bit-packed in one row a key and read
through a blob handle. TinyStore's `main` has since built that layout in
its own engine: bit-packed blocks in runs of 32 KiB, a fixed-width
directory bounded by BM25's saturation, blob reads, the walk of passive
lists, directories kept between searches and a key's best ten as a floor.
This round measures that engine, through its public API, beside the arms
it was built from.

Changed after the hypotheses were written, before any search was timed: a
first build, at `a5143eb1`, packed each key's postings onto its list where
it lay, and each million passages took longer than the one before (292,
475 and 659 s); it was stopped. The engine since keeps postings in
segments a set writes one after another and merges join (`a3f9f915`), and
the build below is of that engine.

## What is built

- `src/tiny1.rs`: an index of the token stream, a passage a document whose
  value is its tokens, set a thousand a commit while the store's background
  work merges their segments, as a program's does, then merged into one
  segment by `search::maintain`; durability to the operating system, as the
  other builds. A search is the query's terms joined by spaces, the ten
  hits read with their values, as every TinyStore search reads them. Typo
  correction is off, as no other engine corrects.
- `tiny/round.sh`: the steps, `tools`, `build`, `answers`, `timing`.
- `tiny/agree.py`: TinyStore's ten against compact's on the sample.

TinyStore analyzes the tokens again; they are lower-case words already,
which it keeps as they are, and it stems no English, so its index holds
the stream as the others do. Its BM25 keeps scores in whole points of
2⁻³², so its ten may order ties differently.

The bundled SQLite is the harness's, built without TinyStore's
`LIBSQLITE3_FLAGS`, as every other SQLite arm here: the store logs once
that they are missing.

## Hypotheses

- **T1, the engine keeps the prototype's speed**: TinyStore's p99 within
  1.5 times `fast4-cache`'s on the sample, in the same run.
- **T2, near Tantivy**: TinyStore's p99 within two times Tantivy's, H2.1's
  bar, on this machine.
- **T3, the space**: what a search reads, runs, lists and keys, at most
  2.5 bytes a posting.
- **T4, the answers**: TinyStore's ten share at least 95 % of compact's
  documents on the sample.

## Run

```sh
sh tinystore/bench/search-msmarco/tiny/round.sh tools
sh tinystore/bench/search-msmarco/tiny/round.sh build
sh tinystore/bench/search-msmarco/tiny/round.sh answers
LABEL="quiet machine, WSL2 container" sh tinystore/bench/search-msmarco/tiny/round.sh timing
```
