# Writes into the vectors' lists — 16 October 2026

[search-vectors-2026-10-16](search-vectors-2026-10-16.md) searched a
million vectors from clusters trained on all of them, written once. This
round writes into them: a set or a delete commits alone, a new member waits
in its cluster's head, a replaced one is marked gone, and three copies of
one index take the same hundred batches of writes, kept up three ways. The
clusters were trained without the 100,000 passages then inserted, all of
them Chinese, so that drift is part of the mix. Nothing here is a product
promise.

| Question | Answer |
|---|---|
| What does a write cost? | 3.1–6.1 ms at the median and 4.8–14.2 at the 99th percentile, a commit each, in every arm |
| Does a search stay fast under writes? | Never packed, RaBitQ over 128 clusters went from 2.9 ms to 14.2 at the median and from 4.6 to 38.4 at the 99th percentile in 130,000 writes; packed after every batch, 4.5 and 6.9 |
| Does it keep finding what it found? | Yes: recall of the exact ten 0.942 at the start and 0.942 after, Chinese questions 0.975 and 0.976, though their passages came after the clusters were trained |
| What does packing cost the writer? | A cluster a commit, 6 ms at the median, 20–25 at the 99th percentile, 249 at most; 3–4 ms of packing a write in all, as much as the write |
| Does splitting help with drift? | It kept the largest cluster at 340 members, where packing alone let one grow from 985 to 3,674; at an equal budget of members read, recall and time came out alike |
| Is the exact answer still exact? | Yes: 0 differ in 60 questions at each of 18 checkpoints, 1,080 in all |

## The mix

An index of the first 696,059 rows of mixed-1m: the judged pool of all three
languages, and MIRACL's Russian and English passages, without its Chinese
ones. It was built as search-vectors built its million: 4,096 clusters from
spherical k-means over 131,072 rows, eight rounds, in 273 s; 155 members at
the median, 985 at most. Then a hundred batches, each of a thousand of the
Chinese passages inserted in order, two hundred documents given the vector
of a random earlier passage, and a hundred deleted, shuffled together by
one seed, every write its own commit.

- **never**: the head is never packed and the gone stay in the lists.
- **clusters**: after every batch, each cluster the batch touched is packed
  in a transaction of its own: its gone members dropped with their vectors,
  its head appended to its lists.
- **split**: as clusters, and a cluster past twice the mean, 339 members,
  is split in two by spherical 2-means over its vectors, both halves' RaBitQ
  codes made again around their new centres, in the same transaction.

Every twentieth batch, after its writes and before its upkeep, the 600
questions are asked against the exact answer over what is live then, which
the first arm computed in memory and the others read back: by RaBitQ over
the 128 clusters nearest a question, R = 50; over the clusters nearest a
question until 30,000 members were read, so that a split changes how many
clusters are probed and not how much is read; and exactly, twenty questions
of each language.

## Searches

