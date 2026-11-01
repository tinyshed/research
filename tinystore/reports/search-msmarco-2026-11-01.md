# Search over MS MARCO passage v1, without a clock — 1 November 2026

**The harness of this round,
[bench/search-msmarco](../bench/search-msmarco/), is at research commit
`5c6c363`, and no time in this report is a measurement.** The machine was
not quiet: a GPU training job and another agent's container builds ran
beside every step, so by the `measure` skill a time taken tonight is not
evidence. What this round measures is what no clock moves: answers, scores,
effectiveness and bytes. Its timing harness is written and was run once on
20 queries, as a smoke test; every latency it printed is labelled so, and
none is quoted below as a result.

A ranking of TinyStore's paper candidates, kept outside this repository, puts second a paper on
exact top-k search by block-max MaxScore over SQLite's pages, and names what
it lacks: MS MARCO, PISA and Lucene beside Tantivy, one BM25 fixed in every
engine. This round puts the compact blocks of
[search-compact-2026-10-16](search-compact-2026-10-16.md) on MS MARCO's
8,841,823 passages beside FTS5, Tantivy, PISA and Lucene, every engine
indexing one token stream with k1 1.2 and b 0.75, and tests the two
hypotheses of candidates.md that need no clock: H2.2, exact and equally
effective, and H2.4, competitive in space. Nothing here is a product promise.

| Question | Answer | What follows |
|---|---|---|
| H2.2: is every top ten exact? | Yes. Every engine's ten scores equal exhaustive scoring on every query asked: 6,980 of 6,980 dev queries for the compact blocks, FTS5, Tantivy, Lucene and both PISA algorithms against an exhaustive scorer written apart from all of them, and 1,097 of 1,097 sample queries for every executor against its own engine scoring every match | The pruning is safe in all five engines; documents differ only where scores tie, in 0 to 82 of 6,980 queries an engine |
| H2.2: equally effective at the same BM25? | Within a BM25 variant, yes: MRR@10 on dev 0.1800 for the compact blocks, 0.1808 for Tantivy and Lucene, 0.1785 for FTS5 and 0.1785–0.1786 for PISA, each its variant's exhaustive value to 0.00013. Across variants, no: 0.1785 to 0.1808, b below l by 0.0024 [0.0010, 0.0038] | Report every engine with its variant; the difference is the idf of seven terms, shown by a split of the queries |
| H2.4: is the space competitive? | Against FTS5, yes; against the dedicated engines, no. What a top-ten query reads is 3.18 bytes a posting in the compact blocks, 1.70 in Tantivy, 1.61 in Lucene, 2.11 in PISA and 4.21 in FTS5, which cannot keep frequencies without positions | The blocks' varint payload, 2.27 bytes a posting, and 18 % of page overhead are the gap; a bit-packed block is the next round's candidate |
| H2.1: within two times Tantivy at p99? | Not measured. [timing.sh](../bench/search-msmarco/timing.sh) is written and ran once on 20 queries, a smoke test | Run it on a quiet machine, two to two and a half hours, then on bare Linux ; ran on 1 November in WSL2's containers, [search-msmarco-timing-2026-11-01](search-msmarco-timing-2026-11-01.md): 3.0–3.5 times Tantivy at p99 there |
| H2.3: do writes stay cheap under reads? | Not measured. [churn.sh](../bench/search-msmarco/churn.sh) is written and ran once for three seconds an engine, a smoke test | Run it on a quiet machine, about 45 minutes |
| H2.5: does the vector arm reproduce RaBitQ? | Not started | Its arms assume bge-m3's 1,024 normalized dimensions; SIFT and GIST are Euclidean |

## One token stream for every engine

MS MARCO passage v1 from MS MARCO's own distribution: the 8,841,823 passages,
the 6,980 small dev queries with their 7,437 judgments, and the TREC
deep-learning 2019 and 2020 passage queries with NIST's graded judgments, of
which 43 and 54 queries are judged. Every file is checked against
[msmarco-sha256.txt](../bench/msmarco-sha256.txt). The whole collection is
indexed; no subset was needed.

