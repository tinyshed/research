# A prefix's postings in memory, against the store's budget — 10 November 2026

**The harness is [search-prefix-bench](../search-prefix-bench/), at research
commit `4ac4410`; TinyStore at `02231bd0`, its `main`.** A read-only audit
of TinyStore's search, its finding B-4-04, read in `search/read.rs` that a
search with `prefix` reads the postings of up to 64 words its prefix begins
whole into memory, 12 bytes a posting, and reserves none of them from the
store's memory; with `prefix_every`, up to eight such prefixes. On a large
index a short prefix would then hold far more than a budget, and answer
rather than fail `limit`, against STORE-05 ("bound each call alone") and
ERR-05. Nobody had measured it. This round does, on indexes generated from
a seed, each search beside the same text searched without a prefix.

| Question                                  | Answer                                                                                                                                                                                                                                                                        | What follows                                                                            |
|-------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------|
| Is the finding true?                      | **Yes.** Every search with a prefix of two or four letters held heap in proportion to the postings its prefix's words hold, reserved none of the store's memory, and answered: none of the run's 2,704 searches failed, with a budget or without                              | STORE-05 and ERR-05 do not hold for `prefix` and `prefix_every`; the fix is TinyStore's |
| How much above the same search without?   | A prefix of two letters on a million documents of fifty words: **46.3 MiB**, against 339 KiB for the search without it; at four million documents 185.2 MiB. `prefix_every` with eight such prefixes: **157.1 MiB** and **628.2 MiB**, against 3 KiB                          | —                                                                                       |
| How does it grow?                         | As a line through zero: 32.1–32.2 bytes a posting the prefix's words hold for one prefix, 16.3 for eight, at every size from 250,000 to 4,000,000 documents. With the words a document has, as those postings grow: 24.0, 33.2, 46.3 and 66.3 MiB at 12, 25, 50 and 100 words | —                                                                                       |
| Against a budget?                         | With `memory` at 16 MiB, `prefix` held 0.5–11.7 times the budget and `prefix_every` 2.0–39.6 times; the store's memory reserved during a search: 0 bytes, every one; every search answered                                                                                    | —                                                                                       |
| One letter, two, four?                    | One letter is read whole, as `LEAST_PREFIX` says, and holds nothing more. Two and four hold what their words' postings ask: the worst prefix of four letters here begins the commonest word, 993,479 postings and 34.0 MiB at a million                                       | —                                                                                       |
| Does it hang on how a realloc is counted? | Little: counted by what it adds rather than as a new block beside the old, `prefix` of two letters held 4.6 % less, 44.5 MiB at a million documents, and every search with eight prefixes the same                                                                            | —                                                                                       |
| Windows?                                  | The same heaps natively, each search's to within 64 bytes, on the million documents of fifty words                                                                                                                                                                            | —                                                                                       |
| Time?                                     | `prefix` of two letters: 82.6 ms against 0.97 without at a million documents, 443 ms at four million; `prefix_every` 728 ms and 3.5 s, against 0.10 and 0.09 without                                                                                                          | —                                                                                       |
| Which fix do the numbers favour?          | A hypothesis: merging the words' lists a block at a time, since reserving the postings before reading would refuse a prefix of two letters from about 350,000 documents under this budget                                                                                     | a candidate, measured against `02231bd0`                                                |

## The fixture

`search-prefix-bench build` generates an index from a seed and nothing
else; nothing is fetched or kept. Its vocabulary is 50,000 words of five to
nine letters of an alphabet of eight, `a` to `h`, each drawn at random; a
word's rank is the order it was drawn in, and a document's words are drawn
by Zipf's law of exponent 1 over the ranks. Two letters begin some 780 words
and four some 12, so a short prefix begins common words among rare ones, as
`th` begins `the`. Each document is a fixed number of words; at fifty, the
commonest word, `bdcchah`, is in 99 % of documents. The documents are one
stream of the seed, so a smaller index of the same length is the first
documents of a larger one.

