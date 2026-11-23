# A day of metrics by the hour, where its time went and what took it away — 24 November 2026

**The harness is [metrics-day-bench](../metrics-day-bench/); TinyStore at
`9d6eb149`, its `main`, against `8cca207e` before it.**
[tinystore#96](https://github.com/tinyshed/tinystore/issues/96) found that a
chart of a counter's last day by the hour decodes every sample of the day,
since a block of a series written once a minute holds four hours and a
summary answers only a block inside one bucket, and asked for the read to
cost what the day as one number costs. This round finds where the read's
time goes, tries the cut the issue proposed, and takes away what the profile
showed without changing a byte of the file.

| Question | Answer | What follows |
|---|---|---|
| Does cutting a minute's series by the hour pay? | **No**: the file grew from 0.60 to 1.42 B a sample on ramps and from 2.86 to 3.23 on noisy counters, the day by the hour went from 23.5 to 20.1 ms, and the day as one number from 11.1 to 18.6 | The ladder stays; a summary of a block costs about what decoding its hour does, so summaries by a finer step would save little either |
| Where did a day by the hour spend its time? | Of the instructions of seven reads: counting the samples 37 %, decoding them 28 %, taking the series from the file 20 %, joining buckets 9 % | Counting and decoding were the two to take away |
| How fast now? | **A day by the hour 33.0–34.8 → 20.2–21.0 ms** on Windows and 29.0–29.1 → 16.6–17.2 in the container on noisy counters, 23.4–24.0 → 14.5–14.6 and 21.6–22.3 → 13.4–13.7 on ramps: 0.57–0.64 of what it took | Built at `d57dfeae`, `b2734b6c`, `9d6eb149` |
| What else moved? | A day by a calendar's day 21.6–22.0 → 11.4–12.2 ms on Windows on noisy counters, 0.53–0.55 of it, and 0.48–0.49 on ramps; a gauge's average by the hour 0.65–0.76; the day as one number 0.74–0.79; a day by five minutes 0.79–0.84 on Windows and 0.88–1.02 in the container | Five minutes is the buckets' own cost, which this round left |
| What is left? | Of 20 ms about half is taking 600 series from the file, a few statements a series | The next step is reading series in batches, not the format |

## Environment and reproduction

- TinyStore `9d6eb149` in `tinystore/source`, and `8cca207e` in a worktree of
  it at `tinystore/base`; the harness at research `d6639bc`.
- Windows 11 Pro 10.0.26300, AMD Ryzen 7 7700, rustc 1.99.1; and the image
  `rust:1.99`, rustc 1.99.1, under Docker 29.6.2 on WSL2's kernel
  6.18.33.2 on the same machine, 16 processors, its stores in the
  container's own file system.
- 600 counters and 600 gauges, a sample a minute each for two days, at a
  second of the minute by the series, written an hour at a time and packed.
  `noisy` counters grow by a random step up to 5,000 and gauges are random
  eighths; `ramp` counters grow by one step and gauges count the minutes of
  the hour. A read is run 31 times, and a cell below is the least time of
  each of two passes, `first; second`. The day read holds 864,000 samples of
  a name; a read's `decoded` is what `explain` says of it.
- The passes ran in turn, base then candidate, twice, noisy then ramp, on 23
  November from 22:01:19 to 22:02:28 UTC on Windows and from 22:04:22 to 22:05:19
  in the container, with nothing else running.

```sh
git -C tinystore/source worktree add ../base 8cca207e
cd tinystore/metrics-day-bench
(cd base && cargo build --release) && (cd candidate && cargo build --release)
for data in noisy ramp; do for pass in 1 2; do for side in base candidate; do
  $side/target/release/metrics-day-bench-$side $data 31
done; done; done
```

In the container, from Git Bash with `MSYS_NO_PATHCONV=1`:

```sh
docker run --rm -v <research>:/src -v tinystore-cargo:/usr/local/cargo/registry -v tinystore-target:/target \
  -w /src/tinystore/metrics-day-bench rust:1.99 sh -c '
  (cd base && CARGO_TARGET_DIR=/target/mdb-base cargo build --release) &&
  (cd candidate && CARGO_TARGET_DIR=/target/mdb-candidate cargo build --release) &&
  for data in noisy ramp; do for pass in 1 2; do for side in base candidate; do
    /target/mdb-$side/release/metrics-day-bench-$side $data 31; done; done; done'
```

The raw output is [windows.txt](data/metrics-day-2026-11-24/windows.txt) and
[linux.txt](data/metrics-day-2026-11-24/linux.txt).

## Cutting a minute's series by the hour

The first thing tried was the issue's second way: a series with 48 samples an
hour or more took no unit of the ladder longer than an hour, so that an
hourly bucket holds whole blocks and takes their summaries. It was measured
before this round's harness existed, on Windows, with an earlier form of the
same program holding counters alone, 600 series of a sample a minute for two
days, at TinyStore `88827ee8` and that commit with the one rule added; the
rule was never committed, so these four rows are an anecdote by this
repository's rules and are kept as the reason the ladder was left alone.

| | Blocks of four hours | Blocks of an hour |
|---|---|---|
| `metrics.db`, ramps | 1,032,192 bytes, 0.597 B a sample | 2,445,312 bytes, 1.415 B a sample |
| `metrics.db`, noisy | 4,947,968 bytes, 2.863 B a sample | 5,578,752 bytes, 3.228 B a sample |
| A day by the hour, ramps | 23.5 ms, 900,000 decoded | 20.1 ms, 72,000 decoded |
| A day as one number, ramps | 11.1 ms, 180,000 decoded | 18.6 ms, 72,000 decoded |

The bytes went to the rows that hold a block each: on ramps `payloads` from
7,200 rows and 242,232 bytes to 28,200 rows and 935,521, `groups` from 705
rows and 174,629 bytes to 1,281 and 523,222, `clocks` from 244 rows and
29,472 bytes to 506 and 113,875. Twelve times fewer samples decoded bought
3.4 ms, and four times the blocks cost the read of one number 7.5 ms: taking
a block's summary and its entry costs about what decoding an hour of a
minute's samples does. The same holds against summaries kept by a finer
step inside a block, the issue's first way, which would add about the same
bytes to the directories.

## Where the time went

The reads were run under callgrind in the container, three passes of each of
the seven, collecting inside `Engine::aggregate` alone. The first profile was
taken once a sample's bucket was no longer found again for each sample, the
first change below, and before the others
([profile-before-inclusive.txt](data/metrics-day-2026-11-24/profile-before-inclusive.txt),
[profile-before-self.txt](data/metrics-day-2026-11-24/profile-before-self.txt)):
8,428 million instructions.

| Part | Instructions | What it was |
|---|---|---|
| `Fold::sample`, counting | 37.2 % | 215 instructions a sample: each checked against the range, its bucket found again, two exact additions a step of a counter |
| `decode_block` | 28.2 % | `read_rice` 10.8 % of the whole, a rice code's unary part read a bit at a time; a block's times 5.1 %, with a division a step of an even clock |
| `Engine::take`, the file | 20.1 % | a group's directory 5.5 %, clocks 4.5 %, SQLite's steps 5.2 % |
| `Counting::join`, buckets | 9.4 % | a bucket of a series joined into its group's |

A profile counts instructions and suggests; the timings below are what
confirms each change.

## What took it away

Each change keeps every answer: TinyStore's 296 tests of metrics pass at each,
among them the aggregates held to a read of the samples, and two new tests
hold the bit reader to a read a bit at a time on whole and cut data.

- **The open bucket kept** (`d57dfeae`). A sample before the open bucket's
  end is in it, since times only grow, so its bucket is not found again. For
  buckets of a width this was a division a sample; for a calendar's it was
  the zone's arithmetic, more than counting the sample.
- **A run of samples counted in a loop** (`d57dfeae`). The sample that opens a
  bucket goes the whole way; those after it up to the bucket's end are
  checked for their order and their value alone.
- **A counter's rising steps counted at once** (`d57dfeae`). Steps without a
  reset between them add up to the last sample less the first, exactly, so
  the exact sum takes two additions a run and not two a sample.
- **A rice code read with one shift** (`b2734b6c`). Its unary part is counted
  in the word that waits, and a code that lies in one word is taken whole.
- **An even clock walked without a division a step** (`b2734b6c`).
- **64 decoded clocks kept a snapshot, not 8** (`9d6eb149`). The series here
  are written at sixty seconds of the minute, so sixty clocks came round and
  each series decoded its own again.

After the first four and with 64 clocks
([profile-after-fold-inclusive.txt](data/metrics-day-2026-11-24/profile-after-fold-inclusive.txt)),
the same reads took 6,444 million instructions: counting 23.4 %, decoding
35.7 %, the file 24.0 %, joining 12.8 %.

## How fast now

The least time of 31 runs, milliseconds, `first pass; second pass`.

### Windows

| Read | noisy, `8cca207e` | noisy, `9d6eb149` | ramp, `8cca207e` | ramp, `9d6eb149` |
|---|---|---|---|---|
| increase, one number | 14.37; 14.76 | 11.13; 10.97 | 10.12; 10.27 | 7.94; 8.01 |
| increase by the hour | 33.04; 34.76 | 20.97; 20.22 | 23.97; 23.35 | 14.50; 14.63 |
| increase by the hour, by node | 33.23; 34.27 | 21.05; 20.09 | 23.94; 23.50 | 14.55; 14.70 |
| increase by 5 minutes | 50.06; 49.19 | 40.55; 38.96 | 38.45; 39.58 | 32.17; 32.58 |
| increase by the day, Moscow | 21.97; 21.57 | 12.16; 11.43 | 17.51; 17.29 | 8.32; 8.40 |
| avg by the hour | 33.75; 33.66 | 25.52; 24.29 | 21.38; 20.68 | 14.00; 14.08 |
| max by the hour | 32.28; 32.71 | 24.20; 23.09 | 20.28; 19.78 | 13.32; 13.23 |

### The Linux container

| Read | noisy, `8cca207e` | noisy, `9d6eb149` | ramp, `8cca207e` | ramp, `9d6eb149` |
|---|---|---|---|---|
| increase, one number | 10.53; 11.05 | 7.74; 8.40 | 8.73; 9.19 | 7.07; 7.00 |
| increase by the hour | 28.96; 29.12 | 16.59; 17.17 | 21.58; 22.33 | 13.72; 13.41 |
| increase by 5 minutes | 44.13; 44.35 | 38.85; 41.10 | 36.71; 38.36 | 37.62; 35.74 |
| increase by the day, Moscow | 17.91; 17.65 | 8.38; 8.88 | 16.11; 16.32 | 8.06; 7.57 |
| avg by the hour | 27.33; 27.16 | 19.95; 20.25 | 19.00; 19.22 | 13.45; 13.45 |
| max by the hour | 26.33; 25.86 | 18.81; 19.03 | 17.93; 17.92 | 12.41; 12.04 |

A day by the hour decodes 936,000 samples and the day as one number 216,000,
and the two now stand 9.5 ms apart on Windows on noisy counters where they
stood 19 apart: decoding and counting 720,000 samples more costs 13 ns a
sample, from 27.

## What follows

- The three commits are on TinyStore's `main`. The issue's read is 0.57–0.64
  of what it was, not the cost of one number: a day by the hour still
  decodes the day.
- **The file is the next half.** The day as one number takes 11 ms on noisy
  counters with 216,000 samples decoded, about 3 ms of it by the figure
  above: the rest is 600 series taken from the file, 13 µs a series, in a
  statement for its state, one for its groups, one for each payload decoded
  and a directory of 32 blocks unpacked for the six or seven the day needs.
  Reading the series of a query in batches would be measured against that.
- **Buckets have a cost of their own.** A day by five minutes is 173,400
  buckets of series and stands 16 to 19 ms above a day by the hour on both sides:
  about 100 ns a bucket, a bucket of a series holding an exact sum of 35
  words that is moved several times before it joins its group's.
- **The ladder stays, and no summary by a finer step is built**: by the four
  rows above, a block's entry costs what its hour of samples costs to decode.