Each passage goes through the lab's analyzer, `auto` of
[search-plan-2026-10-16](search-plan-2026-10-16.md) — NFKC, lower case,
Latin accents folded, words by Unicode's rules — and each token becomes its
English Snowball stem where it has one: one field, no stop words. That gives
505,731,319 tokens, 57.20 a passage, 2,465,971 terms and 345,553,319
postings, a term and a passage each; the longest token is 249 bytes. Every
engine indexes exactly these tokens: FTS5, Tantivy and Lucene read them
joined by spaces and split them on spaces, PISA reads its binary collection
written from the same stream, and the compact blocks are built from that
collection. Every thousandth passage was analysed again from its joined
tokens, as an engine that splits on spaces sees it: 8,842 of 8,842 came back
unchanged. Each build counted its postings again: 345,553,319 in every one.

A query is asked as the same tokens, each distinct term once, a term no
passage holds left out, which adds nothing in any engine: 32 such terms in
dev, one in DL19, none in DL20; a dev query keeps 5.83 terms on average and
none is left empty. The sample of the exactness checks is the 1,000 dev
queries of lowest SHA-256 of their qid and the 97 judged TREC queries, 1,097
in all.

A passage is document pid in PISA and Lucene and pid + 1 in the SQLite
stores and Tantivy, and in every engine the documents follow pid order:
Tantivy indexes on one thread and merges its segments in pid order, Lucene
indexes on one thread and merges only adjacent segments, and both builds
check that every document's number follows its pid.

## The engines

| Engine | Index | A query | Its exhaustive control | BM25 |
|---|---|---|---|---|
| compact | the lab's compact format, built streamed (below) | the window executor, eight blocks read ahead; also the window alone, the cursor and the lab's own reader | the lab's `exhaustive` executor | a |
| FTS5, SQLite 3.53.2 | one column, `ascii` tokenizer, contentless with deletes, optimized into one segment | an OR of one-token phrases, `order by rank limit 10` | itself: FTS5 scores every match | b |
| Tantivy 0.26.2 | one field with frequencies and positions, one segment | a union of term queries, `TopDocs`: block-max WAND | `(TopDocs, Count)`, which scores every match | t |
| PISA v0.10.0 | `block_simdbp`, WAND data in fixed blocks of 64, PISA's default | `block_max_maxscore` and `block_max_wand` | `ranked_or` | b |
| Lucene 10.5.2, codec Lucene104 | one field with frequencies and positions, one segment | `IndexSearcher.search(query, 10)`: exact hit counts to 1,000, then windowed MaxScore | every hit counted, `totalHitsThreshold` the largest int | l |

No engine stores the passages; the SQLite stores keep each document's pid
as its key. The compact blocks are the format and executors of
search-compact-2026-10-16, compiled from
[search-plan-lab](../search-plan-lab/README.md)'s sources by path, unchanged,
with two things of their own in [compact1.rs](../bench/search-msmarco/src/compact1.rs):
a build that streams one term at a time from the prepared collection, where
the lab's holds every list in memory, and a reader that asks one key a term,
`b|term`, where the lab's asks four. Built both ways over the first 200,000
passages, the lab's build and the streamed one wrote the same 166,866 key
rows, 67,198 blocks, 4 rows of lengths and 4 of statistics, 0 rows
differing; and the lab's own four-key reader answers the full build's sample
queries exactly as the one-key reader does.

PISA's queries run through [pass.cpp](../bench/search-msmarco/pisa/pass.cpp),
which builds PISA's cursors, scorer and algorithms exactly as its `queries`
tool does but times each query once, as the other engines are timed; the
tool itself warms every posting list first and times a query as the mean of
two runs after an untimed one. FTS5's and Tantivy's k1 and b are fixed in
their sources at 1.2 and 0.75; PISA and Lucene were given them.

