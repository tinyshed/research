# Pingmon's metrics.db, and records' search by trigrams — 27 October 2026

A real metrics store, Pingmon's, took 2.69 bytes a sample on its host. Its
times took more than half: ping's times are when each answer came, a few
milliseconds either side of a second, and a clock kept each step as a
two-byte varint. Four changes to the metrics engine take a replay of the same
934,779 samples from 2.721 to 1.192 bytes a sample, every value and time
read back bit for bit. A fifth, a trigram filter of each records block, did not
speed the rare id of issue 13's test and was not committed.

## The corpus and how it was replayed

Pingmon (`pingmon-bench/README.md`): six hosts pinged every second, two
gauges a host, label `target`, keep 30 days, ingested through the Python
SDK's sidecar every 10 s, TinyStore 0.3.0 on a 2 vCPU x86_64 Ubuntu VPS. The
corpus is the store from 2026-10-26 11:11 to 2026-10-27 08:57 UTC, read
through its running server by `dump.py`: 934,779 samples, private, not
committed.

The host's own file, divided read-only with `dbstat` at 08:50 UTC (930,128
samples, 611 pages): clocks 1,347,584 bytes, payloads 675,840, groups
339,968, twenty small tables and indexes 139,264; 2.69 bytes a sample. Its
WAL beside it held 4,231,272 bytes, a WAL's length at SQLite's 1,000-frame
checkpoint, which TinyStore truncates only at close.

`pingmon-bench` replays the corpus as Pingmon wrote it: the samples since
the last flush every 10 s of data, maintenance every minute, on a test clock
at the data's time, background work off. Its baseline file, 621 pages and
2.721 bytes a sample, divides as the host's did, so the replay stands for it.

Machine: AMD Ryzen 7 7700, 31 GiB, Samsung SSD 990 PRO, Windows 11 Pro, Rust
1.99.0, SQLite bundled by rusqlite 0.40.2. A file's pages are the same on any
system for the same operations, so these sizes are not Windows'; the search
timings below are.

```sh
cd tinystore/pingmon-bench
git -C ../source checkout <commit>
cargo build --release
target/release/pingmon-bench <corpus>/pingmon.csv <store>
target/release/division <store>/metrics.db 934779
```

## What each change took from which b-tree

Bytes of each b-tree, `dbstat`'s pages; a sample is one of 934,779.

| TinyStore                                 | File, B/sample | clocks    | payloads | groups  | the rest |
|-------------------------------------------|----------------|-----------|----------|---------|----------|
| `3a9bdbeb`, baseline                      | 2.721          | 1,396,736 | 679,936  | 331,776 | 135,168  |
| a clock's steps by their median, alone    | 1.827          | 561,152   | 679,936  | 331,776 | 135,168  |
| `89a08d0e`, and lasting groups at the end | 1.683          | 495,616   | 679,936  | 266,240 | 131,072  |
| `6c081bc1`, and rice-coded integers       | 1.599          | 495,616   | 602,112  | 266,240 | 131,072  |
| `d7935317`, and blocks of 320 samples     | 1.192          | 397,312   | 540,672  | 65,536  | 110,592  |

- **A clock's steps by their median.** A block's steps were kept plain, in
  runs, or as exceptions to the commonest; ping's are none of these, and plain
  spent two bytes each. Each step's distance from the block's median, ZigZag
  and rice-coded, takes 0.50 bytes a time in a model of three hours of the
  corpus, against 1.86, its entropy 3.44 bits. Clock bodies: 1,131,349 →
  375,923 bytes. Version 2 of a clock also keeps a start but the first as its
  distance from the block before's end; version 1 reads as it was written.
- **Lasting groups at the end.** A group takes in the one before while that
  holds no more blocks, so a series' rows are rewritten 1, 2, 4 … 32 blocks
  long, and the short-lived rows shared pages with the 32-block ones that
  stay: a third of the clocks' and groups' pages was empty. A group of 32
  blocks now goes after every row of its table, appended, and one still
  growing below zero: groups' unused bytes 103,195 → 38,017, clocks' 169,291 →
  104,012. What is left in the clocks' pages is rows of 1.2 KiB laid three to
  a page.
