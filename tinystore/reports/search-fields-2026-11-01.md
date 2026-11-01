# Two fields and prefixes over ordinary SQLite tables, on MS MARCO — 1 November 2026

**The harness of this round,
[bench/search-msmarco/fields](../bench/search-msmarco/fields/README.md) with
`src/fields.rs`, is at research commit `7216ae1`.** Its specification was
written before any time was taken.
[search-fast-2026-11-01](search-fast-2026-11-01.md) measured one field and
designed a layout for several and for prefixes without measuring it; this
round measures it. As there, every time is this machine's in WSL2's
containers, an order of engines and their ratios in one run, and the
prototype's, not TinyStore's own code.

| Question | Answer | What follows |
|---|---|---|
| Over two fields, how near Tantivy? | **The native layout passes Tantivy at p99**: 53.6–57.8 ms against 73.4–75.3, 0.73–0.78 of it; its p50 7.8–8.4 ms against 5.3–5.4, 1.44–1.56 times | — |
| And the lab's layout, keys with their field's name in front? | 3.7 times slower than the native at p99 and 2.5 times at p50: 198–209 ms and 20.1–21.4 ms | the native layout, not field prefixes in keys |
| With the last word a prefix, expanded to 32 words? | The same order: native p99 125–133 ms against Tantivy's 163–171 and the lab's 357–372; p50 16.5–17.3 against 12.3–12.7 and 34.3–35.3 | — |
| Does the native layout grow the index? | No: 1.99 bytes a posting over both fields' 699,299,014 postings, against the lab layout's 3.13 and Tantivy's 1.73 without positions, 3.01 with them | — |
| Are its answers the lab's? | Yes: on both sets every native top ten has the lab reader's scores within 1e-9, documents differing only where scores tie, 10 queries of 1,097 and 21 of 929 | scoring every posting was stopped after 30 minutes; a sample of it is still to run |

## The fields and the queries

MS MARCO has no titles; the lab keeps a body as two fields, and so does this
round: `b`, every token as written (`MSMARCO_TOKENS=surface`, 353,745,695
postings, 2,738,982 words), and `bs`, the stems search-msmarco prepared
(345,553,319 postings). A word asks compact.rs's forms in fields 2 and 3: as
written in `b`, its stem in `bs`, its stem in `b` where it differs, weighed
1.0 and 0.75. The 1,097 sample queries, 6,345 words; the prefix set, the 929
whose last word has four letters or more, cut to its first half and three
letters at least, expanded to the 32 words of `b` that begin with it and hold
the most documents, each engine in its own dictionary.

## Timed

Four rounds, the arms in one order and then the other, warm, one query at a
time, each container 2 CPUs and 6 GiB, 15:15–15:48 UTC
([summary.md](data/search-fields-2026-11-01/timing/20261101T151544Z/summary.md)).
p50 and p99 in ms, `a; b; c; d`, and against Tantivy's of the same round, the
first round's interval:

| set | arm | p50 | p99 | p50 / Tantivy | p99 / Tantivy |
|---|---|---|---|---|---|
| fields | lab | 20.19; 20.06; 21.39; 20.22 | 206.7; 198.3; 208.9; 205.2 | 3.78 [3.53, 3.94]; 3.69; 3.97; 3.84 | 2.75 [2.38, 3.24]; 2.70; 2.81; 2.74 |
| fields | **native** | **8.11; 7.84; 8.38; 7.82** | **57.0; 53.6; 57.8; 54.7** | 1.52 [1.42, 1.60]; 1.44; 1.56; 1.49 | **0.76 [0.71, 0.83]; 0.73; 0.78; 0.73** |
| fields | native, directories read again | 8.36; 9.08; 9.14; 8.96 | 56.8; 59.0; 59.9; 58.1 | 1.56 [1.47, 1.65]; 1.67; 1.70; 1.70 | 0.76 [0.70, 0.81]; 0.80; 0.80; 0.78 |
| fields | Tantivy | 5.35; 5.44; 5.39; 5.26 | 75.3; 73.4; 74.4; 74.9 | 1 | 1 |
| prefix | lab | 34.97; 34.95; 35.27; 34.31 | 356.9; 365.1; 372.0; 372.4 | 2.78 [2.64, 2.92]; 2.75; 2.86; 2.76 | 2.15 [1.94, 2.44]; 2.14; 2.21; 2.28 |
| prefix | **native** | **16.97; 16.90; 17.28; 16.51** | **128.6; 133.3; 127.6; 125.3** | 1.35 [1.27, 1.44]; 1.33; 1.40; 1.33 | **0.78 [0.68, 0.87]; 0.78; 0.76; 0.77** |
| prefix | native, directories read again | 17.56; 17.77; 18.49; 17.31 | 128.2; 133.7; 133.1; 126.7 | 1.40 [1.32, 1.49]; 1.40; 1.50; 1.39 | 0.77 [0.67, 0.84]; 0.78; 0.79; 0.78 |
| prefix | Tantivy | 12.59; 12.70; 12.31; 12.45 | 165.7; 170.5; 168.0; 163.2 | 1 | 1 |