[exact.rs](../bench/search-msmarco/src/exact.rs) scores every posting of a
query in f64, apart from every engine, under each variant the engines
compute; a term adds idf × tf (k1 + 1) / (tf + k1 (1 − b + b · len / avg)):

| Variant | idf | A document's length | Engines |
|---|---|---|---|
| a | ln(1 + (N − df + ½) / (df + ½)), Lucene's | exact | the compact blocks |
| b | ln((N − df + ½) / (df + ½)), at least 10⁻⁶ | exact | FTS5, PISA |
| l | Lucene's | one byte, Lucene's `SmallFloat.intToByte4` | Lucene |
| t | Lucene's | one byte, Tantivy's `FIELD_NORMS_TABLE` | Tantivy |

Tantivy's table turned out to be Lucene's: no length of the 8,841,823
passages decodes differently, so l and t rank alike to the last digit.
Lucene leaves out the constant k1 + 1, and its scores are multiplied by 2.2
before they are compared.

## Every top ten is exact

Two tens are the same when their scores agree position by position within
the engine's precision, 10⁻⁹ of a score for the f64 engines and 2 × 10⁻⁵ for
the f32 ones; their documents may then differ only where scores tie. MS
MARCO holds many near-duplicate passages, whose scores tie exactly, and f32
makes more ties than f64.

Each pruned executor against its own engine scoring every match:

| Engine | Queries | Same ten scores | Same documents | Documents differ at equal scores | Scores differ | Worst relative difference |
|---|---|---|---|---|---|---|
| compact, window, read ahead 8 | 1,097 | 1,097 | 1,095 | 2 | 0 | 4.3 × 10⁻¹⁶ |
| compact, window | 1,097 | 1,097 | 1,095 | 2 | 0 | 4.3 × 10⁻¹⁶ |
| compact, cursor | 1,097 | 1,097 | 1,096 | 1 | 0 | 4.3 × 10⁻¹⁶ |
| compact, the lab's reader | 1,097 | 1,097 | 1,095 | 2 | 0 | 4.3 × 10⁻¹⁶ |
| Tantivy | 1,097 | 1,097 | 1,095 | 2 | 0 | 2.3 × 10⁻⁷ |
| PISA, `block_max_maxscore` | 6,980 | 6,980 | 6,973 | 7 | 0 | 3.2 × 10⁻⁷ |
| PISA, `block_max_wand` | 6,980 | 6,980 | 6,958 | 22 | 0 | 3.3 × 10⁻⁷ |
| Lucene | 1,097 | 1,097 | 1,097 | 0 | 0 | 0 |

Each engine against exact.rs under its variant:

| Engine | Variant | Queries | Same ten scores | Same documents | Documents differ at equal scores | Scores differ | Worst relative difference |
|---|---|---|---|---|---|---|---|
| compact, window, read ahead 8 | a | 6,980 | 6,980 | 6,976 | 4 | 0 | 5.8 × 10⁻¹⁶ |
| compact, the lab's reader | a | 1,097 | 1,097 | 1,095 | 2 | 0 | 4.9 × 10⁻¹⁶ |
| compact, exhaustive | a | 1,097 | 1,097 | 1,097 | 0 | 0 | 4.3 × 10⁻¹⁶ |
| FTS5 | b | 6,980 | 6,980 | 6,980 | 0 | 0 | 4.8 × 10⁻¹⁶ |
| PISA, `block_max_maxscore` | b | 6,980 | 6,980 | 6,909 | 71 | 0 | 2.6 × 10⁻⁷ |
| PISA, `block_max_wand` | b | 6,980 | 6,980 | 6,898 | 82 | 0 | 3.5 × 10⁻⁷ |
| PISA, `ranked_or` | b | 6,980 | 6,980 | 6,909 | 71 | 0 | 2.5 × 10⁻⁷ |
| Tantivy | t | 6,980 | 6,980 | 6,960 | 20 | 0 | 2.6 × 10⁻⁷ |
| Lucene | l | 6,980 | 6,980 | 6,980 | 0 | 0 | 2.1 × 10⁻⁷ |

