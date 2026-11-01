# MS MARCO's queries timed, in WSL2's containers — 1 November 2026

**The harness of this round is
[search-msmarco-2026-11-01](search-msmarco-2026-11-01.md)'s, at research
commit `5c6c363`, unchanged.** That round measured answers, effectiveness and
bytes and left H2.1 — the compact blocks within two times Tantivy at p99 —
to a quiet machine and then bare Linux. This is the quiet machine: one
Windows 11 desktop, every arm in a Docker container in WSL2's virtual
machine. By the `measure` skill a latency with one call in flight in WSL2's
VM measures that VM's processors waking as well as the engine, and is quoted
from bare Linux only; every millisecond below is this machine's in WSL2, and
what the round can say is the order of the engines and their ratios in one
run, each against Tantivy in the same round.

| Question | Answer | What follows |
|---|---|---|
| H2.1: within two times Tantivy at p99? | **No, here.** Over the 1,097 sample queries the compact blocks' p99 is 3.02–3.06 times Tantivy's cold and 3.46 warm, every interval above 2 (the lowest bound 2.75); their p50 is 4.78–4.84 times | bare Linux, before H2.1 is closed |
| Where do the engines stand? | Lucene's p50 0.69–0.74 of Tantivy's, PISA's 0.82–0.94, the compact blocks' 4.8, FTS5's 1,357–1,815 on its 200 queries | a profile of the compact arm's passes |
| Does a cold cache change it? | Little for the compact blocks: p50 12.05 and 12.07 ms cold against 11.13 and 10.99 warm, p99 95.6 and 96.6 against 92.4 and 92.6 | — |
| Are the answers stable? | Every arm's ids are the same in all four of its passes, for every query | — |

## The run

`timing.sh` as the earlier report describes it: five arms, each a process of
its own a pass, one query at a time, each container 2 CPUs and 6 GiB; rounds
A B C D E and E D C B A, cold and then warm, four rounds; a cold pass first
evicts its arm's files from the VM's page cache and logs the bytes cached
before and after; a warm pass runs every query once untimed, then times each
once. Every arm but FTS5 answers the 1,097 sample queries (`all`), and every
arm their first 200 (`fts`). p50 and p99 are nearest rank, each with a
percentile interval from 10,000 resamples of the queries; each ratio is an
arm's p50 or p99 over Tantivy's of the same round, resampled in pairs.

Every cold pass's eviction left 0 bytes of the arm's files cached; the
first pass of each arm found none cached either, the other 17 found 0.57
to 1.41 GB ([passes.jsonl](data/search-msmarco-2026-11-01/timing/20261101T111106Z/passes.jsonl)).
The run's label, `quiet machine, WSL2 container`, is not the harness's
default `measurement`, so `bootstrap.py` heads its summary "not a
measurement"; this report is what says what the numbers are.

## The sample's 1,097 queries

p50 and p99 in ms, the two rounds of each mode as `a; b`, and each over
Tantivy's of its round with the interval of the first
([summary.md](data/search-msmarco-2026-11-01/timing/20261101T111106Z/summary.md)):

| arm | cold p50 | cold p99 | warm p50 | warm p99 | p50 / Tantivy, cold | p99 / Tantivy, cold | p99 / Tantivy, warm |
|---|---|---|---|---|---|---|---|
| compact blocks | 12.05; 12.07 | 95.6; 96.6 | 11.13; 10.99 | 92.4; 92.6 | 4.78 [4.56, 5.26]; 4.81 | 3.06 [2.75, 3.87]; 3.02 | 3.46 [2.77, 3.87]; 3.46 |
| Tantivy | 2.52; 2.51 | 31.2; 31.9 | 2.30; 2.29 | 26.7; 26.8 | 1 | 1 | 1 |
| PISA `block_max_maxscore` | 2.17; 2.37 | 14.7; 14.8 | 2.00; 2.01 | 15.3; 14.9 | 0.86 [0.81, 0.95]; 0.94 | 0.47 [0.42, 0.62]; 0.46 | 0.57 [0.46, 0.65]; 0.56 |
| PISA `block_max_wand` | 2.06; 2.07 | 23.5; 25.0 | 2.04; 2.06 | 23.6; 23.5 | 0.82 [0.78, 0.88]; 0.83 | 0.75 [0.66, 0.97]; 0.78 | 0.88 [0.81, 0.99]; 0.88 |
| Lucene | 1.87; 1.84 | 19.4; 18.0 | 1.59; 1.57 | 11.9; 11.9 | 0.74 [0.71, 0.81]; 0.73 | 0.62 [0.45, 1.10]; 0.56 | 0.44 [0.38, 0.53]; 0.45 |