The build sets a thousand documents a commit, two sets in flight while
background work merges, as search-msmarco's harness sets, then merges the
rest into one segment with `search::maintain`. Of each index's 50,000 keys'
rows, none counts a word's documents otherwise than the fixture does, so the
engine chooses a prefix's words as the harness reckons it does:

| documents | words a document | postings    | search.db | built in, s |
|-----------|------------------|-------------|-----------|-------------|
| 250,000   | 50               | 10,595,672  | 169 MiB   | 12          |
| 500,000   | 50               | 21,188,989  | 329 MiB   | 21          |
| 1,000,000 | 50               | 42,376,849  | 647 MiB   | 48          |
| 2,000,000 | 50               | 84,753,788  | 1,268 MiB | 83          |
| 4,000,000 | 50               | 169,508,751 | 2,506 MiB | 212         |
| 1,000,000 | 12               | 11,315,412  | 215 MiB   | 24          |
| 1,000,000 | 25               | 22,475,825  | 368 MiB   | 27          |
| 1,000,000 | 100              | 79,126,628  | 1,291 MiB | 59          |

## The searches

`search-prefix-bench measure` opens a store on an index, `background:
false`, its `memory` none or 16 MiB, and runs each search with the default
limit of ten:

- **`prefix`**: the commonest word and a prefix, `bdcchah ea`, read with
  `prefix()`; beside it the same text without, where `ea` is a whole word
  no document holds. The prefix of each length is the one whose words, as
  `FIND_PREFIXED` chooses them, the 64 most held, hold the most postings.
- **`prefix_every`**: the eight such prefixes of a length, read with
  `prefix_every()`, beside the same text whole.
- **eight common words**, whole, for what a search of words in most
  documents holds.

A search's **postings read** are those its prefixes' chosen words hold, the
sum of their documents. How much they vary among the prefixes of an index,
at a million documents of fifty words (`summary.md` has every index):

| letters | prefixes | least   | median  | p90       | most      |
|---------|----------|---------|---------|-----------|-----------|
| 2       | 64       | 193,595 | 381,255 | 1,006,900 | 1,509,713 |
| 4       | 4,096    | 204     | 4,536   | 15,368    | 993,479   |

Each search runs once first, so that what searches keep between them, the
rows of lengths and the pieces' directories, is kept; then each three times
with the counting allocator read, the searches in one order and then the
other, then nine times timed alone. A search's **heap** is the most the
process's heap held during it above what it held before, counted by the
prototype's own `GlobalAlloc` as `tests/metrics_memory.rs` counts an
ingest's; beside it a thread reads the store's `memory().used()`, as that
test does. SQLite allocates through its own malloc, which the allocator does
not see, so its own most is read from `sqlite3_memory_highwater`. Every
search's heap was the same to the byte in all three passes.

## What one search holds

At a million documents of fifty words, no budget; time in ms, the median of
nine passes and their least and most:

| search         | letters | prefix          | words read | postings read | heap without | heap with | above     | bytes a posting | ms without       | ms with               |
|----------------|---------|-----------------|------------|---------------|--------------|-----------|-----------|-----------------|------------------|-----------------------|
| `prefix`       | 1       | `e`             | 0          | none          | 339 KiB      | 339 KiB   | 0 B       | —               | 0.66 (0.52–3.10) | 0.81 (0.62–1.83)      |
| `prefix`       | 2       | `ea`            | 64         | 1,509,713     | 339 KiB      | 46.6 MiB  | 46.3 MiB  | 32.1            | 0.97 (0.62–1.26) | 82.6 (63.5–173.5)     |
| `prefix`       | 4       | `bdcc`          | 12         | 993,479       | 339 KiB      | 34.3 MiB  | 34.0 MiB  | 35.9            | 1.20 (0.94–1.42) | 50.9 (39.0–72.1)      |
| `prefix_every` | 1       | 8, `e` first    | 0          | none          | 3 KiB        | 3 KiB     | 0 B       | —               | 0.04 (0.04–0.17) | 0.05 (0.02–0.07)      |
| `prefix_every` | 2       | 8, `ea` first   | 512        | 10,094,660    | 3 KiB        | 157.1 MiB | 157.1 MiB | 16.3            | 0.10 (0.07–0.19) | 727.6 (576.3–1,427.0) |
| `prefix_every` | 4       | 8, `bdcc` first | 95         | 5,458,460     | 3 KiB        | 126.7 MiB | 126.7 MiB | 24.3            | 0.09 (0.07–0.13) | 380.1 (277.7–448.7)   |