Every executor of every engine returned the exhaustive ten scores on every
query it was asked. The f64 engines agree with exact.rs to rounding; the f32
ones to a few parts in ten million, the precision of f32. Tantivy indexed
again with a key field for H2.3's writer gave the same documents and scores
as the index without it in all 9,271 answers of its five runs.

## Equally effective, within a variant

MRR@10 and nDCG@10 a set, each run ordered as trec_eval orders one — by
score, a tie by document id as a string, descending — so that the order an
engine returns equal scores in does not count. On dev every passage judged
is relevant; on the TREC sets nDCG@10 takes the grades 0–3 as gains and
MRR@10 counts a passage graded 2 or more, as TREC's passage task does.

| Engine | Variant | dev MRR@10 | dev nDCG@10 | DL19 MRR@10 | DL19 nDCG@10 | DL20 MRR@10 | DL20 nDCG@10 |
|---|---|---|---|---|---|---|---|
| compact | a | 0.1800 | 0.2254 | 0.6810 | 0.4848 | 0.6198 | 0.4728 |
| exhaustive a | a | 0.1800 | 0.2254 | 0.6810 | 0.4848 | 0.6198 | 0.4728 |
| FTS5 | b | 0.1785 | 0.2236 | 0.6690 | 0.4761 | 0.6121 | 0.4748 |
| PISA, `block_max_maxscore` | b | 0.1785 | 0.2237 | 0.6690 | 0.4761 | 0.6121 | 0.4748 |
| PISA, `block_max_wand` | b | 0.1786 | 0.2237 | 0.6690 | 0.4761 | 0.6121 | 0.4748 |
| PISA, `ranked_or` | b | 0.1785 | 0.2237 | 0.6690 | 0.4761 | 0.6121 | 0.4748 |
| exhaustive b | b | 0.1785 | 0.2236 | 0.6690 | 0.4761 | 0.6121 | 0.4748 |
| Tantivy | t | 0.1808 | 0.2260 | 0.6798 | 0.4864 | 0.6118 | 0.4732 |
| Lucene | l | 0.1808 | 0.2260 | 0.6798 | 0.4864 | 0.6118 | 0.4732 |
| exhaustive l and t | l, t | 0.1808 | 0.2260 | 0.6798 | 0.4864 | 0.6118 | 0.4732 |

Against its variant's exhaustive run, an engine's reciprocal rank differs on
dev in 1 query for the compact blocks, 0 for FTS5, 5 for Tantivy, 0 for
Lucene, and 5, 9 and 12 for PISA's `ranked_or`, `block_max_maxscore` and
`block_max_wand`, each from a tie at the tenth score; the
largest difference in a mean is 0.00013, PISA's `block_max_wand`. Within a
variant the engines are equally effective.

Across variants they are not. On dev, paired over the 6,980 queries with
10,000 resamples:

| Variants | MRR@10 difference | 95 % interval | nDCG@10 difference | 95 % interval |
|---|---|---|---|---|
| a − b | +0.0016 | [+0.0003, +0.0029] | +0.0018 | [+0.0005, +0.0032] |
| a − l | −0.0008 | [−0.0017, +0.0000] | −0.0006 | [−0.0014, +0.0002] |
| b − l | −0.0024 | [−0.0038, −0.0010] | −0.0024 | [−0.0039, −0.0010] |

Variants a and b differ only in idf, and most where a term is in more than
half the passages: there b's idf is at most zero and is floored to 10⁻⁶,
while a's stays between 0.14 and 0.61. Seven terms are — the, of, and, a,
to, in, is — and 4,732 of the 6,980 dev queries hold one. If the floor is
what parts a from b, the other queries should rank alike under both:

| Dev queries | Count | MRR@10 a | MRR@10 b | MRR@10 l | a − b | 95 % interval | Ranks differ, a and b |
|---|---|---|---|---|---|---|---|
| holding one of the seven terms | 4,732 | 0.1796 | 0.1773 | 0.1800 | +0.0023 | [+0.0005, +0.0041] | 560 |
| holding none | 2,248 | 0.1808 | 0.1808 | 0.1826 | +0.0000 | [−0.0010, +0.0010] | 70 |