**The compact blocks lose by a factor of three at p99 and five at p50.**
The two rounds of each mode agree within 2 % for them, and the intervals of
every round's ratio lie above 2. Cold, PISA's `block_max_maxscore` has the
shortest tail, under half of Tantivy's; warm, Lucene's is shorter still,
0.44 of Tantivy's; Lucene has the shortest median in both. The
earlier round found every engine's top ten exact on these queries, so these
are the costs of one answer computed five ways.

## The first 200, with FTS5

| arm | cold p50 | cold p99 | warm p50 | warm p99 | p50 / Tantivy, warm |
|---|---|---|---|---|---|
| compact blocks | 17.70; 18.14 | 133.4; 131.4 | 11.19; 11.32 | 69.8; 68.9 | 4.93 [4.43, 5.70]; 4.79 |
| FTS5 | 4,166; 4,218 | 8,604; 8,979 | 4,116; 4,220 | 8,340; 8,383 | 1,815 [1,461, 2,233]; 1,787 |
| Tantivy | 3.07; 3.00 | 44.4; 44.2 | 2.27; 2.36 | 21.2; 20.7 | 1 |
| PISA `block_max_maxscore` | 2.47; 2.29 | 12.8; 11.0 | 2.02; 1.98 | 11.2; 10.6 | 0.89 [0.81, 1.09]; 0.84 |
| PISA `block_max_wand` | 2.06; 2.08 | 17.8; 18.2 | 2.05; 1.98 | 17.9; 18.2 | 0.90 [0.84, 1.05]; 0.84 |
| Lucene | 3.30; 3.19 | 33.8; 63.8 | 1.70; 1.83 | 10.6; 10.6 | 0.75 [0.69, 0.91]; 0.77 |

FTS5, scoring every match with `bm25()`, takes four seconds a query, as the
earlier round's untimed runs suggested. Lucene's cold p99 differs between
its rounds (33.8 and 63.8 ms) on 200 queries; a p99 of 200 by nearest rank
is the 198th, and a JVM starts cold in each cold pass.

## Limits

- One machine, in WSL2's VM. H2.1 asks for two machines, and the `measure`
  skill for bare Linux before a latency is quoted; the factor of three may
  be smaller or larger there.
- During the run two agents and this session read files and ran `grep`,
  `awk` and `zcat` on small files; no build, test, fit or other container
  ran until it ended. Before it started no process used more than 0.2 % of
  the processors over five seconds.
- A cold pass empties the VM's page cache; whether Windows still holds the
  virtual disk's blocks in its own cache is not measured. Every arm is
  barely slower cold than warm (Tantivy 2.52 against 2.30 ms at p50), which
  that would explain, so "cold" here may mean "not in the VM's memory" only.
- The gap grew with the collection: on search-compact-2026-10-16's million
  documents the compact blocks' p99 was 1.5–1.8 times Tantivy's in one
  session, here 3.0–3.5 times. The corpora, queries and sessions differ, so
  this is a pointer for the profile, not a measured scaling.
- TinyStore itself searches with FTS5, the arm four seconds a query here;
  the compact blocks are a prototype, not what TinyStore ships.

  > **Correction, 1 November 2026.** The bullet above is wrong. It was read
  > from the submodule here, pinned at `e307c48`, a commit of the Go code
  > before the Rust rewrite reached TinyStore's `main`. TinyStore's `main`
  > searches with its own index, not FTS5: postings in blocks of 128 with a
  > directory of each block's best possible score, windows of 4,096 passed by
  > their bounds, a head of recent changes packed 2,048 rows a commit, scores
  > in whole points (its handbook, `docs/handbook/inside/how-search-finds.md`,
  > at `4390117b`). That is the design the compact blocks here measure; this
  > round did not build TinyStore's own code, and its numbers are the
  > prototype's.
- The compact arm is the lab's executor in Rust over rusqlite with
  SQLite's 1 MiB page cache as TinyStore opens a file; why it is slower is
  not measured here. A profile of its passes is the next step, and its
  answer a hypothesis until a second measurement separates it.
- H2.3, `churn.sh`, did not run in this window.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop 29.6.2's WSL2 VM,
16 CPUs and 15.2 GiB, on an AMD Ryzen 7 7700 with 31.1 GiB under Windows 11
Pro 10.0.26300; every image, version and index as in
[search-msmarco-2026-11-01](search-msmarco-2026-11-01.md#environment-and-reproduction).
The harness at research commit `5c6c363`; the passes from 11:11:10 to
12:35:51 UTC, 44 of them, compressed and summarised by 12:40.

```sh
# from <repo>, after round.sh has built the tools and every index
LABEL="quiet machine, WSL2 container" sh tinystore/bench/search-msmarco/timing.sh
```

Raw output, every pass's query ids and nanoseconds, is in
[timing/20261101T111106Z](data/search-msmarco-2026-11-01/timing/20261101T111106Z/).