A letter alone is a whole word, as `search/read.rs` and the guide say: its 64
words would hold 4,182,423 postings, and it reads none. SQLite's own most
during a search with a prefix was 96–97 KiB for one, 215–219 KiB for eight of
four letters and 1.0–1.9 MiB for eight of two, within a reader's page cache
of 1 MiB and what it sorts, and does not grow past a million documents.

Which vectors the 28–36 bytes are is read from the code, not measured
apart, and is a hypothesis: `prefixed` reads each word's list whole into a
vector of its own and extends one vector with them, which doubles as it
grows, then sorts it and joins it into a second vector of up to the same
length while the first lives, and `List::joined` copies that into blocks
while it lives; each is 12 bytes a posting. The worst prefix of four letters
reads one list of 990,132 postings and eleven small ones, so its vector
doubles once past them, 24 bytes a posting, beside the joined one's 12: 36,
as measured. That it is vectors held together rather than a growth's moment,
a new block beside the old, the count in place below separates. With
`prefix_every` the eight joined lists are held together, and
`holding_every_word_first` and `held_by_all` clone them.

## With the documents

Fifty words a document, no budget, the worst prefix of two letters:

| documents | `prefix`: postings | above     | bytes a posting | ms                  | `prefix_every`: postings | above     | bytes a posting | ms                        |
|-----------|--------------------|-----------|-----------------|---------------------|--------------------------|-----------|-----------------|---------------------------|
| 250,000   | 377,405            | 11.6 MiB  | 32.1            | 23.3 (15.4–26.4)    | 2,524,109                | 39.3 MiB  | 16.3            | 161.6 (143.5–176.8)       |
| 500,000   | 754,588            | 23.1 MiB  | 32.1            | 42.7 (38.3–47.4)    | 5,046,713                | 78.6 MiB  | 16.3            | 325.4 (284.9–351.7)       |
| 1,000,000 | 1,509,713          | 46.3 MiB  | 32.1            | 82.6 (63.5–173.5)   | 10,094,660               | 157.1 MiB | 16.3            | 727.6 (576.3–1,427.0)     |
| 2,000,000 | 3,019,091          | 92.6 MiB  | 32.2            | 219.9 (162.5–291.2) | 20,193,905               | 314.2 MiB | 16.3            | 1,529.1 (1,274.4–2,499.5) |
| 4,000,000 | 6,033,655          | 185.2 MiB | 32.2            | 443.4 (317.5–804.7) | 40,388,621               | 628.2 MiB | 16.3            | 3,476.2 (2,900.4–4,927.9) |

The postings read are 1.51 a document for one prefix and 10.1 for eight at
every size, and the heap above is the same bytes a posting at every size: a
line through zero, 46.3 MiB a million documents for one prefix and 157.1 for
eight. Nothing in the search bounds it but the index.

## With the words a document has

A million documents, no budget, the worst prefix of two letters:

| words a document | `prefix`: postings | above    | bytes a posting | ms                  | `prefix_every`: postings | above     | bytes a posting | ms                    |
|------------------|--------------------|----------|-----------------|---------------------|--------------------------|-----------|-----------------|-----------------------|
| 12               | 766,005            | 24.0 MiB | 32.9            | 27.0 (26.2–27.9)    | 3,424,266                | 72.8 MiB  | 22.3            | 140.9 (135.0–155.6)   |
| 25               | 1,101,170          | 33.2 MiB | 31.6            | 66.3 (58.5–73.4)    | 6,144,251                | 116.4 MiB | 19.9            | 391.2 (376.6–503.3)   |
| 50               | 1,509,713          | 46.3 MiB | 32.1            | 82.6 (63.5–173.5)   | 10,094,660               | 157.1 MiB | 16.3            | 727.6 (576.3–1,427.0) |
| 100              | 2,478,273          | 66.3 MiB | 28.1            | 150.7 (117.3–190.5) | 15,878,103               | 183.0 MiB | 12.1            | 994.6 (890.1–1,651.3) |

More words a document make more postings for the prefix's words, less than
in proportion, since a common word's documents cannot pass the index's. The
heap follows the postings at 28–33 bytes each for one prefix. For eight it
falls from 22.3 to 12.1 bytes a posting as documents grow longer; a
hypothesis is that the lists held are joined, a document once however many
of a prefix's words it holds, and a longer document holds more of them.

## Against a budget

The same searches with `memory: Some(16 MiB)`: the most heap of the three
passes, and its share of the budget:

| index       | `prefix`, 2 letters | `prefix`, 4     | `prefix_every`, 2 | `prefix_every`, 4 | reserved | answered |
|-------------|---------------------|-----------------|-------------------|-------------------|----------|----------|
| 250000x50   | 11.7 MiB, 0.7×      | 8.6 MiB, 0.5×   | 39.3 MiB, 2.5×    | 31.6 MiB, 2.0×    | 0        | ok       |
| 500000x50   | 23.3 MiB, 1.5×      | 17.2 MiB, 1.1×  | 78.6 MiB, 4.9×    | 63.3 MiB, 4.0×    | 0        | ok       |
| 1000000x50  | 46.6 MiB, 2.9×      | 34.3 MiB, 2.1×  | 157.1 MiB, 9.8×   | 126.7 MiB, 7.9×   | 0        | ok       |
| 2000000x50  | 93.2 MiB, 5.8×      | 68.6 MiB, 4.3×  | 314.2 MiB, 19.6×  | 253.3 MiB, 15.8×  | 0        | ok       |
| 4000000x50  | 187.6 MiB, 11.7×    | 138.2 MiB, 8.6× | 633.1 MiB, 39.6×  | 509.4 MiB, 31.8×  | 0        | ok       |
| 1000000x12  | 24.3 MiB, 1.5×      | 23.2 MiB, 1.4×  | 72.8 MiB, 4.5×    | 53.3 MiB, 3.3×    | 0        | ok       |
| 1000000x25  | 33.5 MiB, 2.1×      | 31.2 MiB, 1.9×  | 116.4 MiB, 7.3×   | 88.2 MiB, 5.5×    | 0        | ok       |
| 1000000x100 | 66.6 MiB, 4.2×      | 35.9 MiB, 2.2×  | 183.1 MiB, 11.4×  | 161.9 MiB, 10.1×  | 0        | ok       |

`reserved` is the most of the store's memory the thread beside saw taken
during a search, over every search of the run: none. The budget held only
what searches keep between them, the rows of lengths and the directories,
which `kept.rs` keeps to half the budget: 8.0 MiB at rest on four million
documents, where the store without a budget kept 15.4 MiB. A search with a
budget held as much as one without up to two million documents, and at four
million 0.9–4.9 MiB more, the rows of lengths it read again rather than kept.
With no budget the store has no per-call bound for a search's postings
either, as `limits.rs` has `ROWS_BYTES` for SQL's rows.

## A whole word, for comparison

A search of whole words reads its lists a block at a time, and holds a block
of each and their directories' entries:

| documents | heap, `bdcchah ea` whole | heap, 8 common words | ms, `bdcchah ea`  | ms, 8 common words  |
|-----------|--------------------------|----------------------|-------------------|---------------------|
| 250,000   | 112 KiB                  | 457 KiB              | 1.28 (1.08–2.08)  | 10.2 (7.6–11.4)     |
| 500,000   | 188 KiB                  | 864 KiB              | 1.24 (1.04–2.99)  | 15.0 (13.6–15.8)    |
| 1,000,000 | 339 KiB                  | 1.6 MiB              | 0.97 (0.62–1.26)  | 41.0 (34.1–60.2)    |
| 2,000,000 | 641 KiB                  | 3.2 MiB              | 2.23 (1.44–3.08)  | 70.2 (55.5–122.5)   |
| 4,000,000 | 1.2 MiB                  | 6.4 MiB              | 4.96 (2.54–11.86) | 122.6 (112.6–163.9) |

Its heap grows with the index too, 0.32 bytes a posting its words hold: the
eight commonest words hold 5,326,516 postings at a million documents and
21,305,185 at four, and their search held 1.6 and 6.4 MiB. That is a
hundredth of a prefix's, a list's blocks' bounds rather than its postings,
and none of it is reserved either.

## A realloc counted two ways

The allocator counts a realloc as the trait's own does, and as
`metrics_memory` counts it: a new block, the bytes copied, the old block
freed, both held at that moment. The system's allocator may instead grow a
block where it lies. `REALLOC=in-place` counts the system's realloc by what
it adds, so that a growth's moment is bounded from above by the first count
and from below by the second; every index and budget ran again so, from
10:28 to 10:33 UTC ([summary-in-place.md](data/search-prefix-memory-2026-11-10/summary-in-place.md)).

Every search answered and held the same heap in its three passes. Counted in
place, `prefix` of two letters held 4.6 % less at fifty words a document,
44.5 MiB rather than 46.6 at a million documents and 177.7 rather than 186.4
at four million, 30.6–30.7 bytes a posting rather than 32.1–32.2, and
`prefix` of four letters 3.1 % less at a hundred words; every other search
held the same to within 0.1 %, `prefix_every` among them. The heap is not a
vector's growth beside its old block but what is held at once.

## Windows

The prototype built natively on Windows, `rustc 1.99.1` for
`x86_64-pc-windows-msvc` with the same SQLite flags, built the index of a
million documents of fifty words from the same seed and measured it, both
budgets, from 10:25 to 10:27 UTC
([windows/](data/search-prefix-memory-2026-11-10/windows/)). Every search's
heap was the container's to within 64 bytes, 48,877,790 against 48,877,822
for `prefix` of two letters and 164,747,822 against 164,747,886 for
`prefix_every`; its times were within the container's spread, `prefix` 85.5
ms (77.6–90.6) and `prefix_every` 759 ms (726–943).

## What the numbers do not say

- **How much a real index's prefix reads.** The fixture is no language: eight
  letters, ranks drawn at random, one field, a vocabulary that does not grow
  with the documents. What carries over is the bytes a posting, 28–36 for one
  prefix and 12–24 for eight; the postings a real prefix's 64 words hold are
  the index's. At most they are 64 words each in every document, 64 postings
  a document a prefix: at 32 bytes, 1.9 GiB a million documents, by
  arithmetic, not measured.
- **A program's resident memory.** The allocator counts the sizes Rust asks
  for, not what the system's allocator adds or gives back to the system;
  SQLite's own is counted apart. Peak RSS was not read.
- **More fields.** A prefix is read in each field of the index, 64 words in
  each; every index here has one.
- **A live index.** Each was merged into one segment; a live index reads a
  key's piece in each segment and joins them, the same postings.
- **The first search.** The heap above is of searches after the first, with
  what searches keep already kept; the first search of each kind held a
  little more and kept what it read, 49.7 MiB against 48.9 for `prefix` of
  two letters at a million documents.