| arm | batch | live | head | clusters | largest | 128: p50 ms | p99 ms | recall | Chinese | members read | budget: p50 ms | recall | exact p50 ms |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| never | 0 | 696,059 | 0 | 4,096 | 985 | 2.94 | 4.62 | 0.942 | 0.975 | 25,035 | 3.22 | 0.949 | 191 |
| | 20 | 714,059 | 24,000 | 4,096 | 1,022 | 4.35 | 8.16 | 0.943 | 0.977 | 26,355 | 5.18 | 0.950 | 233 |
| | 40 | 732,059 | 48,000 | 4,096 | 1,431 | 8.03 | 18.08 | 0.943 | 0.977 | 27,615 | 9.13 | 0.948 | 334 |
| | 60 | 750,059 | 72,000 | 4,096 | 2,318 | 10.50 | 25.38 | 0.943 | 0.978 | 28,924 | 10.88 | 0.945 | 379 |
| | 80 | 768,059 | 96,000 | 4,096 | 3,065 | 12.04 | 31.89 | 0.942 | 0.977 | 30,311 | 13.01 | 0.942 | 415 |
| | 100 | 786,059 | 120,000 | 4,096 | 3,751 | 14.24 | 38.42 | 0.942 | 0.976 | 31,614 | 13.80 | 0.940 | 463 |
| clusters | 0 | 696,059 | 0 | 4,096 | 985 | 2.69 | 4.18 | 0.942 | 0.975 | 25,035 | 3.17 | 0.949 | 193 |
| | 20 | 714,059 | 1,200 | 4,096 | 1,013 | 2.97 | 3.80 | 0.943 | 0.977 | 26,152 | 3.64 | 0.950 | 221 |
| | 40 | 732,059 | 1,200 | 4,096 | 1,421 | 3.76 | 4.53 | 0.943 | 0.977 | 27,210 | 4.28 | 0.949 | 254 |
| | 60 | 750,059 | 1,200 | 4,096 | 2,295 | 3.90 | 5.58 | 0.943 | 0.978 | 28,319 | 4.15 | 0.946 | 282 |
| | 80 | 768,059 | 1,200 | 4,096 | 3,017 | 4.14 | 5.34 | 0.942 | 0.977 | 29,512 | 4.47 | 0.944 | 284 |
| | 100 | 786,059 | 1,200 | 4,096 | 3,674 | 4.49 | 6.89 | 0.942 | 0.976 | 30,634 | 4.84 | 0.942 | 296 |
| split | 0 | 696,059 | 0 | 4,096 | 985 | 2.86 | 4.71 | 0.942 | 0.975 | 25,035 | 3.24 | 0.949 | 193 |
| | 20 | 714,059 | 1,200 | 4,475 | 353 | 2.83 | 3.63 | 0.939 | 0.975 | 23,061 | 3.70 | 0.952 | 226 |
| | 40 | 732,059 | 1,200 | 4,550 | 368 | 3.73 | 5.09 | 0.939 | 0.975 | 23,336 | 4.65 | 0.951 | 271 |
| | 60 | 750,059 | 1,200 | 4,614 | 363 | 3.70 | 4.85 | 0.937 | 0.975 | 23,578 | 4.81 | 0.949 | 275 |
| | 80 | 768,059 | 1,200 | 4,684 | 349 | 4.04 | 5.57 | 0.936 | 0.974 | 23,849 | 5.04 | 0.946 | 291 |
| | 100 | 786,059 | 1,200 | 4,751 | 340 | 3.87 | 5.32 | 0.933 | 0.970 | 24,075 | 4.63 | 0.944 | 294 |

A head is a cluster's rows in one table read through an index on the
cluster; never packed, 120,000 of them cost a search four to five times its
time. The exact arm read every head too, 191 ms growing to 463. Recall never
fell: a new vector goes to the cluster nearest it, so a question that would
find it probes that cluster, even when the clusters never saw its language.
What the drift did was make clusters large: the Chinese passages fell into a
few, one of them past 3,600 members, so that the same 128 clusters read a
quarter more members. Splitting held every cluster under twice the mean;
over the same 128 clusters it then read fewer members and found a little
less, 0.933, and at the budget of 30,000 members both kept up arms found
0.942–0.944 in about 4.7 ms.

## Writes and upkeep

Between checkpoints, twenty batches, 26,000 writes:

