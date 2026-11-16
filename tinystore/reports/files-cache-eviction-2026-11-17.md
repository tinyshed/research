# Which files go first under a set's ceiling — 17 November 2026

**The rank ships, with a head start of one day.** On the crawler traces it
beats LRU on the same coarse clock by 0.7 to 4.2 points of hit rate over the
60 days, and by 6.2 to 8.3 points in the 24 hours after each crawl, where
LRU loses 3.8 to 10.6 points against the same trace without a crawler and the
rank 0.6 to 3.7. On the other traces it never falls below coarse LRU: from
+0.0 to +6.3 points, the same sign on every seed. Seven days would win more
on a stationary Zipf mix and loses 3.4 points to LRU when the hot set moves
weekly; six hours is safe and buys a third of the day's gain. The hourly
clock costs LRU at most 0.3 points against exact LRU.

| Question                                             | Answer                                                                             | What follows                                         |
|------------------------------------------------------|------------------------------------------------------------------------------------|------------------------------------------------------|
| Does the 1-day rank beat coarse LRU after a crawler? | yes: +0.7 to +4.2 points over 60 days, +6.2 to +8.3 in the day after a pass        | the book's order ships                               |
| Does it stay within a point of LRU elsewhere?        | it is never below it: +0.0 to +6.3, every seed                                     | no trace here argues for plain LRU                   |
| Is a day the right head start?                       | 7 d loses 3.4 points on a weekly shift at 10 %; 6 h gains a third of what 1 d does | a day, as the book says                              |
| What does the coarse clock cost?                     | LRU on an hourly clock against exact LRU: 0.0 to 0.3 points                        | the hourly write stays                               |
| How far is the rank from 2Q?                         | 2Q leads by 0.3 to 2.5 points on Zipf, loses at 30 % shift                         | not worth a second structure; the rank is one column |

The rank is the book's (`tinystore/.scratch/files-cache/book.md`, "Which
files go first", not on TinyStore's `main`): a file's rank is its last
recorded use, plus the head start once a use was recorded after the write
that made it; eviction takes the lowest rank first. A use is recorded at most
once an hour, so a read within the hour after the write gives no head start.
`lru-coarse` is the same code with a head start of zero.

## The simulator and the traces

`tinystore/files-cache-sim/sim.ts` replays a trace of reads, each a time, a
key and its size. A hit is a key in the cache; a miss writes the file at its
size, then evicts until the set is under the ceiling (the file just written
may be the one that goes). Reads of the first two days fill the cache and are
not counted. Hit rate is hits over reads; byte hit rate is bytes read from
the cache over bytes read. A crawler's own reads count in neither.

Every trace has 100,000 keys, sizes log-normal with a median of 200 KiB and
σ 1 (clipped to 4 KiB–64 MiB; mean about 330 KiB, 33.6–33.8 GB in all), and
lasts 60 days. The ceiling is a fraction of all keys' bytes.

| Trace     | Reads                                                                                                                                           | Ceilings             |
|-----------|-------------------------------------------------------------------------------------------------------------------------------------------------|----------------------|
| `zipf`    | 30,000 a day at uniform times, Zipf α 0.8 or 1.0 over the keys in a random order                                                                | 10 %, 30 %           |
| `crawler` | `zipf`, plus every key once in random order from 02:00 to 05:00 of days 20 and 40                                                               | 10 %, 30 %           |
| `shift`   | 30,000 a day: 70 % Zipf 0.8 over a hot set of 5,000 keys drawn again every 7 days, 30 % Zipf 0.8 over every key                                 | 10 %, 30 %           |
| `lecture` | 300 course keys read Poisson(4) times each between 08:00 and 20:00 of every weekday; each other key read once, at a uniform time in the 60 days | 1 %, 2 %, 10 %, 30 % |

The lecture trace's ceilings go down to 1 % because at 10 % every policy
already keeps the 300 course keys through a weekend: 34.3 % is every read
that can hit, the long tail missing by construction.