They do: the whole difference between a and b sits in the queries that hold
one of the seven, which is the measurement that separates the floor from the
rest of the two idfs. Between a and l, which differ only in a length kept
in a byte, the difference is within its interval of zero. This is the
variant effect `kamphuis2020bm25` describes; a paper must name each engine's
variant rather than quote one BM25.

## The space

Every object in bytes a posting, the same 345,553,319 for every engine.
SQLite's objects are dbstat's pages; Tantivy's, Lucene's and PISA's their
files, which each keep a part of the index apart. Objects under 1 MiB are
left out of the table.

| Engine | Object | What it holds | MiB | Bytes a posting |
|---|---|---|---|---|
| compact | `cblob` | blocks: documents and frequencies | 870.1 | 2.640 |
| compact | `ckeys` | keys, directories, short lists inline | 110.5 | 0.335 |
| compact | `norms` | lengths | 67.5 | 0.205 |
| compact | `docs`, its key index | document keys | 158.0, 148.1 | 0.479, 0.449 |
| FTS5 | `idx_data` | postings with positions, term dictionary | 1,268.1 | 3.848 |
| FTS5 | `idx_docsize` | lengths | 116.8 | 0.354 |
| Tantivy | `.idx` | documents and frequencies, skips, block maxima | 521.6 | 1.583 |
| Tantivy | `.pos` | positions | 427.4 | 1.297 |
| Tantivy | `.term` | term dictionary | 28.7 | 0.087 |
| Tantivy | `.fieldnorm` | lengths | 8.4 | 0.026 |
| Tantivy | `.store` | document store, its documents empty | 33.9 | 0.103 |
| Lucene | `.doc` | documents and frequencies, skips, impacts | 498.5 | 1.513 |
| Lucene | `.pos` | positions | 412.1 | 1.251 |
| Lucene | `.tim` | term dictionary | 23.3 | 0.071 |
| Lucene | `.nvd` | lengths | 8.4 | 0.026 |
| Lucene | `.dvd` | document keys | 16.9 | 0.051 |
| PISA | index | documents and frequencies | 514.1 | 1.560 |
| PISA | WAND data | block maxima, lengths, term statistics | 139.7 | 0.424 |
| PISA | term lexicon | term dictionary | 40.2 | 0.122 |
| PISA | document lexicon | document keys | 125.4 | 0.381 |

What a top-ten query reads — the postings and what finds and bounds them,
without positions, document keys or a document store:

| Engine | MiB | Bytes a posting | Against the compact blocks | Positions, MiB | Everything, MiB |
|---|---|---|---|---|---|
| compact | 1,048.1 | 3.180 | 1 | — | 1,354.2 |
| FTS5 | 1,385.6 | 4.205 | 1.32 | in its postings | 1,385.6 |
| Tantivy | 558.7 | 1.695 | 0.53 | 427.4 | 1,020.2 |
| Lucene | 530.6 | 1.610 | 0.51 | 412.1 | 959.8 |
| PISA | 694.0 | 2.106 | 0.66 | — | 819.4 |

The compact blocks read 1.9 times Tantivy's bytes and 2.0 times Lucene's,
1.5 times PISA's, and 0.76 times FTS5's, whose postings cannot keep
frequencies without positions. With their positions counted, Tantivy and
Lucene hold 986.1 and 942.7 MiB, 6 and 11 % below the compact blocks
without theirs.