| arm | batch | insert p50 | p99 | update p50 | p99 | delete p50 | p99 | clusters packed | pack p50 ms | p99 | max | packing s | splits | split p50 ms | max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| never | 20 | 3.71 | 8.04 | 3.72 | 8.42 | 3.14 | 6.60 | | | | | | | | |
| | 100 | 3.55 | 8.48 | 3.56 | 8.81 | 3.09 | 6.32 | | | | | | | | |
| clusters | 20 | 3.81 | 9.68 | 3.81 | 10.05 | 3.31 | 6.76 | 12,892 | 5.9 | 19.1 | 27.7 | 79.5 | | | |
| | 40 | 5.24 | 10.20 | 4.81 | 10.31 | 3.53 | 8.32 | 14,455 | 6.0 | 19.7 | 28.7 | 89.8 | | | |
| | 60 | 5.87 | 10.03 | 5.82 | 11.64 | 5.47 | 7.93 | 14,365 | 6.3 | 21.8 | 108.3 | 99.8 | | | |
| | 80 | 6.05 | 11.77 | 5.97 | 11.98 | 5.64 | 11.03 | 14,330 | 6.3 | 22.7 | 39.4 | 104.7 | | | |
| | 100 | 4.25 | 13.27 | 4.26 | 14.18 | 3.52 | 11.92 | 14,331 | 6.1 | 25.1 | 249.1 | 101.3 | | | |
| split | 20 | 6.09 | 11.74 | 6.03 | 12.45 | 5.61 | 8.77 | 13,156 | 6.3 | 18.9 | 29.7 | 89.4 | 379 | 31.5 | 118.1 |
| | 40 | 3.73 | 9.87 | 3.75 | 8.65 | 3.23 | 7.40 | 14,873 | 3.8 | 18.9 | 31.3 | 81.6 | 75 | 13.1 | 32.5 |
| | 60 | 6.03 | 11.77 | 5.96 | 11.18 | 5.58 | 8.24 | 14,970 | 6.2 | 20.3 | 33.4 | 98.7 | 64 | 14.4 | 30.5 |
| | 80 | 3.71 | 8.49 | 3.72 | 9.95 | 3.18 | 6.65 | 14,982 | 3.7 | 19.7 | 27.7 | 76.9 | 70 | 14.4 | 38.6 |
| | 100 | 3.74 | 9.67 | 3.75 | 8.56 | 3.18 | 6.64 | 15,083 | 3.8 | 20.6 | 29.2 | 80.5 | 67 | 15.4 | 39.1 |

The never-packed arm's writes held at 3.6 ms by the end; [the
measurements](data/search-vchurn-2026-10-16/) hold every checkpoint. Packing
touched about 720 clusters a batch, nearly every cluster a hundred deletes
and two hundred updates reach, and cost about as much as the writes: 3.1–3.9
ms a write in all, no commit of it longer than 249 ms. Writes in the kept-up
arms ran at 3.7 ms in some stretches and 6 ms in others, in no order of
time; nothing here separates why, and a GPU embedding the hybrid round's
corpora ran on the host through all three arms.

## What this round does not show

- Writes and searches at once, from two threads; a writer held by packing
  while a search waits.
- Packing a cluster only once its head or its gone reach a size, which
  would spend less than packing every cluster a batch touched.
- Splits that move a neighbouring cluster's members, as SPFresh's LIRE does,
  and merges of clusters a delete left small.
- A second seed, corpus or machine.

## What follows

RaBitQ's clusters take writes: a write costs a commit, a search stays within
twice its time through 130,000 writes once each touched cluster is packed in
its own commit, recall holds through a language the clusters never saw, and
the exact answer stays exact. Packing everything a batch touched costs as
much again as the writes, so a store would pack a cluster once its head or
its gone grow, and split one past twice the mean.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop's WSL2 VM on an AMD
Ryzen 7 7700, the container limited to 2 CPUs and 6 GiB; Rust 1.99.0;
rusqlite 0.40.2 with SQLite 3.53.2, every SQLite file opened as TinyStore
opens one (4 KiB pages, WAL, `synchronous=full`, a 1 MiB cache). The vectors
are search-vectors-2026-10-16's, checked against
[vectors.sha256](../search-plan-lab/vectors.sha256).

Harness `b764f06` ran [run-vchurn.sh](../search-plan-lab/run-vchurn.sh)
from 13:21:19 to 14:28:09 UTC; `summarize-vchurn.py` writes every table here
from the raw lines.

```sh
MSYS_NO_PATHCONV=1 docker run --rm --cpus 2 --memory 6g -v <repo>:/src \
  -v search-plan-cargo:/usr/local/cargo/registry -v search-plan-target:/target \
  -v search-plan-perf:/perf -e CARGO_TARGET_DIR=/target rust:1.99 \
  sh -c 'cd /src/tinystore/search-plan-lab && sh run-vchurn.sh'
python tinystore/search-plan-lab/summarize-vchurn.py tinystore/reports/data/search-vchurn-2026-10-16
```