- **Times.** Other work ran on the machine throughout, the host's processors
  42–97 % busy at the start of each run (`environment.jsonl`), and a time's
  passes spread up to two or three times. Only the ratio of a search with a
  prefix to one without, 9 to 39,000 times, is larger than that spread; the
  times with and without a budget are not compared.

## Which fix the numbers favour, as a hypothesis

The finding sketches two fixes: reserve `count × 12` before reading and
answer `limit`, or merge the lists a block at a time.

A reservation of 12 bytes a posting would reserve a third of what the search
holds, 28–36 bytes for one prefix, unless the copies above go first. Even
exact, it turns the heap into refusals: under a budget of 16 MiB, `prefix`
of two letters would fail from about 350,000 documents of fifty words here
and `prefix_every` at every size measured, and with no budget a bound
of the call's own, as `ROWS_BYTES` is 64 MiB, would refuse them from 1.4 and
0.4 million documents; a search box that types `ea` gets an error rather than
documents, and how many documents an index may hold before it does depends
on its words.

Merging the 64 lists a block at a time would hold what a search of 64 whole
words holds, by the table above some 0.3 bytes a posting of their blocks'
bounds and a block of each, two orders less, though still growing with the
index. It is not free: a prefix's score uses the documents that hold any of
its words, which a merged list knows only once read, and a block's bound
over 64 words is weaker than a word's, so it may read as much and take as
long; that is the candidate's to measure, against `02231bd0` on this
fixture.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop 29.6.2's WSL2 VM,
16 CPUs and 15.2 GiB, on an AMD Ryzen 7 7700 under Windows 11 Pro
10.0.26300; `rust:1.99`, its `rustc 1.99.1` (TinyStore pins 1.99.0), each
container 2 CPUs and 6 GiB, the stores on a volume of their own. TinyStore
`02231bd0`, `default-features = false, features = ["search"]`; its bundled
SQLite 3.53.2 built with TinyStore's `LIBSQLITE3_FLAGS`, `SQLITE_DQS=0
-USQLITE_ENABLE_MEMORY_MANAGEMENT SQLITE_ENABLE_DBPAGE_VTAB
-DHAVE_FDATASYNC=1`, which the binary reads back from SQLite's compile
options in every run. The seed is 20261110. Builds from 10:07 to 10:14 UTC,
the measurement from 10:15 to 10:23 and counted in place from 10:28 to
10:33.

```sh
# from <repo>, tinystore/source at 02231bd0
sh tinystore/search-prefix-bench/round.sh tools     # the binary in the volume search-prefix-target
sh tinystore/search-prefix-bench/round.sh build     # eight indexes, about 8 minutes, 7 GB on search-prefix-data
sh tinystore/search-prefix-bench/round.sh measure   # each index without a budget and with 16 MiB
sh tinystore/search-prefix-bench/round.sh summary
REALLOC=in-place sh tinystore/search-prefix-bench/round.sh measure
REALLOC=in-place sh tinystore/search-prefix-bench/round.sh summary
```

On Windows, from `tinystore/search-prefix-bench`, a target directory
outside the repository:

```sh
CARGO_TARGET_DIR=<target> cargo build --release --locked
<target>/release/search-prefix-bench build <dir> 1000000 50 20261110
<target>/release/search-prefix-bench measure <dir> none 3 9
<target>/release/search-prefix-bench measure <dir> 16777216 3 9
```

Raw output is in [data/search-prefix-memory-2026-11-10](data/search-prefix-memory-2026-11-10/):
`builds.jsonl`, a line an index; `measure.jsonl` and `measure-in-place.jsonl`,
a line a search with its heap, reservation, SQLite's most, answer or time;
`environment.jsonl`, the machine and each run's start and load; and
`summary.md` and `summary-in-place.md`, every table above and more.