The gap is in the blocks. Their payload is 772,967,147 bytes for the
340,730,197 postings in blocks, 2.27 bytes a posting as varints of a
document's delta and its frequency, against 1.56 for PISA's SIMD-BP128 index
whole; and their pages hold 912,347,136 bytes for that payload, 18 % more, a
WITHOUT ROWID b-tree's cells and fill. Everything else is small: 2,322,700 of
the 2,465,971 keys keep their 4,823,122 postings inline, 23.7 MiB; the keys
26.1 MiB, the directories with every block's bounds 15.0 MiB, the lengths
0.20 bytes a posting as two bytes a field a document. The documents' keys,
306 MiB in two b-trees, are a store's map from a key to a number, which no
other engine here keeps in its index.

H2.4 as candidates.md states it — competitive with PISA's index, Tantivy's
segments and FTS5 — holds against FTS5 and fails against the three
dedicated engines, by a factor of 1.5 to 2.0, which the blocks' encoding and
their pages account for. This is a division of the file, measured once; a
bit-packed block in place of varints is the obvious candidate, and only a
round that builds one can say what it saves in pages rather than payload.

## The timing harness, and its smoke test

[timing.sh](../bench/search-msmarco/timing.sh) runs H2.1 with one command
once round.sh has built the tools and the indexes. Five arms, each a
process of its own a pass, one query at a time, each container 2 CPUs and
6 GiB as in the search rounds before: A the compact blocks, window executor
reading ahead 8; B FTS5; C Tantivy; D PISA's `block_max_maxscore` and then
its `block_max_wand`; E Lucene. Rounds A B C D E and then E D C B A, cold
first and then warm, four rounds. A cold pass evicts its arm's files from
the Linux VM's page cache first, POSIX_FADV_DONTNEED through
`dd iflag=nocache`, and logs the bytes cached before and after by fincore;
it then times each query once. A warm pass runs every query once untimed
and then times each once. Every arm but FTS5 answers the 1,097 sample
queries, and every arm answers their first 200, the only set FTS5 is given,
since it takes seconds a query. [bootstrap.py](../bench/search-msmarco/bootstrap.py)
turns the passes into p50 and p99, nearest rank, each with a percentile
interval from 10,000 resamples of the queries, and each arm's p50 and p99
over Tantivy's of the same round, resampled in pairs; and it checks that an
arm's ids are the same in every pass.

It ran once, on the first 20 sample queries and FTS5's first 2, labelled
`smoke test, not a measurement`, from 01:07:04 to 01:10:21 UTC, beside the
night's other jobs: 44 passes, eviction taking each arm's files from
0.18–1.40 GB cached to 0–37 MB, the 37 MB FTS5's while four other FTS5
processes were reading the same file, and every arm's ids the same in all
four of its passes. Its Tantivy arm read the index with the key field, the
only one built then. Its output is in [timing/smoke-20261101](data/search-msmarco-2026-11-01/timing/smoke-20261101/summary.md);
its latencies are not quoted here, and must not be.

On a quiet machine the harness should take two to two and a half hours.
FTS5 answered the 43 DL19 queries in 272 s tonight, 6.3 s a query, and the
other 5,980 dev queries in six processes of about a thousand each in 4,909
to 5,643 s a process, 5.0 to 5.6 s a query; in the harness its 200 queries
are timed in four passes and run untimed in two more, 1,200 queries, 1.7 to
2.1 hours at those rates. Every other arm adds minutes. Those rates come
from a busy machine and are not a measurement either.

What the harness cannot settle alone: one call in flight in WSL2's VM,
which the `measure` skill says to quote latencies of only from bare Linux;
a JVM started cold in Lucene's cold passes; and one machine. H2.1's two
machines remain.

## The writing harness, and its smoke test

[churn.sh](../bench/search-msmarco/churn.sh) runs H2.3 the same way: for the
compact blocks, FTS5 and Tantivy, a fresh copy of the index and nine cells
on it, 10, 100 and 1,000 sets a second each with 1, 4 and 16 readers, 60
seconds a cell after five of readers alone. The writer is open loop: every
100 ms it commits the sets that have come due, each a random passage given
another random passage's tokens, and a set's latency runs from when it was
due to when its commit returned. The compact blocks pack their head beside
the writes, a step of 4,000 head rows after any commit that leaves 20,000 or
more, timed as the writer's. Readers ask the sample's queries in a seeded
order and see the writer's commits as a store's readers would: the compact
reader reads its statistics and the gone again when SQLite's data_version
moves, FTS5 reads each query in a new snapshot, Tantivy's reader reloads on
commit. Tantivy's copy is the index with a key field to delete by.

