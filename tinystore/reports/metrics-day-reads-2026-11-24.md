# What a day of metrics costs to take from the file — 24 November 2026

**The harness is [metrics-day-bench](../metrics-day-bench/); TinyStore at
`731d8298`, its `main`, against `9d6eb149` before it.** The
[round before](metrics-day-2026-11-24.md) left a day by the hour at 20 ms on
Windows on noisy counters, about half of it taking 600 series out of the
file, and named reading series in batches as the next step. This round splits
that half by what it reads, batches what costs a statement a block, and finds
that most of what is left is SQLite asking the system for its pages.

| Question | Answer | What follows |
|---|---|---|
| What did taking a series from the file spend? | On Windows, of a day by the hour's 11 ms: the blocks' bodies 5.1, the groups' rows 1.9, their directories 1.7, the series' state 1.1, finding the series 0.8, clocks 0.6 | The bodies were a statement each, 4,200 a chart |
| Do batched bodies pay? | **A little**: the SQL of the bodies went from 5.1 to 4.3 ms; with a directory's sums checked without building them, a day by the hour is 0.92–0.94 of what it was in the container, 0.93–0.98 on Windows | Built at `47d050d3` and `731d8298` |
| What do the bodies still cost? | SQLite reading their pages from the system: a reader's cache is 1 MiB and a day's read touches several | A page cache of 16 MiB took the bodies from 4.3 to 2.0 ms on Windows |
| Should a reader's cache grow, or SQLite map the file? | **Not for this**: either gives about 7 % in the container and about 20 % on Windows, and the bench's whole file fits a cache of 8 MiB where a dashboard of many names does not | Left as it is; the costs are below |

## Environment and reproduction

- TinyStore `731d8298` in `tinystore/source`, and `9d6eb149` in a worktree of
  it at `tinystore/base`; the harness at research `d6639bc`, unchanged.
- Windows 11 Pro 10.0.26300, AMD Ryzen 7 7700, rustc 1.99.1; and the image
  `rust:1.99`, rustc 1.99.1, under Docker 29.6.2 on WSL2's kernel 6.18.33.2
  on the same machine, 16 processors.
- The data and the reads are the round before's: 600 counters and 600
  gauges of a sample a minute for two days, each read 31 times, a cell the
  least time of each of two passes, `first; second`.
- The passes ran in turn, base then candidate, twice, noisy then ramp, on 23
  November from 22:39:02 to 22:40:03 UTC on Windows and from 22:40:32 to
  22:41:21 in the container, with nothing else running.

The commands are the round before's, with `9d6eb149` as the base:

```sh
git -C tinystore/source worktree add ../base 9d6eb149
cd tinystore/metrics-day-bench
(cd base && cargo build --release) && (cd candidate && cargo build --release)
for data in noisy ramp; do for pass in 1 2; do for side in base candidate; do
  $side/target/release/metrics-day-bench-$side $data 31
done; done; done
```

The raw output is [windows.txt](data/metrics-day-reads-2026-11-24/windows.txt)
and [linux.txt](data/metrics-day-reads-2026-11-24/linux.txt).

## Where taking a series went

The split below is not a measurement by this repository's rules: it comes
from a build of `9d6eb149` with timers around each part of a series' read,
never committed, run on Windows on noisy counters, the timers' sums over one run of
the read. It says where to look, and the timings after it
say what the change did.

| Part, a day by the hour | µs |
|---|---|
| the blocks' bodies, a statement each | 5,135 |
| the groups' rows, a statement a series | 1,906 |
| the directories decoded | 1,721 |
| the series' state, a statement a series | 1,057 |
| finding the series by their labels | 776 |
| clocks, read or kept | 582 |
| counting what was taken, outside the file | 10,227 |

## What was changed

- **Bodies copied last** (`47d050d3`). A block's body was copied as its
  block was taken, a statement of `payloads` by its id. Now a snapshot takes
  every series' blocks first, paid for as before, and copies the bodies of
  those it decodes at the end of the same snapshot, a statement for up to
  1,024 of them in the order of their ids. Every snapshot hands back what it
  took through one trait, so no read can leave a body behind; a body missing
  or of another length still names its series and suspends it. With the same
  timers the bodies' SQL went from 5,135 to 4,267 µs: the statements were the
  smaller part of what a body cost.
- **A directory's sums checked, not built** (`731d8298`). A directory checks
  every block's exact sums for their one spelling, and built each into a sum
  of 35 words to do it, which callgrind put at 13.6 % of taking the series.
  The spelling is now parsed once, checked by its bytes, and built into a sum
  only where a block's summary is read.

TinyStore's 296 tests of metrics pass at both, and four new cases hold the
spelling's bound across two bytes.

## How fast now

