# TinyStore's own search on MS MARCO, and what its writes cost — 2 November 2026

**The harness is [bench/search-msmarco](../bench/search-msmarco/) with
`tiny/` beside it, at research commit `b48a1e9`; TinyStore at
`d2e65efd`, its `main`.** The rounds of 1 November measured prototypes.
This one builds TinyStore's own engine through its public API on the same
8,841,823 passages, compares its answers with the compact blocks', and
times its searches beside the arms it was built from
([tiny/README.md](../bench/search-msmarco/tiny/README.md) wrote the
hypotheses T1–T4 before any time was taken).

A first build, of `a5143eb1`, did not finish: each million passages took
longer than the one before. Finding why, and changing the layout so that it
does not, became most of the round; making the new layout fast, the rest.

| Question | Answer | What follows |
|---|---|---|
| Why did a build take hours? | **Every set wrote a page for each word it held.** 300,000 passages, set and packed a thousand a commit, wrote 11.8 GB for an index of tens of MB, the process at 55 % of a CPU. Packing ten or a hundred times less often, a larger SQLite cache or smaller runs did not change it | a layout whose writes are as long as their postings |
| What changed? | Postings in **segments**, as Lucene and FTS5 keep them (`a3f9f915`): a set writes the newest segment's pieces one after another, and merges join ten segments of a level into one of the next | — |
| How long does a build take now? | **291 s** for 8,841,823 passages, two sets in flight: 267 s setting them while background work merges, 24 s merging the rest into one segment. One set at a time took 375 s at `df8bfc1e`; the first build of segments took 786 s | — |
| T1: the prototype's speed? | **Yes**: p99 34.0–34.5 ms against `fast4-cache`'s 26.9–28.5, 1.20–1.27 times in each round, under 1.5; p50 5.0–5.7 against 3.6–3.7 | — |
| T2: within two times Tantivy at p99? | **Yes**: 1.25–1.28 times its p99, every interval under 1.5; p50 2.12–2.48 times | the p50, which `fast4-cache` holds at 1.54–1.62 |
| T3: at most 2.5 bytes a posting? | **Yes**: 1.75 bytes a posting of payload, 1.90 in pages | — |
| T4: the same answers as compact? | **Yes**: 1,093 of 1,097 sample queries the same ten in the same order, 1,095 the same ten, 10,967 of 10,970 documents | — |

## The build that did not finish

`a5143eb1` packed a key's postings onto its list where it lay: its last run
of 32 KiB, its directory's last chunk, its row. A set of a thousand
passages holds some 12,000 keys, and packing them dirtied a page for each,
and counting each key's documents another page on every set. The first
three millions took 292, 475 and 659 s, each longer than the one before,
and the build was stopped.

The first million, each variant a container of 2 CPUs and 3 GiB, several
at once on the same disk, so that the times compare within a run and not
with others; bytes are what `/proc/self/io` counts as written:

| variant | at 300,000 passages | set | pack | written |
|---|---|---|---|---|
| as built: pack after every set of a thousand | 54 s | 20 s | 35 s | 11.8 GB |
| pack after every hundredth set | 41 s | 37 s | 4 s | 15.7 GB |

Packing less often moved the cost to the sets: a head kept by key grows,
and each set writes its 12,000 keys at as many places in it. In a run of
five at once, a writer's cache of 64 MiB rather than 1 MiB and runs of
8 KiB rather than 32 reached 300,000 passages in 74 and 72 s against 75 for
the build as it was; packing after every tenth set reached 600,000 in 88 s.
`set_many` takes a thousand documents at most, so no larger commit was tried.

## Segments

A segment holds every key's postings of a run of documents, a piece a key,
its pieces keyed by the segment first so that they lie together in the
file. A set writes into the newest segment while it holds fewer than a
thousand documents, else into a new one; a piece of the open segment is
appended to as bytes. Background work merges ten segments of a level into
one of the next, a step of 4,096 pieces of each a commit. A search reads a
key's piece in each segment and joins them, in the order of their
documents, into one list that block-max MaxScore walks as before.

A key's count of documents is its row's and its pieces' in segments of
level 0 and 1: a set writes no key's row, and a merge to level 2 or above
adds its pieces' counts to their keys' rows, in the keys' order.
`search::maintain` merges every segment into one, as the harness does at
the end of its build and Tantivy's arm does with its own.

## Making it fast

Each change was profiled with `perf` in a container of the same shape,
the first million passages a run; where a change helped, the next profile
named the next cost. At one set at a time:

| TinyStore | the first million | what changed |
|---|---|---|
| `a3f9f915` | 66–69 s | segments |
| | 54 s | a merge packs its steps in a snapshot and takes the writer only to write them; the best ten of a block scored once a posting; a step's lengths read a row at a time |
| | 50 s | text already in NFKC not composed again; the ids of the keys sets met kept in memory, learned once a set commits, cleared by a purge |
| | 42 s | a large search.db checkpoints its WAL at up to 16,000 frames while they are under an eighth of its pages, rather than at 1,000 |
| `df8bfc1e` | 40–43 s | words between spaces taken whole when every letter is one Unicode's rules join, asked of them once a code point below U+3000; a merge's next step packed beside the commit of the one before |
| `6e58e0e8` | 37 s | a set's words looked up once; a merge's statements prepared once a commit; its keys' parts read 256 a statement |
| `d2e65efd` | — | ten segments a level rather than eight |