Policies: `lru-exact`, a list moved on every read; `2q-exact`, the full 2Q of
Johnson and Shasha in bytes, A1in a FIFO of a quarter of the ceiling, A1out
the ghosts of half of it, Am an LRU; `lru-coarse`; and the rank with head
starts of 6 hours, 1 day and 7 days, `rank-6h`, `rank-1d`, `rank-7d`, all
three on the hourly clock. Each seed draws its own sizes, popularity order
and trace; five seeds a trace.

## Hit rate

Mean of five seeds, % of reads; the last column is `rank-1d` minus
`lru-coarse` on each seed, 1 to 5.

| trace, ceiling      | lru-exact | 2q-exact | lru-coarse | rank-6h | rank-1d | rank-7d | rank-1d − lru-coarse         |
|---------------------|----------:|---------:|-----------:|--------:|--------:|--------:|------------------------------|
| zipf α 0.8, 10 %    |      46.8 |     53.3 |       46.8 |    48.2 |    50.8 |    55.5 | +4.0; +4.0; +4.0; +4.0; +4.0 |
| zipf α 0.8, 30 %    |      67.8 |     69.7 |       67.8 |    68.1 |    68.9 |    71.9 | +1.1; +1.1; +1.1; +1.1; +1.1 |
| zipf α 1.0, 10 %    |      73.8 |     77.3 |       73.8 |    74.4 |    75.6 |    78.5 | +1.7; +1.8; +1.7; +1.7; +1.8 |
| zipf α 1.0, 30 %    |      85.7 |     86.3 |       85.7 |    85.8 |    86.0 |    87.0 | +0.3; +0.3; +0.3; +0.3; +0.3 |
| crawler α 0.8, 10 % |      46.6 |     53.2 |       46.5 |    48.0 |    50.8 |    55.4 | +4.2; +4.2; +4.3; +4.2; +4.2 |
| crawler α 0.8, 30 % |      67.2 |     69.5 |       67.2 |    67.7 |    68.7 |    71.9 | +1.5; +1.4; +1.4; +1.5; +1.4 |
| crawler α 1.0, 10 % |      73.5 |     77.3 |       73.4 |    74.2 |    75.5 |    78.5 | +2.0; +2.1; +2.1; +2.0; +2.0 |
| crawler α 1.0, 30 % |      85.0 |     86.1 |       85.0 |    85.4 |    85.7 |    87.1 | +0.7; +0.7; +0.7; +0.7; +0.7 |
| shift, 10 %         |      77.4 |     78.7 |       77.4 |    78.1 |    79.1 |    74.0 | +1.8; +1.8; +1.7; +1.7; +1.7 |
| shift, 30 %         |      88.0 |     86.8 |       88.0 |    88.0 |    88.1 |    88.5 | +0.1; +0.1; +0.1; +0.1; +0.1 |
| lecture, 1 %        |      26.2 |     33.4 |       26.2 |    28.9 |    32.4 |    34.3 | +6.2; +6.3; +6.3; +6.2; +6.1 |
| lecture, 2 %        |      32.2 |     34.3 |       32.2 |    32.5 |    32.7 |    34.3 | +0.4; +0.5; +0.5; +0.5; +0.4 |
| lecture, 10 %       |      34.3 |     34.1 |       34.3 |    34.3 |    34.3 |    34.3 | +0.0; +0.0; +0.0; +0.0; +0.0 |
| lecture, 30 %       |      34.3 |     34.1 |       34.3 |    34.3 |    34.3 |    34.3 | +0.0; +0.0; +0.0; +0.0; +0.0 |

Byte hit rates follow the hit rates within a point everywhere, with the
same sign of the difference on every seed; they are in `data/files-cache-eviction-2026-11-17/tables.md`. The
largest gaps between the two: lecture at 1 %, 27.0 for coarse LRU and 33.4
for the rank, +5.9 to +6.9 a seed.

## The day after a crawl

User reads in the 24 hours after each pass ended, both passes together, %:

| trace, ceiling      | lru-exact | 2q-exact | lru-coarse | rank-6h | rank-1d | rank-7d | rank-1d − lru-coarse         |
|---------------------|----------:|---------:|-----------:|--------:|--------:|--------:|------------------------------|
| crawler α 0.8, 10 % |      43.1 |     52.7 |       43.0 |    46.0 |    50.2 |    55.9 | +7.0; +7.2; +7.2; +7.4; +7.1 |
| crawler α 0.8, 30 % |      57.5 |     68.8 |       57.2 |    62.0 |    65.4 |    72.0 | +8.1; +8.3; +8.3; +8.4; +8.2 |
| crawler α 1.0, 10 % |      67.9 |     77.3 |       67.8 |    70.9 |    74.2 |    78.8 | +6.5; +6.3; +6.4; +6.2; +6.3 |
| crawler α 1.0, 30 % |      75.6 |     86.1 |       75.3 |    79.8 |    82.3 |    86.6 | +7.2; +6.8; +6.8; +6.9; +7.1 |

Against the same reads without a crawler (the `zipf` row's 60-day rate,
which a day's window of a stationary mix matches), a pass costs coarse LRU
3.8, 10.6, 6.0 and 10.4 points the day after, and the 1-day rank 0.6, 3.5,
1.4 and 3.7. LRU's loss is larger at the larger ceiling, since a pass of
every key flushes all of it; the rank keeps the files used again within the
day before the pass. Spread over 60 days and two passes the gain shrinks to
the 0.7–4.2 points above, much of which the rank wins on the plain Zipf
trace too.

## Findings

**The rank wins on a stationary mix without any crawler.** On `zipf` the
1-day rank beats coarse LRU by 0.3 to 4.0 points, most where the ceiling is
tight and α low: a head start rewards a file read twice, which on a fixed
popularity is a frequency signal LRU ignores. Its gain on `crawler` is that
gain plus the crawler's, and the day-after table separates the two.

**Seven days pays on fixed popularity and costs when it moves.** `rank-7d`
leads every Zipf, crawler and lecture row and is the only head start below
coarse LRU anywhere: on `shift` at 10 %, 74.0 against 77.4, since last
week's hot set keeps a week's lead over this week's. A day never loses;
six hours never loses and gains about a third as much. The book's argument
for a day (protected files age out) is borne out on the one trace that tests
it, and a week would put that trace's loss into a product.

**The coarse clock is free.** `lru-coarse` is within 0.3 points of
`lru-exact` on every row, the largest gap in the day after a crawl.

**2Q is the better policy and not by much.** It leads the 1-day rank by 0.3
to 2.5 points on Zipf and crawler over 60 days and by 2.5 to 3.8 in the day
after a crawl, by 1.0 and 1.6 on lecture at 1 % and 2 %, and is below coarse LRU on `shift` at 30 % (86.8 against 88.0) and on
lecture at 10 % and 30 %, where, as a hypothesis not measured apart, its A1in
quarter holds files that are never read again. It needs two lists and a ghost list where the rank needs one
column the coarse write already writes, and nothing here makes up for that.

**What the traces do not test.** A crawler that passes twice within a day
gives every file the head start, as the book says, and is not simulated;
nor are sizes that correlate with popularity, nor a real trace. These
numbers are a model's: a hit rate of derived files on a real host may order
the policies differently.

## Environment and reproduction

Harness `tinystore/files-cache-sim` at research `247810d`. The rank is the
design in TinyStore's `.scratch/files-cache/book.md`, not code; no TinyStore
commit is measured and the submodule did not move. Machine: AMD Ryzen 7
7700, 31 GiB, Windows 11 Pro, Bun 1.4.2. The simulator reads no clock and
no disk and its PRNG (sfc32, seeded by splitmix32) is deterministic, so the
numbers are the same on any machine; two runs, the second
2026-11-16T23:05:10Z to 23:05:35Z UTC, gave byte-identical output. 70 lines,
6 traces × 5 seeds, 2 or 4 ceilings each, 1.8–2.0 million reads a trace.

```sh
cd tinystore/files-cache-sim
sh run.sh > results.jsonl     # 25 s on 8 processes
bun table.ts results.jsonl    # the tables above, unrounded inputs in results.jsonl
```

Raw output: `data/files-cache-eviction-2026-11-17/results.jsonl` (hits,
reads and bytes for each policy, ceiling and seed) and `tables.md`.

## What follows

The book's section stands as written: ship the rank with a day's head start
and the hourly coarse write. Its line "if the day's head start does not beat
LRU's hit rate on the crawler trace and match it on the others, the engine
ships plain LRU" can cite this report as met.
