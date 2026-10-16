# The words' writer held in steps — 16 October 2026

[search-compact-2026-10-16](search-compact-2026-10-16.md) kept a search's
tail flat under writes by packing the head every thousand writes, in one
commit that held the writer 0.86 s on average, and let the documents a set
or a delete left gone pile up in the blocks. This round packs the head a
few thousand rows a commit, and takes the gone out of the blocks that hold
them, a few thousand keys a commit, on three copies of the same index under
the same 50,000 operations. Nothing here is a product promise.

| Question | Answer |
|---|---|
| How long does packing hold the writer? | Whole, 0.86–1.0 s a commit; in steps of 2,048 rows, 71–92 ms at most |
| What do steps cost in all? | 3.1 s of commits a thousand writes against 0.92 s whole: 3.4 times the work |
| Can the gone be taken out? | Yes: twice in the run, about 207,000 keys each in 4.8–5.0 s of commits of 2,048 keys, none past 88 ms, the gone from 5,911 to 1,430 and from 5,830 to 1,385 |
| Do searches notice? | Not at these sizes: `or` 2.3–2.9 ms at the median and 14–19 at the 99th percentile in every arm |
| Is every answer still exact? | Yes: 1,404 checks an arm against scoring every posting, 0 differ |

## The mix

The compact index of search-compact, a million documents, three copies.
Each takes 50,000 operations of seed 7: in a hundred, 70 searches of the
600 questions, `or` and `pruned-or` 5 % in turn, 25 sets of a judged
passage with its body changed and 5 deletes, every 25th search checked
against scoring every posting. The window executor, no reading ahead. Every
thousand writes the head is packed:

- **whole**: in one commit, as search-compact packed it.
- **steps**: 2,048 of its rows a commit, in key order, a key never split
  between two, until it is empty; a document's lengths go to its norms in
  the first step that packs any of its keys.
- **purged**: steps, and once the gone are 0.5 % of the documents, a purge.
  A set keeps the text of the document it replaces; the purge reads those
  texts once to know which keys hold them, then takes them out of those
  keys a step of 2,048 keys at a time, writing again only the blocks whose
  range holds one, each block's bound with it, and forgets them, their texts
  and their lengths, in its last step.

## Every 5,000 operations

| arm | operations | `or` p50 | p99 | `pruned-or` p50 | p99 | set p50 | p99 | head rows | gone | commits held | longest ms | purging s | checked | differ |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| whole | 5,000 | 2.33 | 14.82 | 1.69 | 8.36 | 4.03 | 13.02 | 49,006 | 1,501 | 1 | 1,000 | | 139 | 0 |
| | 10,000 | 2.52 | 16.36 | 1.98 | 9.33 | 6.74 | 19.03 | 94,791 | 2,962 | 1 | 974 | | 281 | 0 |
| | 15,000 | 2.46 | 17.04 | 2.02 | 8.28 | 6.78 | 19.82 | 40,994 | 4,425 | 2 | 989 | | 422 | 0 |
| | 20,000 | 2.59 | 16.88 | 2.09 | 9.20 | 4.22 | 15.35 | 91,024 | 5,911 | 1 | 872 | | 562 | 0 |
| | 25,000 | 2.65 | 18.28 | 1.93 | 8.86 | 3.99 | 15.61 | 42,046 | 7,404 | 2 | 958 | | 701 | 0 |
| | 30,000 | 2.61 | 14.12 | 2.02 | 9.41 | 4.25 | 15.35 | 94,964 | 8,898 | 1 | 900 | | 841 | 0 |
| | 35,000 | 2.56 | 15.89 | 2.07 | 8.28 | 4.00 | 14.90 | 43,871 | 10,370 | 2 | 882 | | 981 | 0 |
| | 40,000 | 2.74 | 15.57 | 2.04 | 8.79 | 4.17 | 14.81 | 86,411 | 11,804 | 1 | 861 | | 1,123 | 0 |
| | 45,000 | 2.68 | 15.70 | 2.06 | 8.55 | 3.97 | 14.52 | 39,539 | 13,262 | 2 | 880 | | 1,262 | 0 |
| | 50,000 | 2.86 | 17.60 | 2.08 | 8.61 | 4.17 | 15.17 | 86,798 | 14,686 | 1 | 932 | | 1,404 | 0 |
| steps | 5,000 | 2.40 | 14.98 | 1.71 | 8.48 | 3.99 | 13.64 | 49,006 | 1,501 | 51 | 75 | | 139 | 0 |
| | 10,000 | 2.60 | 16.23 | 2.02 | 10.22 | 4.21 | 14.64 | 94,791 | 2,962 | 49 | 71 | | 281 | 0 |
| | 15,000 | 2.51 | 17.78 | 2.06 | 8.89 | 6.75 | 20.80 | 40,994 | 4,425 | 98 | 91 | | 422 | 0 |
| | 20,000 | 2.56 | 15.89 | 2.08 | 9.05 | 5.90 | 20.21 | 91,024 | 5,911 | 47 | 76 | | 562 | 0 |
| | 25,000 | 2.55 | 18.24 | 1.90 | 8.51 | 4.78 | 17.35 | 42,046 | 7,404 | 96 | 85 | | 701 | 0 |
| | 30,000 | 2.63 | 14.57 | 2.02 | 9.31 | 7.03 | 20.71 | 94,964 | 8,898 | 48 | 84 | | 841 | 0 |
| | 35,000 | 2.58 | 15.97 | 2.09 | 8.46 | 4.06 | 15.25 | 43,871 | 10,370 | 98 | 79 | | 981 | 0 |
| | 40,000 | 2.91 | 15.79 | 2.13 | 9.77 | 6.21 | 21.24 | 86,411 | 11,804 | 48 | 72 | | 1,123 | 0 |
| | 45,000 | 2.73 | 16.12 | 2.09 | 8.62 | 6.66 | 20.87 | 39,539 | 13,262 | 93 | 92 | | 1,262 | 0 |
| | 50,000 | 2.91 | 16.37 | 2.07 | 8.63 | 4.74 | 20.52 | 86,798 | 14,686 | 50 | 89 | | 1,404 | 0 |
| purged | 5,000 | 2.38 | 15.08 | 1.73 | 8.50 | 6.97 | 20.59 | 49,006 | 1,501 | 51 | 75 | | 139 | 0 |
| | 10,000 | 2.61 | 16.64 | 2.00 | 9.93 | 7.19 | 22.42 | 94,791 | 2,962 | 49 | 80 | | 281 | 0 |
| | 15,000 | 2.51 | 17.41 | 2.05 | 8.72 | 7.02 | 24.52 | 40,994 | 4,425 | 98 | 83 | | 422 | 0 |
| | 20,000 | 2.61 | 15.95 | 2.08 | 9.19 | 6.98 | 22.24 | 91,024 | 5,911 | 47 | 85 | | 562 | 0 |
| | 25,000 | 2.68 | 19.10 | 2.03 | 9.83 | 4.56 | 17.91 | 42,046 | 1,430 | 198 | 82 | 5.0 | 701 | 0 |
| | 30,000 | 2.79 | 16.61 | 2.18 | 9.78 | 4.92 | 21.14 | 94,964 | 2,924 | 48 | 76 | | 841 | 0 |
| | 35,000 | 2.71 | 17.65 | 2.16 | 9.12 | 6.79 | 24.04 | 43,871 | 4,396 | 98 | 88 | | 981 | 0 |
| | 40,000 | 2.79 | 15.28 | 2.07 | 8.96 | 7.14 | 21.63 | 86,411 | 5,830 | 48 | 80 | | 1,123 | 0 |
| | 45,000 | 2.62 | 15.42 | 2.02 | 8.49 | 4.11 | 16.06 | 39,539 | 1,385 | 194 | 83 | 4.8 | 1,262 | 0 |
| | 50,000 | 2.79 | 16.34 | 2.05 | 8.66 | 6.93 | 19.81 | 86,798 | 2,809 | 50 | 75 | | 1,404 | 0 |