The least time of 31 runs, milliseconds, `first pass; second pass`.

### The Linux container

| Read | noisy, `9d6eb149` | noisy, `731d8298` | ramp, `9d6eb149` | ramp, `731d8298` |
|---|---|---|---|---|
| increase, one number | 7.85; 7.75 | 7.10; 7.39 | 6.86; 6.90 | 6.39; 6.41 |
| increase by the hour | 16.88; 16.87 | 15.58; 15.81 | 13.35; 13.26 | 12.11; 12.24 |
| increase by 5 minutes | 39.52; 40.76 | 37.93; 37.49 | 35.08; 34.43 | 33.12; 33.24 |
| increase by the day, Moscow | 8.35; 8.86 | 7.77; 7.95 | 7.56; 7.48 | 7.04; 7.10 |
| avg by the hour | 19.93; 21.09 | 18.69; 19.37 | 12.95; 13.04 | 12.18; 12.22 |
| max by the hour | 18.73; 19.21 | 17.95; 18.72 | 11.83; 12.04 | 11.19; 11.31 |

### Windows

| Read | noisy, `9d6eb149` | noisy, `731d8298` | ramp, `9d6eb149` | ramp, `731d8298` |
|---|---|---|---|---|
| increase, one number | 11.42; 10.82 | 10.83; 10.82 | 7.57; 7.56 | 7.49; 7.30 |
| increase by the hour | 21.77; 20.24 | 20.33; 19.74 | 14.23; 14.50 | 13.44; 13.22 |
| increase by 5 minutes | 38.17; 37.94 | 38.43; 37.96 | 31.65; 31.67 | 31.40; 31.15 |
| increase by the day, Moscow | 11.31; 10.85 | 10.90; 10.50 | 8.45; 8.58 | 7.85; 7.73 |
| avg by the hour | 24.35; 23.80 | 23.64; 22.29 | 14.19; 14.46 | 13.35; 12.65 |
| max by the hour | 23.53; 22.31 | 22.14; 21.34 | 13.08; 13.27 | 11.99; 12.13 |

The base's first noisy pass on Windows is slower than its second in every
read; against the second alone a day by the hour moved from 20.24 to 19.74
and 20.33, within what two passes of one build differ by. On ramps and in the
container every read moved, by 6 to 9 %.

## The page cache

Each reader of a file keeps 1 MiB of SQLite's pages. The bench's
`metrics.db` holds about 8 MB for both names, and a day of one name touches
the bodies of 4,200 blocks, about 1.7 MB on noisy counters, beside the
groups' directories and the series' rows, so every read asks the system for
nearly every page again. The timings below come from the same kind of build
as the split above, with two switches never committed: the metrics readers'
cache in KiB, and SQLite's `mmap_size` for every reader. They are not
measurements by this repository's rules, only the reason nothing was built.

| A day by the hour, noisy | Windows | The container |
|---|---|---|
| a cache of 1 MiB, as built | 20.80 ms | 15.18; 15.28 ms |
| a cache of 8 MiB | 15.91 ms | 14.14; 14.12 ms |
| a cache of 16 MiB | 16.62 ms | — |
| 1 MiB and a map of 256 MiB | 17.10 ms | 14.17; 14.21 ms |

With a cache of 16 MiB the timers put the bodies' SQL at 1,954 µs and the
groups' rows at 911, from 4,267 and 1,906.

- **A larger cache** helps here because this whole file fits 8 MiB. A
  dashboard reads many names, on whichever readers are free, each with a
  cache of its own: its pages would not fit 8 MiB either, and the readers,
  up to 16 a file, would hold eight times the memory they hold now while
  they last.
- **A map of the file** reads pages without a call to the system whatever
  their number, but on Windows a file another connection maps cannot be
  truncated, and SQLite then leaves it as long as it was: the pages metrics
  gives back to the disk would stay with the file while a reader is open,
  and one always is.
- On Linux either gains about 7 %, where a server runs, against about 20 %
  on Windows, whose calls to read a page cost more.

## What follows

- `47d050d3` and `731d8298` are on TinyStore's `main`. The
  [round before](metrics-day-2026-11-24.md) and this one together take a day
  by the hour from 29.0–29.1 ms to 15.6–15.8 in the container on noisy
  counters, in two sessions whose bases are a day apart, so the product of
  the two ratios, 0.53–0.56, is the figure to quote, not the difference.
- **No cache and no map**, for the reasons above, unless a measurement of a
  real store's dashboard shows its pages fit.
- What is left of taking a series is small statements and a directory of
  32 blocks decoded for the six or seven a day needs; what is left of the
  read is decoding and counting 936,000 samples, about 12 ns each in the
  container, by the difference between a day by the hour and the day as one
  number.