It ran once, three seconds an engine at 10 sets a second with two readers,
from 01:13:58 to 01:15:48 UTC, labelled `smoke test, not a measurement`; all
three engines set their 30 passages and answered beside them, and the
compact head held 1,127 rows at the end. Its lines are in
[churn/smoke-20261101](data/search-msmarco-2026-11-01/churn/smoke-20261101/cells.jsonl).
A full run should take about 45 minutes: 27 cells of 65 seconds and the
copies and start of each.

## H2.5 was not started

Its non-timing half, recall@10 of each arm on SIFT 1M and GIST 1M, needs
the lab's vector arms on Euclidean vectors of 128 and 960 dimensions, and
those arms are built for bge-m3's: `vectors.rs` fixes 1,024 dimensions,
clusters by spherical k-means and scores by inner product, as do its usearch
and vec1 controls. Porting them, building RaBitQ's own library and hnswlib,
and fetching the two corpora was more than the night left.

## What this round does not show

- Any latency: H2.1 and H2.3 are harnesses, not results.
- Other BM25 parameters, stop words, several fields, phrases: one field of
  stems, k1 1.2 and b 0.75, the values FTS5 and Tantivy fix.
- The compact format's positions, which this round did not build, and PISA
  with variable blocks or other encodings.
- A second machine. Answers and bytes are deterministic and did not need a
  second pass; every count above is from one build.

## What follows

- Run `timing.sh` and `churn.sh` on a quiet machine, and then on a second
  machine on bare Linux; only then is H2.1 or H2.3 a result.
- H2.2 holds as stated once each engine is named with its variant: a, b or
  l. The paper's effectiveness table should be this one.
- H2.4 needs a round of its own: the blocks' payload bit-packed, and their
  rows sized to the page, each measured as a division of the file.
- H2.5 needs the vector arms on Euclidean vectors before it can be asked.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop 29.6.2's WSL2 VM,
16 CPUs and 15.2 GiB, on an AMD Ryzen 7 7700 under Windows 11; a GPU
training job and another agent's containers ran beside the round. Rust
1.99.1 in `rust:1.99` with rusqlite 0.40.2 and its bundled SQLite 3.53.2,
every SQLite file opened as TinyStore opens one (4 KiB pages, WAL,
`synchronous=full`, a 1 MiB cache); Tantivy 0.26.2; PISA v0.10.0 at
`cf0cc78f94d0d4a8dceac637dbbbb519c3803976`, built in `gcc:13` (GCC 13.5.0,
CMake 3.25.1 from Debian) as PISA's own CI builds it, Release with
`-march=native`; Lucene 10.5.2 from Maven Central, checked against
[jars.sha256](../bench/search-msmarco/lucene/jars.sha256), on Temurin
21.0.13 in `eclipse-temurin:21-jdk`. The analyzer, blocks and compact format
are [search-plan-lab](../search-plan-lab/README.md)'s sources at research
`d107510`, last changed in `91b9db5`; the TinyStore submodule, at `e307c48`,
is not built by this round, whose prototype, like the search rounds before
it, is Rust over rusqlite.