- **Rice-coded integers.** A run's integers were Simple8b words of their
  deltas; the round trips' deltas wander a few units either way, which rice
  codes keep shorter. A run takes the shorter of the two: payload bodies
  594,407 → 522,131 bytes. A model of the corpus put words at 1.17 bytes a
  value, rice codes of the deltas at 1.01 and of the distance from the median
  at 0.99; the deltas were taken.
- **Blocks of 320 samples.** At 240 a block, a series of a sample a second
  took a block a minute, the ladder's next unit, 5 minutes, holding 300; each
  block paid its directory entry of about 25 bytes, its clock's extent and a
  payload row. At 320, five minutes of a sample a second fit with room for
  samples a little early, and 15,624 blocks became 3,132: groups 266,240 →
  65,536 bytes. A bucket smaller than a block now decodes its samples rather
  than answering from the block's summary: a minute's buckets of a series a
  second.

Not taken: a WAL bound below its 1,000-frame cycle. Pingmon's 4.2 MB WAL is
that cycle, and a `journal_size_limit` under it would turn the WAL's
overwrites into appends at every cycle, whose syncs carry the file's size;
nothing measured that. ALP's `rd` mode for doubles a decimal grid cannot
hold had no series to apply to: Pingmon's values are decimals and TSBS's
integers.

## What blocks of 320 cost an aggregate

`aggregate` (`pingmon-bench/src/aggregate.rs`) times `avg` and `max` of
`ping_rtt_ms` by target over the replays of `6c081bc1`, blocks of a minute,
and `d7935317`, of five, in turns, the median of 21 passes, both read by
`d7935317`'s code; `decoded` is what `explain` says each decodes.

| Range    | Every | 1 min blocks, avg | 5 min blocks, avg | samples decoded, 5 min |
|----------|-------|-------------------|-------------------|------------------------|
| an hour  | 1m    | 0.66 ms           | 1.36 ms           | 21,497                 |
| an hour  | 5m    | 0.52 ms           | 0.69 ms           | 0                      |
| an hour  | 1h    | 0.48 ms           | 0.64 ms           | 0                      |
| 20 hours | 1m    | 12.36 ms          | 19.97 ms          | 429,586                |
| 20 hours | 5m    | 7.86 ms           | 5.05 ms           | 0                      |
| 20 hours | 1h    | 7.32 ms           | 4.58 ms           | 0                      |

A minute's buckets of a series a second now decode its samples, twice the
time over an hour and 1.6 times over twenty; buckets of five minutes and
more read a fifth of the summaries, and over twenty hours take two thirds of
the time. `max` moves the same way. Nothing decoded on the minute blocks,
which answered every bucket from summaries.

## Records: a block's trigrams

A block's trigrams, two bits a distinct trigram, set from the texts a search
reads in lower case, was built and measured, and not committed: its patch is
`records-grams/grams.patch`. On LogHub's 2k samples a block of 1,024 lines
holds 1,100 to 5,400 trigrams, a filter of 1 to 10 % of the block under zlib.

The timing test of issue 13 (`a_search_of_ten_million_lines_a_page_after_another_and_read_ahead`,
10 million HDFS-like lines, the median of 7 passes), without and with the
filter in turns of one build each:

| Read                  | Found     | Without, read ahead | With, read ahead |
|-----------------------|-----------|---------------------|------------------|
| a rare block id       | 12        | 527 ms              | 516 ms           |
| its first 100 lines   | 12        | 834 ms              | 852 ms           |
| a common word counted | 1,999,374 | 2,123 ms            | 1,746 ms         |

The rare id passes no fewer blocks: `blk_` and 19 digits, and a block of
1,024 lines each naming such an id holds nearly every one of the 1,000
three-digit trigrams, so every block may hold any id; hex ids fare the same.
The filters took 9,854 pages, 10,090,496 bytes, 0.60 bytes a line of payload
and 38 % of their pages empty, of a 274,880,512-byte file. A rare word would
pass few blocks; none was measured, so the cost has nothing yet to weigh it
against. The common word's difference lies within the spread of the passes
(1,977–2,197 against 1,703–2,325 ms).
