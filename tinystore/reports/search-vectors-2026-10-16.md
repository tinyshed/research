# Vectors beside the words — 16 October 2026

[search-compact-2026-10-16](search-compact-2026-10-16.md) answered a million
passages by their words, exactly, from blocks of our own in SQLite. This
round embeds the same million and asks the same 600 questions by their
vectors: exactly, by a bound on what each vector's code leaves out, and
approximately, from the clusters nearest a question, against three controls
built from the same rows; then it judges the words, the vectors and their
fusion on the questions' judged passages. Nothing here is a product promise.

| Question | Answer |
|---|---|
| What does an exact answer cost? | 245–274 ms a question at a million on 2 CPUs, against sqlite-vec's 1,124–1,145: every byte code read, 988 MiB, and 25 vectors scored again; 0 of 100 differ. The clusters' caps prune nothing: 4,094 of 4,096 read |
| How near the controls? | At recall 0.90 of the exact ten, our lists answer in 1.93–2.01 ms, vec1 in 1.31–1.37, HNSW in 0.21–0.22; at 0.93–0.94, ours 2.65–2.96, vec1 (0.928) 2.14–2.21, HNSW (0.949) 0.34–0.41. Readers held 96 MiB, 80 and 2,220–2,266 |
| Which code a cluster's list keeps? | A bit a dimension: RaBitQ's or the signs' read 7.4 times fewer bytes than a byte a dimension and reach the same recall three times sooner; RaBitQ needs a quarter of the signs' vectors scored again for 0.93, and at equal time the two tie |
| What do vectors add to the words? | bge-m3's vectors alone reach nDCG@10 0.736, the words 0.432; their fusion by rank 0.641, 0.719 with the vectors weighing four times. Here fusion never beats the vectors, whose model was trained on MIRACL |
| And without a vector index? | The words' hundred best scored again on their vectors reach 0.687, 93 % of the vectors alone, at 2.2–2.9 ms, with no clusters and no graph |

## The vectors

BAAI/bge-m3's dense vectors, revision `5617a9f`, 1,024 dimensions of length
one, through sentence-transformers 6.1.0 and torch 2.11.0+cu128 in float16
on an RTX 5070: a passage's title and text joined by a line break and cut at
512 tokens, a question as it is. The million passages took 121,628,127 tokens
and 2,315 s, the 600 questions 7,230 tokens. [embed.py](../search-plan-lab/embed.py)
wrote them in the order every index numbers the passages, which `lab export`
writes; [vectors.sha256](../search-plan-lab/vectors.sha256) checks them.

bge-m3 was trained with MIRACL's training questions among its data, so these
dev questions are in its domain: its lead over the words here is an upper
bound for a model meeting a collection it never saw.

## The lists

One SQLite file, opened as TinyStore opens one, holds every vector as given,
a row a passage, and 4,096 clusters from spherical k-means over 131,072 of
them, eight rounds; the mean cosine of a vector to its centroid grew from
0.576 to 0.715. A cluster holds 219 members at the median, 724 at the 99th
percentile, 1 to 1,273. Each cluster's members are kept in rows of up to
256, three ways:

- **codes**: a byte a dimension, scaled by the vector's largest magnitude,
  and the length of what the bytes leave out. A question's product with the
  vector lies within the question's length times that of the code's: a
  range, which the exact arm uses.