A row is the 5,000 operations before it; head rows and gone are counted at
its end. The purges began at operations 20,217 and 40,219: reading the gone
texts took 566 and 501 ms, outside any commit, then 208,153 and 206,229
keys in steps. Readers peaked at 121–123 MiB, and 156 in the purged arm,
which held a purge's keys in memory.

## The cost

A whole pack wrote about 52,000 keys' postings in one commit, 917 ms on
average. Steps wrote the same in 47–51 commits, 3.1 s in all: each step reads
its rows in one statement and deletes them by a range of keys, but every
commit syncs and every step reads the rows of norms its documents fall in,
512 KiB each. Which of these costs most is not measured; the first version
of the steps read and deleted a key at a time and cost 3 s for 300 writes.

Sets ran at 4 ms at the median in some stretches and 7 in others, in every
arm and in no order of time, as the vectors' writes did: an embedding of the
next round's corpora ran on the host's GPU throughout, with a core of
tokenizing beside it.

## What this round does not show

- What purging saves: the file's size before and after, and the searches'
  time once the gone are far more than the 1.5 % this mix made.
- A step that commits less often, or packs norms apart from postings.
- A search waiting on a step: the lab writes and searches on one thread.
- A second seed, corpus or machine.

## What follows

Steps take the longest hold of the writer from a second to 92 ms, and a
purge takes the gone out in commits as short, with every answer exact; both
cost more work in all than one commit does, and steps 3.4 times as
much, which a store would trade down with larger, rarer steps.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop's WSL2 VM on an AMD
Ryzen 7 7700, the container limited to 2 CPUs and 6 GiB; Rust 1.99.0;
rusqlite 0.40.2 with SQLite 3.53.2, every SQLite file opened as TinyStore
opens one (4 KiB pages, WAL, `synchronous=full`, a 1 MiB cache). The compact
index of search-compact-2026-10-16, the corpus checked against
[corpus.sha256](../search-plan-lab/corpus.sha256).

Harness `0d0616b` ran [run-wchurn.sh](../search-plan-lab/run-wchurn.sh)
from 14:37:49 to 14:51:34 UTC; `summarize-wchurn.py` writes every table here
from [the raw lines](data/search-wchurn-2026-10-16/).

```sh
MSYS_NO_PATHCONV=1 docker run --rm --cpus 2 --memory 6g -v <repo>:/src \
  -v search-plan-cargo:/usr/local/cargo/registry -v search-plan-target:/target \
  -v search-plan-perf:/perf -e CARGO_TARGET_DIR=/target rust:1.99 \
  sh -c 'cd /src/tinystore/search-plan-lab && sh run-wchurn.sh'
python tinystore/search-plan-lab/summarize-wchurn.py tinystore/reports/data/search-wchurn-2026-10-16
```