Every arm's ids were the same in all four of its passes. Two fields cost
Tantivy 2.3 times its one field's p50 and 2.8 times its p99
([search-fast](search-fast-2026-11-01.md)); the native layout 2.2 and 2.0
times. Keeping directories between queries buys 5–10 % at p50 here, little at
p99.

## The space

Each layout's objects, both fields' postings together
([build.jsonl](data/search-fields-2026-11-01/logs/build.jsonl),
[build-native.jsonl](data/search-fields-2026-11-01/logs/build-native.jsonl)):

| layout | what a query reads | MiB | bytes a posting |
|---|---|---|---|
| lab | `cblob` 1,782.1, `ckeys` 236.6, `norms` 67.5 | 2,086.2 | 3.128 |
| native | `lists` 1,115.6, `dirs` 115.2, `words` 81.5, `lengths` 17.0 | 1,329.3 | 1.993 |
| Tantivy | `.idx` 1,076.7, `.term` 61.1, `.fieldnorm` 16.9 | 1,154.7 | 1.731 |
| Tantivy, with `.pos` 855.7 | | 2,010.4 | 3.015 |

The native layout's first build kept a directory and ten seeds for each of
its 4,879,826 lists of 16 postings or fewer, and its `dirs` took 319.0 MiB;
such a list is now one row of varints with no directory, its bound and seed
taken from its postings when it is read, and `dirs` takes 115.2. The words a
field holds are one row each, `words`, 81.5 MiB for 3,504,826 words, where
the lab keeps a key a field and a word, 5,204,953 of them in 236.6 MiB of
`ckeys` with their directories and short lists.

## Answers

Both sets answered by the lab's reader, the native one and Tantivy
([answers/](data/search-fields-2026-11-01/answers/)). Against the lab's, the
native layout's ids differ in 10 of 1,097 queries and 21 of 929, every
difference where two documents' scores tie within 1e-14, two of them at the
tenth place with a document outside the other's ten. The lab's reader was
exact against scoring every posting in
[search-msmarco-2026-11-01](search-msmarco-2026-11-01.md) on one field; here
scoring every posting through the lab's reader ran 30 minutes without
finishing and was stopped, so the check against it waits for a sample.
Tantivy's top tens share 97.1 % of their documents with the lab's, its BM25
keeping a field's length in a byte.

## Does a boost cost Tantivy its fast path?

Tantivy's p99 grew 2.8 times from one field to two where the native
layout's grew 2.0, and each of Tantivy's term queries here is wrapped in a
`BoostQuery` for its field's weight, which might leave it off its block-max
path. A second run at research commit `2d450dc`, 16:27–16:47 UTC, asked it with
both fields weighed 1.0 and plain term queries (`FIELDS_EQUAL=1`), and the
native layout with the same weights, beside both as before
([summary.md](data/search-fields-2026-11-01/timing/20261101T162754Z/summary.md)):

| set | arm | p50 | p99 |
|---|---|---|---|
| fields | Tantivy, boosted | 5.62; 5.54; 5.68; 5.41 | 77.2; 75.4; 78.6; 77.7 |
| fields | Tantivy, equal and plain | 5.53; 5.32; 5.40; 5.32 | 75.6; 71.6; 75.2; 72.1 |
| fields | native, the lab's weights | 8.49; 8.04; 8.02; 8.00 | 56.7; 55.6; 58.7; 56.8 |
| fields | native, equal | 7.89; 7.99; 7.28; 7.51 | 53.0; 53.3; 50.5; 52.5 |
| prefix | Tantivy, boosted | 13.04; 12.87; 12.91; 13.22 | 177.0; 170.5; 174.9; 174.9 |
| prefix | Tantivy, equal and plain | 12.83; 12.70; 12.68; 12.45 | 171.2; 168.7; 168.0; 173.4 |
| prefix | native, the lab's weights | 17.39; 17.28; 16.35; 16.52 | 129.1; 128.4; 125.9; 132.1 |
| prefix | native, equal | 15.80; 15.94; 16.00; 15.92 | 114.8; 112.2; 116.2; 116.4 |

**No**: without boosts Tantivy is 1–7 % faster, its p50 0.94–0.99 and its
p99 0.93–0.99 of the boosted arm's in the same rounds. At equal weights the
native layout's p99 is 0.67–0.75 of Tantivy's on the sample and 0.67–0.69
on the prefixes, its p50 1.35–1.50 and 1.23–1.28 times. The lead at p99
stands; why Tantivy's tail grows more with a second field — 18 term queries
for six words — is not measured here.

## Limits

- One machine, in WSL2's VM; the prototype's code, not TinyStore's, which
  keeps its scores in whole points.
- Reads of a settled index; the native layout has no writer.
- A prefix's 32 words are a choice of this round; Tantivy's and Lucene's own
  prefix queries score differently, and none of theirs is measured.

## Environment and reproduction

As in [search-fast-2026-11-01](search-fast-2026-11-01.md#environment-and-reproduction).
The surface tokens prepared 14:32–14:35 UTC, the layouts built 14:37–14:42,
the native layout again 14:43, the answers 14:44–14:46.

```sh
# fields/README.md's builds, then
sh tinystore/bench/search-msmarco/fields/timing.sh
```