| UTC | Step | Raw output |
|---|---|---|
| 00:35:39–00:39:22 | prepare and queries | [logs/prepare.log](data/search-msmarco-2026-11-01/logs/prepare.log) |
| 00:44–00:51 | the compact, FTS5, PISA, Tantivy and Lucene builds | logs/build-*.jsonl |
| 00:45–01:53 | every engine's answers but FTS5's dev | [answers/](data/search-msmarco-2026-11-01/answers/), logs/answers.jsonl |
| 00:56–01:39 | FTS5 on the sample, four processes | answers/fts5-sample.jsonl.gz |
| 01:32–03:05 | FTS5 on the other 5,980 dev queries, six processes | answers/fts5-dev-rest.jsonl.gz, joined into fts5-dev |
| 00:55–01:20 | exact.rs on the sample and dev | answers/exact-*.jsonl.gz, logs/exact.jsonl |
| 01:07:04–01:10:21 | timing.sh, smoke test | timing/smoke-20261101 |
| 01:13:58–01:15:48 | churn.sh, smoke test | churn/smoke-20261101 |
| 01:13–01:18 | Tantivy without the key field, and its answers again | logs/build-tantivy.jsonl |

The harness is in [tinystore/bench/search-msmarco](../bench/search-msmarco/):
[round.sh](../bench/search-msmarco/round.sh) runs every step above in its
container, and [evaluate.py](../bench/search-msmarco/evaluate.py),
[space.py](../bench/search-msmarco/space.py) and
[variants.py](../bench/search-msmarco/variants.py) write every table here
from the raw output, into h22.json, h24.json and variants.json beside it. A
per-query answer file keeps each query's ten pids and scores; its times come
from a busy machine and are not a measurement.

```sh
sh tinystore/bench/search-msmarco/fetch.sh <corpus>     # <corpus> = tinystore/bench/corpus/msmarco
PROXY=<proxy> sh tinystore/bench/search-msmarco/round.sh tools
sh tinystore/bench/search-msmarco/round.sh prepare
for engine in compact fts5 tantivy tantivy-keyed lucene pisa; do
  sh tinystore/bench/search-msmarco/round.sh build $engine
done
for set in dl19 dl20 sample dev; do
  for engine in compact tantivy lucene pisa-block_max_maxscore pisa-block_max_wand pisa-ranked_or; do
    sh tinystore/bench/search-msmarco/round.sh answers $engine $set
  done
done
sh tinystore/bench/search-msmarco/round.sh exact sample
sh tinystore/bench/search-msmarco/round.sh exact dev
for engine in compact-window compact-cursor compact-lab compact-exhaustive tantivy-exhaustive lucene-exhaustive; do
  sh tinystore/bench/search-msmarco/round.sh answers $engine sample
done
sh tinystore/bench/search-msmarco/round.sh answers-parallel fts5 sample 4
sh tinystore/bench/search-msmarco/round.sh dev-rest
sh tinystore/bench/search-msmarco/round.sh answers-parallel fts5 dev-rest 6
python -I tinystore/bench/search-msmarco/merge.py tinystore/reports/data/search-msmarco-2026-11-01
sh tinystore/bench/search-msmarco/round.sh sizes
MSYS_NO_PATHCONV=1 docker run --rm -v <repo>:/src:ro -v search-msmarco-perf:/perf golang:1.27 \
  python3 -I /src/tinystore/bench/search-msmarco/frequent.py /perf/msmarco \
  > tinystore/reports/data/search-msmarco-2026-11-01/logs/frequent.jsonl
python -I tinystore/bench/search-msmarco/evaluate.py tinystore/reports/data/search-msmarco-2026-11-01 <corpus>
python -I tinystore/bench/search-msmarco/space.py tinystore/reports/data/search-msmarco-2026-11-01
python -I tinystore/bench/search-msmarco/variants.py tinystore/reports/data/search-msmarco-2026-11-01 <corpus> <dev.tsv>

# on a quiet machine
sh tinystore/bench/search-msmarco/timing.sh
sh tinystore/bench/search-msmarco/churn.sh
```

`<dev.tsv>` is the dev queries' tokens that `round.sh prepare` writes to the
volume at /perf/msmarco/q/dev.tsv; it is the corpus's and stays out of the
repository, as the tokens and indexes do. FTS5's dev answers are its
sample's 1,000 dev queries and the other 5,980, which `merge.py` joins and
checks hold every dev qid once. FTS5 also answered DL19 on its own,
`round.sh answers fts5 dl19`, the run its 6.3 s a query comes from.