- **signs**: a bit a dimension, the sign of each.
- **RaBitQ**: a bit a dimension of the vector's offset from its cluster's
  centre, turned by a fixed random rotation (three rounds of random signs and
  a Walsh–Hadamard transform), with the offset's length and the cosine of the
  offset with what its bits stand for, the estimate's divisor. A question is
  turned once and held in four bits a dimension, so that an estimate costs
  five popcounts a 64-bit word ([Gao and Long, SIGMOD 2024](https://arxiv.org/abs/2405.12497)).

Every arm scores its candidates again on the vectors as given, so that an
answer's scores are exact even where its set is not.

- `exact`: clusters in the order of what their members could score at most,
  a cap around each centroid as wide as its furthest member; every member's
  code gives a range, and every member whose range reaches the k-th best
  lower end is scored again. A cluster whose cap falls below that lower end
  stops the search.
- `codes:P:R`, `signs:P:R`, `rabitq:P:R`: the P clusters whose centroids lie
  nearest the question, the R best by the code scored again.

The controls, from the same rows:

- usearch 2.26.2's HNSW in memory, vectors in float16, 16 links a node,
  expansion 128 at build; `hnsw:EF` searches at expansion EF.
- sqlite-vec 0.1.9, as its crate builds it, without its AVX kernels: a vec0
  table that scans every vector.
- vec1 0.7, SQLite's own vector extension (check-in `8fc7b115`, which
  [fetch-vec1.sh](../search-plan-lab/fetch-vec1.sh) fetches), built with the
  AVX2 flags its manual gives: IVF over 1,024 buckets with 32-byte OPQ codes,
  trained on 102,400 vectors, as its manual advises for neural embeddings;
  `vec1:P:R` probes P buckets and ranks its R best again on their vectors in
  SQL, as its manual says to.

## Reads

Every arm a process of its own, a pass to warm its pages and one timed;
rounds in one order, then the other. Recall is the share of the exact ten
an arm found; nDCG@10 is judged on the questions' passages. `exact` asked the
first 100 questions, sqlite-vec the first 50, every other arm all 600.

| arm | p50 ms | p99 ms | recall@10 | nDCG@10 | MiB read | scored again | reader MiB |
|---|---|---|---|---|---|---|---|
| exact | 244.85; 273.59 | 258.80; 288.43 | 1.000 | 0.7937 | 988.00 | 25 | 96 |
| sqlite-vec | 1,124.28; 1,144.57 | 1,158.78; 1,181.82 | 0.996 | 0.7735 | — | — | 72 |
| codes:32:40 | 2.79; 3.02 | 3.89; 4.06 | 0.844 | 0.6708 | 9.34 | 40 | 96 |
| rabitq:32:100 | 1.65; 1.41 | 3.37; 1.86 | 0.844 | 0.6708 | 1.26 | 100 | 96 |
| codes:64:40 | 4.81; 5.43 | 6.24; 6.83 | 0.900 | 0.6960 | 18.19 | 40 | 96 |
| rabitq:64:100 | 1.93; 2.01 | 2.66; 2.55 | 0.900 | 0.6960 | 2.46 | 100 | 96 |
| codes:128:40 | 9.07; 10.30 | 11.11; 12.52 | 0.940 | 0.7161 | 35.57 | 40 | 96 |
| rabitq:128:100 | 3.07; 3.18 | 3.85; 4.10 | 0.939 | 0.7161 | 4.81 | 100 | 96 |
| codes:256:40 | 17.37; 19.58 | 20.45; 23.32 | 0.964 | 0.7236 | 69.42 | 40 | 96 |
| rabitq:256:100 | 5.30; 5.54 | 6.71; 6.72 | 0.963 | 0.7236 | 9.38 | 100 | 96 |
| codes:512:40 | 33.10; 37.56 | 38.91; 44.28 | 0.979 | 0.7293 | 135.90 | 40 | 96 |
| rabitq:512:100 | 9.41; 10.01 | 11.56; 12.37 | 0.978 | 0.7293 | 18.36 | 100 | 96 |
| hnsw:16 | 0.21; 0.22 | 0.42; 0.43 | 0.903 | 0.6847 | — | — | 2,229–2,266 |
| hnsw:32 | 0.41; 0.34 | 0.87; 0.71 | 0.949 | 0.7073 | — | — | 2,220–2,266 |
| hnsw:64 | 0.57; 0.60 | 0.88; 0.93 | 0.974 | 0.7211 | — | — | 2,266 |
| hnsw:128 | 1.00; 1.05 | 1.40; 1.70 | 0.987 | 0.7304 | — | — | 2,266 |
| hnsw:256 | 1.91; 2.01 | 2.61; 3.05 | 0.993 | 0.7335 | — | — | 2,266 |
| vec1:8:100 | 0.60; 0.59 | 0.90; 0.81 | 0.735 | 0.6301 | — | — | 80 |
| vec1:16:100 | 0.68; 0.70 | 0.98; 0.97 | 0.809 | 0.6724 | — | — | 80 |
| vec1:32:100 | 0.89; 0.92 | 1.19; 1.27 | 0.863 | 0.6972 | — | — | 80 |
| vec1:64:100 | 1.31; 1.37 | 1.71; 1.88 | 0.903 | 0.7148 | — | — | 80 |
| vec1:128:100 | 2.14; 2.21 | 2.79; 2.92 | 0.928 | 0.7207 | — | — | 80 |

Recall and nDCG were the same in both rounds of every arm. The exact arm's
answers equalled the in-memory scan's, scores bit for bit, in every question
of both rounds; it scored again 25 vectors a question of a million, so
narrow is a byte code's range, and read 4,094 clusters of 4,096, so wide are
the caps: the median cluster's furthest member lies 57° from its centroid.

Between the codes, at 128 clusters and 36,003 members read:

| rescored | RaBitQ recall | p50 ms | signs recall | p50 ms |
|---|---|---|---|---|
| 25 | 0.928 | 2.65; 2.95 | 0.872 | 1.84; 1.91 |
| 50 | 0.938 | 2.78; 2.96 | 0.914 | 1.98; 2.12 |
| 100 | 0.939 | 3.07; 3.18 | 0.931 | 2.21; 2.34 |
| 200 | 0.940 | 4.45; 3.72 | 0.937 | 2.83; 2.87 |
| 400 | 0.940 | 5.51; 4.89 | 0.939 | 3.78; 4.39 |

The clusters probed bound every code's recall, 0.940 at 128, and a byte a
dimension reaches it no better than a bit. RaBitQ's estimate ranks well
enough that 25 scored again come within 0.012 of it, where the signs need
100; its estimate costs more than a Hamming distance, so at equal time the
two tie. Both trail vec1 by 1.2–1.5 times at equal recall. By subtraction,
not by a profile: a vector scored again costs about 5 µs, from the sweeps
above, and RaBitQ's estimate about 60 ns a member, the signs' about 40,
which RaBitQ's own batched four-bit tables are made to cut.

## Size and builds

| store | file MiB | what it holds | build s | build MiB |
|---|---|---|---|---|
| our lists | 5,676 | vectors as given 4,396, codes 994, RaBitQ 139, signs 129, centroids 18 | 334 | 1,629 |
| vec1 | 4,440 | vectors as given 4,396, OPQ codes 35, model 9 | 316 | 953 |
| sqlite-vec | 3,935 | vectors in chunks 3,912, row ids 14, chunks 8 | 220 | 138 |
| HNSW | 2,095 | vectors in float16 and the graph, a file loaded whole | 312 | 2,286 |

Our build: k-means 141 s on 2 CPUs, placing the million and their byte
codes 157, RaBitQ 14, the lists 5. A vector as given takes 4,096 bytes and
a row of its own, 4.5 KiB of pages; what finds it again takes 1 KiB as a
byte code, 146 bytes as RaBitQ's, and 37 bytes as vec1's codes.

## Words, vectors and their fusion

The words' hundred best by `or` over the compact blocks, the vectors' hundred
best, and the ten of their reciprocal rank fusion, 1 / (60 + rank) each,
judged on all 600 questions. The exact vectors come from the in-memory scan,
so their latency is not shown; the words' latency is the second run's, with
warm pages. The main run's first process found the vectors' pages cold: the
words' hundred scored again took 8.01 ms there, 2.3–2.9 in the four after.

| vectors by | mode | nDCG@10 | Recall@10 | MRR@10 | p50 ms | p99 ms |
|---|---|---|---|---|---|---|
| — | words | 0.4318 | 0.5518 | 0.4720 | 1.59 | 9.88 |
| exact | vectors | 0.7360 | 0.8549 | 0.7823 | | |
| exact | fused | 0.6405 | 0.7760 | 0.6776 | | |
| exact | fused, vectors ×2 | 0.6875 | 0.8344 | 0.7240 | | |
| exact | fused, vectors ×4 | 0.7187 | 0.8603 | 0.7579 | | |
| rabitq:128:200 | vectors | 0.7161 | 0.8281 | 0.7703 | 4.16 | 5.07 |
| rabitq:128:200 | fused | 0.6313 | 0.7664 | 0.6694 | 6.39 | 15.96 |
| rabitq:128:200 | fused, vectors ×4 | 0.6996 | 0.8356 | 0.7430 | 6.39 | 15.96 |
| hnsw:128 | vectors | 0.7304 | 0.8493 | 0.7756 | 1.12 | 1.80 |
| hnsw:128 | fused | 0.6390 | 0.7766 | 0.6746 | 2.88 | 11.58 |
| vec1:32:200 | vectors | 0.6975 | 0.7921 | 0.7666 | 1.37 | 2.14 |
| vec1:32:200 | fused | 0.6345 | 0.7748 | 0.6782 | 3.67 | 13.14 |
| none | the words' hundred scored again | 0.6867 | 0.7710 | 0.7535 | 2.21 | 10.56 |
| none | the words' hundred scored again, fused with the words | 0.5798 | 0.6865 | 0.6361 | 2.21 | 10.56 |

The vectors find 1.7 times what the words find by nDCG@10. Fusing them by
rank at equal weight costs the vectors 0.095; weighing the vectors four
times recovers most of it and passes them on Recall@10, 0.860 against
0.855, never on nDCG. Fusion helps where both lists are of a kind; here one
is far the better, and in its own domain. Scoring the words' hundred again
on their vectors needs no vector index at all, a hundred rows read by their
number, and keeps 93 % of the vectors' nDCG; it cannot find a passage that
holds none of the question's words. Every row of all five runs is in
[the data](data/search-vectors-2026-10-16/).

## What this round does not show

- Writes: a set into a cluster's list, a delete, and clusters that drift
  from what they were trained on; the controls' writes.
- A graph kept in SQLite, DiskANN's Vamana through its `DataProvider`.
- RaBitQ's batched estimate, AVX-512, threads a question.
- A model out of its domain, a second model and dimension, questions by
  keyword, a second corpus, a second machine; no significance test.
- The time to embed a question, which a caller pays before any arm.

## What follows

An exact answer costs a full read of a byte a dimension: 245 ms a million,
so about 25 ms a hundred thousand if it stays linear, which would make exact
the default to there and `recall` a choice beyond it. Clusters with a bit a
dimension reach 0.94 in about 3 ms with 96 MiB, 1.2–1.5 times vec1 and 6–9
times HNSW's time at a twentieth of its memory;
the estimate is where our time goes. On this benchmark the vectors carry
the quality and the words add little, and the words' own candidates scored
on their vectors give most of the vectors' gain with no index: a mode a
book could offer first. Writes, a graph in SQLite and a model out of its
domain are the next rounds.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop's WSL2 VM on an AMD
Ryzen 7 7700, the container limited to 2 CPUs and 6 GiB; Rust 1.99.0;
rusqlite 0.40.2 with SQLite 3.53.2, every SQLite file opened as TinyStore
opens one (4 KiB pages, WAL, `synchronous=full`, a 1 MiB cache). The lab's
kernels are compiled for AVX2 and FMA and chosen at run time; AVX-512 is not
used. The words index is the compact one of search-compact-2026-10-16.

Harness `f74cd15` ran [run-vectors.sh](../search-plan-lab/run-vectors.sh)
from 12:09:53 to 12:44:42 UTC; the weighted fusions, added after it, ran
from 12:46:09 to 12:46:22 at `4824759`, into
`hybrid-weighted.jsonl`. `summarize-vectors.py` writes every table here from
the raw lines.

```sh
# in the container, as below: the passages and questions in the indexes' order
/target/release/lab export /src/tinystore/bench/corpus/nomiracl /src/tinystore/bench/corpus/vectors/mixed-1m mixed-1m
# on the host, with a CUDA torch and sentence-transformers
python tinystore/search-plan-lab/embed.py tinystore/bench/corpus/vectors/mixed-1m <model dir>
MSYS_NO_PATHCONV=1 docker run --rm --cpus 2 --memory 6g -v <repo>:/src \
  -v search-plan-cargo:/usr/local/cargo/registry -v search-plan-target:/target \
  -v search-plan-perf:/perf -e CARGO_TARGET_DIR=/target rust:1.99 \
  sh -c 'cd /src/tinystore/search-plan-lab && sh run-vectors.sh'
python tinystore/search-plan-lab/summarize-vectors.py tinystore/reports/data/search-vectors-2026-10-16
```
