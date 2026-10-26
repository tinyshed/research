# TinyStore 0.3.0: aggregating what is never stored, 27 October 2026

Chapter 5's calculations sum, count and take a last value over rows the hub
reads live and never keeps. Some of the same numbers are kept as datasets in
TinyStore Metrics, and a panel reading them live should agree with one
reading the dataset. The question was whether a caller can get Metrics'
answer without storing the samples.

| Question | Answer | What follows |
|---|---|---|
| Ten readings of 0.1, summed | by hand `0.9999999999999999`; Metrics `Exact(1.0)`, rounded once | a float sum needs Metrics' accumulator, or a compensated sum of our own |
| A counter 100, 150, 200, reset to 20, then 70 | by hand, last less first, `-30.0`; Metrics' increase `Exact(170.0)`, a reset counted as its new value | increase and rate are not a subtraction |
| What getting Metrics' answer takes | a store opened, the samples ingested and read back: 24.5–30.6 ms, and `LOCK` and `metrics.db` with its WAL left on disk | asked as [tinyshed/tinystore#34](https://github.com/tinyshed/tinystore/issues/34): the same aggregation over samples held in memory, nothing written |

## Environment and reproduction

- `tinystore = "=0.3.0"` from crates.io, feature `metrics`.
- Windows 11, Rust 1.97.1, Ryzen 7 7700, release build; three runs, the
  numbers the same each time.
- The program was committed before it ran: 679915b.

```sh
cd <research>/dashbin/tinystore-repro/0.3.0
cargo build --release --bin issue_aggregate_in_memory
./target/release/issue_aggregate_in_memory
```

Raw output: [data/tinystore-0.3.0-aggregate-2026-10-27/outputs.txt](data/tinystore-0.3.0-aggregate-2026-10-27/outputs.txt).

## What follows

Until #34 is answered, chapter 5's kernel sums integers and decimals as
integers, which is exact anyway, and floats with a compensated sum, and says
where that can differ from Metrics; increase and rate over live readings wait
for TinyStore's own code rather than a second copy of it.