Two sets in flight, a set's words read while the other commits, took the
first million from 37 to 30 s at `6e58e0e8`. Pages of 16 KiB took it from
33 to 30 s but wrote 13.2 GB rather than 7.3, and were not taken. Reading a
step's keys' parts by their range rather than by their ids made the last
merge of all 8.8 million passages 278 s, since a step's rare words lie far
apart among the keys; the ids are read since.

## The build

At `d2e65efd`, one container of 2 CPUs and 6 GiB, two sets in flight
([build-tinystore.jsonl](data/search-tinystore-2026-11-02/logs/build-tinystore.jsonl)):

| | |
|---|---|
| passages | 8,841,823 |
| setting them, background merges beside | 266.9 s |
| merging the rest into one segment | 24.4 s |
| the whole build | 291.3 s |
| written, as `/proc/self/io` counts | 60.9 GB |
| the process's peak resident memory | 295,160 KiB |

FTS5 built the same passages in 3.4 minutes and Tantivy in 3.6 in the round
of 1 November; TinyStore's build takes 4.9, and it keeps every passage's
row too, its id and value, 3.36 GB of payload. Of the bytes written, the WAL writes each
page twice; most of the rest, by an estimate this round did not measure, is
new words taking random places in the keys' index by term, some 520 a set
over the first million.

## What the index takes

`dbstat` of search.db after the build, the b-trees that hold postings:

| b-tree | bytes | pages | payload | bytes a posting, pages |
|---|---|---|---|---|
| runs | 520,556,544 | 127,089 | 498,496,129 | 1.51 |
| pieces | 135,131,136 | 32,991 | 107,687,826 | 0.39 |
| keys and their index by term | 114,499,584 | 27,954 | 80,313,835 | 0.33 |

345,553,319 postings; 1.75 bytes a posting of payload in runs and pieces,
1.90 in their pages. The documents' rows, their ids and values, take
3.87 GB of the 4.77 GB file.

## Answers

TinyStore's ten against compact's on the 1,097 sample queries
([agree.md](data/search-tinystore-2026-11-02/answers/agree.md)): 1,093 the
same in the same order, 1,095 the same ten, 10,967 of 10,970 documents in
common. TinyStore keeps a score in whole points of 2⁻³², so a tie may order
otherwise.

## Timing

`round.sh timing`: six arms, each a process of its own a pass, warm, one
query at a time, each container 2 CPUs and 6 GiB, the arms in one order and
then the other, four rounds, from 01:48 to 01:55 UTC. p50 and p99 in ms over
the 1,097 sample queries, the four rounds' range, and each over Tantivy's of
its round ([summary.md](data/search-tinystore-2026-11-02/timing/20261102T014812Z/summary.md)):

| arm | p50 | p99 | p50 / Tantivy | p99 / Tantivy |
|---|---|---|---|---|
| TinyStore `d2e65efd` | 5.00–5.75 | 34.0–34.5 | 2.12–2.48 | 1.25–1.28 |
| `fast4-cache` | 3.62–3.72 | 26.9–28.5 | 1.54–1.62 | 1.00–1.04 |
| compact blocks | 11.47–11.78 | 92.6–95.6 | 4.90–5.01 | 3.39–3.54 |
| Tantivy | 2.30–2.36 | 26.8–27.3 | 1 | 1 |
| PISA `block_max_maxscore` | 2.26–2.29 | 15.0–16.0 | 0.96–0.99 | 0.56–0.58 |
| Lucene | 1.66–1.76 | 12.3–12.6 | 0.72–0.77 | 0.45–0.47 |

Every arm's ids were the same in all four of its passes. TinyStore's
search reads each hit's value too, which no other arm does, and its p99
interval is 1.08–1.48 times Tantivy's at its widest. The run's label, `quiet
machine, WSL2 container`, is not the harness's default, so `bootstrap.py`
heads its summary "not a measurement": as in the round of 1 November, a
latency here is this machine's in WSL2's VM, and the ratios in one run are
what the round can say.

## Limits

- One build of each kind, one machine, in WSL2's VM: a time is this
  machine's, and a build's split between setting and merging is one run's.
- The 60.9 GB are a process's writes, not a division of them by b-tree.
- The variants of the stopped build ran several at once on one disk; the
  first million's figures in *Making it fast* are single runs, a few
  seconds apart from run to run.
- Two sets in flight is how this harness sets; a program that sets from one
  thread took 375 s at `df8bfc1e`, and was not run again at `d2e65efd`.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop 29.6.2's WSL2 VM,
16 CPUs and 15.2 GiB, on an AMD Ryzen 7 7700 under Windows 11 Pro
10.0.26300; `rust:1.99`, each container 2 CPUs and 6 GiB; TinyStore's
bundled SQLite without its `LIBSQLITE3_FLAGS`, as every SQLite arm here.

```sh
# from <repo>, after round.sh of search-msmarco built every other index
sh tinystore/bench/search-msmarco/tiny/round.sh tools
sh tinystore/bench/search-msmarco/tiny/round.sh build
sh tinystore/bench/search-msmarco/tiny/round.sh answers
LABEL="quiet machine, WSL2 container" sh tinystore/bench/search-msmarco/tiny/round.sh timing
```

Raw output, every pass's query ids and nanoseconds, is in
[timing/20261102T014812Z](data/search-tinystore-2026-11-02/timing/20261102T014812Z/);
[answers.jsonl](data/search-tinystore-2026-11-02/logs/answers.jsonl) holds
two runs for answers, of the `a3f9f915` build and of this one.
