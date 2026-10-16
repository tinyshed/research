# Words and vectors out of the model's domain — 16 October 2026

[search-vectors-2026-10-16](search-vectors-2026-10-16.md) fused the words
with bge-m3's vectors on MIRACL, whose training questions the model learned
from, and found the vectors so far ahead that every fusion lost to them
alone. This round asks the same of two collections outside its fine-tuning:
Amazon's products in three languages and twelve StackExchange forums. It
judges the words, the vectors exact and from clusters, fusion by rank at
three weights, a convex sum of scores at three, and the words' hundred best
scored again on their vectors, on 600 questions each, split by whether a
question names a thing by a code. Nothing here is a product promise.

| Question | Answer |
|---|---|
| Do the vectors still lead? | On the products, no: the words find 0.306 by nDCG@10 and the vectors 0.307. On StackExchange, yes: 0.240 and 0.378, 1.6 times, near their 1.7 on MIRACL |
| Does fusion beat both? | On the products every fusion of the exact lists does, 0.322–0.361; by rank at equal weight it wins 301 questions over the words and loses 131. On StackExchange only a convex sum leaning to the vectors passes them alone, 0.391 against 0.378; fusion by rank at equal weight loses to them, 0.348 |
| By rank or by score? | By score: a convex sum of each list's scores, scaled to its own range, at equal weight beats fusion by rank in all four pairs, by 0.002–0.020, outside the 95 % interval in three |
| At what weight? | The collection's: of the three, the vectors' share at 0.5 is best on the products and 0.025 behind on StackExchange; at 0.7 it is best there and 0.013 behind on the products; 0.9 trails on both |
| And from clusters? | RaBitQ's clusters find 0.90 of the exact ten in both, and every fusion built on them loses 0.014–0.039. The words' hundred scored again on the vectors as given loses nothing: the best mode with clusters on the products, 0.361 against the convex sum's 0.347, and 0.007 behind the best on StackExchange |
| Do codes need the words? | Not here: the 74 and 59 questions that name a model, a version or a call by a code are answered better by the vectors than by the words in both, 0.412 against 0.329 and 0.460 against 0.291 |

## The collections

**ESCI**, Amazon's Shopping Queries Dataset at `7916cdf`, Apache-2.0: every
product its small version judges in the US, Spanish and Japanese stores,
883,716 (482,105, 167,761 and 233,850), its title as the title and its
brand, colour, bullet points and description, markup taken out, as the text.
200 questions a store from the test split, a product relevant when labelled
Exact. Its labels also grade, Exact 3, Substitute 2, Complement 1 and
Irrelevant 0, and nDCG@10 is given both ways. A question holds 3.1 words on
average and 11.1 relevant products.

**CQADupStack**, twelve StackExchange forums as MTEB keeps them, each at the
revision [fetch-hybrid.sh](../search-plan-lab/fetch-hybrid.sh) pins: 457,199
posts, a title and a body each, from 16,705 in mathematica to 68,184 in tex.
50 test questions a forum, a question a post's title, relevant the posts
marked its duplicates on the forum: 1.9 on average. A question holds 8.7
words on average.

In each store and forum the questions are the first by the SHA-256 of their
ids among those with a relevant document and every judged one in the
collection. A question is a *code* when it holds a token of letters and
digits, an identifier written as code (`snake_case`, `camelCase`,
`dotted.name`, `call()`, `--flag`) or a constant (`ERR_CONNECTION_RESET`),
by the expression in [summarize-hybrid.py](../search-plan-lab/summarize-hybrid.py):
74 of ESCI's 600, such as `ddj400`, `samsung galaxy s10e` and `dolby vision
4k projector`, and 59 of CQADupStack's, such as `Where I can download QGIS
1.8 for Mountain Lion?` and `Couldn't load plugin fTools due an error when
calling its classFactory() method`.

Neither collection is in bge-m3's fine-tuning data as its paper lists it;
StackExchange's titles and bodies are a common source of unsupervised pairs,
and whether bge-m3's held them was not checked.

## The vectors

BAAI/bge-m3's dense vectors, revision `5617a9f`, as search-vectors made
them: 1,024 dimensions of length one, float16 on an RTX 5070 through
sentence-transformers 6.1.0 and torch 2.11.0+cu128, a document's title and
text joined by a line break and cut at 512 tokens. ESCI's products took
223,530,502 tokens and 3,923 s; CQADupStack's posts 97,774,707 tokens and
1,795 s. [embed.py](../search-plan-lab/embed.py) wrote them in the order
the indexes number the documents; `embed-<cohort>.json` beside the data
holds every part's time.

Each cohort's vectors are listed as search-vectors listed a million: 4,096
clusters for ESCI and 2,048 for CQADupStack, trained on 131,072 vectors in
eight rounds, a list a cluster coded by RaBitQ's bit a dimension. The
clusters' arm reads the 128 clusters nearest a question for ESCI and 64 for
CQADupStack, 3 % of them, and scores again on their vectors as given the
200 its estimate ranks first. The exact arm is the true hundred nearest,
computed beforehand and read from a file, so it has no time of its own here.

## The words, the vectors and their fusion

Every mode starts from two lists of a hundred:

- `words`: the hundred best by BM25 over `or` of the question's words, on
  the compact blocks.
- `vectors`: the hundred nearest by cosine.
- `hybrid`: the two fused by rank, 1 / (60 + rank) from each list; `vectors
  ×2` and `×4` weigh the vectors' term twice and four times.
- `convex, vectors w`: each list's scores scaled to [0, 1] by its own lowest
  and highest, a document missing from a list 0 there, then
  (1 − w) · words + w · vectors.
- `words rescored`: the words' hundred ordered by the cosine of their
  vectors as given with the question's; `fused` fuses that order with the
  words' by rank. It reads a hundred rows by their number and needs no
  vector index, so it is the same in both arms.

nDCG@10 on each cohort's 600 questions, an ESCI product relevant when Exact:

| mode | esci exact | esci clusters | cqa exact | cqa clusters |
|---|---|---|---|---|
| words | 0.3063 | 0.3063 | 0.2396 | 0.2396 |
| vectors | 0.3067 | 0.2798 | 0.3777 | 0.3503 |
| hybrid | 0.3589 | 0.3324 | 0.3478 | 0.3251 |
| hybrid, vectors ×2 | 0.3613 | 0.3223 | 0.3679 | 0.3391 |
| hybrid, vectors ×4 | 0.3550 | 0.3180 | 0.3762 | 0.3470 |
| convex, vectors 0.5 | 0.3611 | 0.3467 | 0.3655 | 0.3454 |
| convex, vectors 0.7 | 0.3479 | 0.3185 | 0.3906 | 0.3602 |
| convex, vectors 0.9 | 0.3222 | 0.2928 | 0.3835 | 0.3550 |
| words rescored, fused | 0.3507 | 0.3507 | 0.3110 | 0.3110 |
| words rescored | 0.3610 | 0.3610 | 0.3528 | 0.3528 |

Out of its domain the model's lead is the collection's. On ESCI's products
the vectors find what the words find, and any fusion finds more than either:
by rank at equal weight it wins 301 questions over the words and loses 131.
On CQADupStack the vectors find 1.6 times what the words find, and a fusion
by rank at equal weight loses to them alone, as every fusion by rank did on
MIRACL; only the convex sums at 0.7 and 0.9 pass them. ESCI's graded
nDCG, which counts a Substitute and a Complement too, orders the modes as
the binary one does (below).

## By rank or by score

Each question's nDCG@10 under one mode less its own under another; the
interval is 95 % of 2,000 means over the questions drawn again with
replacement, and wins and losses count the questions that moved.

ESCI:

| this | less that | nDCG@10 | 95 % | wins | losses |
|---|---|---|---|---|---|
| truth hybrid | truth words | +0.0527 | +0.0396 … +0.0656 | 301 | 131 |
| truth convex, vectors 0.5 | truth hybrid | +0.0022 | -0.0057 … +0.0097 | 178 | 193 |
| truth words rescored | truth hybrid | +0.0020 | -0.0062 … +0.0116 | 195 | 214 |
| truth words rescored | truth convex, vectors 0.5 | -0.0002 | -0.0108 … +0.0122 | 206 | 215 |
| rabitq-128-200 vectors | truth vectors | -0.0270 | -0.0364 … -0.0178 | 54 | 69 |
| rabitq-128-200 hybrid | rabitq-128-200 words | +0.0262 | +0.0109 … +0.0404 | 263 | 169 |
| rabitq-128-200 convex, vectors 0.5 | rabitq-128-200 hybrid | +0.0142 | +0.0054 … +0.0228 | 206 | 167 |
| rabitq-128-200 words rescored | rabitq-128-200 convex, vectors 0.5 | +0.0143 | +0.0026 … +0.0272 | 227 | 207 |

CQADupStack:

| this | less that | nDCG@10 | 95 % | wins | losses |
|---|---|---|---|---|---|
| truth hybrid | truth words | +0.1082 | +0.0910 … +0.1266 | 192 | 20 |
| truth convex, vectors 0.5 | truth hybrid | +0.0177 | +0.0070 … +0.0283 | 104 | 60 |
| truth words rescored | truth hybrid | +0.0050 | -0.0087 … +0.0186 | 93 | 83 |
| truth words rescored | truth convex, vectors 0.5 | -0.0127 | -0.0286 … +0.0031 | 94 | 109 |
| rabitq-64-200 vectors | truth vectors | -0.0274 | -0.0405 … -0.0155 | 25 | 32 |
| rabitq-64-200 hybrid | rabitq-64-200 words | +0.0854 | +0.0672 … +0.1046 | 182 | 37 |
| rabitq-64-200 convex, vectors 0.5 | rabitq-64-200 hybrid | +0.0204 | +0.0102 … +0.0310 | 109 | 54 |
| rabitq-64-200 words rescored | rabitq-64-200 convex, vectors 0.5 | +0.0074 | -0.0107 … +0.0256 | 109 | 102 |

A convex sum at equal weight beats fusion by rank in all four pairs, three
of them outside the interval: a score says how far apart two documents are,
which a rank throws away. No weight wins both collections. The vectors'
share at 0.5 is ESCI's best and 0.025 behind CQADupStack's at 0.7; 0.7 is
0.013 behind ESCI's 0.5; 0.9 trails on both. The weight that wins follows
how far the vectors lead the words: an equal share where they tie, more
where they lead, and on MIRACL, where they lead most, no fusion by rank
passed them alone.

The words' hundred scored again ties every fusion of the exact lists on
ESCI and trails the convex sum at 0.5 by 0.013 on CQADupStack, inside the
interval. It cannot find a document that holds none of the question's
words, which costs it where the vectors lead.

## Clusters instead of exact vectors

RaBitQ's clusters find 0.9023 of the exact ten on ESCI and 0.9025 on
CQADupStack, where search-vectors' million passages found 0.939 reading the
same 3 % of their clusters; why these collections cluster worse is not
measured. The vectors alone lose 0.027 in both, and every fusion built on
them 0.014 to 0.039, the convex sum at 0.5 least. The words' hundred scored again reads
those hundred vectors as given and never the clusters, so it loses nothing:
with clusters it is ESCI's best mode, 0.014 ahead of the convex sum at 0.5
and outside the interval, and 0.007 behind CQADupStack's best, the convex
sum at 0.7.

## Times

Each mode's time a question in the clusters' arm, warm pages, 2 CPUs, from
the every-mode table `summarize-hybrid.py` prints; a fusion's time is the
words' and the vectors' one after the other, the same for every fusion.

| cohort | mode | p50 ms | p99 ms |
|---|---|---|---|
| esci | words | 2.24 | 17.82 |
| esci | vectors, 128 clusters | 5.05 | 8.09 |
| esci | any fusion | 7.44 | 23.27 |
| esci | words rescored | 2.88 | 18.43 |
| cqa | words | 11.78 | 61.32 |
| cqa | vectors, 64 clusters | 2.89 | 4.54 |
| cqa | any fusion | 14.79 | 65.62 |
| cqa | words rescored | 12.42 | 62.01 |

Scoring the words' hundred again adds 0.6–0.8 ms at the median, where
fusing with the clusters' hundred adds their search, 3–5 ms. ESCI's times
were taken while the host's GPU embedded CQADupStack beside them, so they
may stand high. The words take most of a search by both on CQADupStack,
11.8 ms at the median and 61 at the 99th percentile against 1.6 and 9.9 on
search-vectors' million passages; its questions hold 8.7 words against
ESCI's 3.1, and what costs most there is not measured.

## Codes and words

nDCG@10 of the exact arm by kind of question; ESCI's also graded and by
store.

ESCI, 74 questions with a code and 526 without:

| vectors by | mode | nDCG@10 | code | words | graded | us | es | jp |
|---|---|---|---|---|---|---|---|---|
| truth | words | 0.3063 | 0.3294 | 0.3030 | 0.3402 | 0.2927 | 0.2953 | 0.3307 |
| truth | vectors | 0.3067 | 0.4122 | 0.2919 | 0.3286 | 0.2944 | 0.3174 | 0.3084 |
| truth | hybrid | 0.3589 | 0.4051 | 0.3524 | 0.3947 | 0.3465 | 0.3608 | 0.3696 |
| truth | hybrid, vectors ×2 | 0.3613 | 0.4412 | 0.3500 | 0.3944 | 0.3494 | 0.3647 | 0.3697 |
| truth | hybrid, vectors ×4 | 0.3550 | 0.4291 | 0.3445 | 0.3852 | 0.3372 | 0.3663 | 0.3614 |
| truth | convex, vectors 0.5 | 0.3611 | 0.4357 | 0.3506 | 0.3922 | 0.3510 | 0.3671 | 0.3653 |
| truth | convex, vectors 0.7 | 0.3479 | 0.4343 | 0.3357 | 0.3760 | 0.3318 | 0.3592 | 0.3527 |
| truth | convex, vectors 0.9 | 0.3222 | 0.4143 | 0.3093 | 0.3454 | 0.3095 | 0.3360 | 0.3213 |
| truth | words rescored, fused | 0.3507 | 0.3858 | 0.3457 | 0.3870 | 0.3442 | 0.3423 | 0.3654 |
| truth | words rescored | 0.3610 | 0.4362 | 0.3504 | 0.3916 | 0.3418 | 0.3632 | 0.3779 |

CQADupStack, 59 with a code and 541 without:

| vectors by | mode | nDCG@10 | code | words |
|---|---|---|---|---|
| truth | words | 0.2396 | 0.2912 | 0.2340 |
| truth | vectors | 0.3777 | 0.4603 | 0.3687 |
| truth | hybrid | 0.3478 | 0.4395 | 0.3378 |
| truth | hybrid, vectors ×2 | 0.3679 | 0.4381 | 0.3603 |
| truth | hybrid, vectors ×4 | 0.3762 | 0.4616 | 0.3669 |
| truth | convex, vectors 0.5 | 0.3655 | 0.4628 | 0.3549 |
| truth | convex, vectors 0.7 | 0.3906 | 0.4801 | 0.3809 |
| truth | convex, vectors 0.9 | 0.3835 | 0.4740 | 0.3737 |
| truth | words rescored, fused | 0.3110 | 0.3794 | 0.3036 |
| truth | words rescored | 0.3528 | 0.4332 | 0.3440 |

The words were expected to matter most where a question names a thing by a
code; here the vectors answer those questions better than the words in both
collections, by more than they answer the rest, and on ESCI's codes the
vectors alone pass fusion by rank at equal weight. The groups are small, the expression counts a size such
as `4k` or `14k` as a code, and a question that is an identifier alone, a
part number or an error code, was not sampled apart: the case where the
words should matter most is not tested here.

## Builds

| cohort | index | documents | MiB | of which, MiB | s | HWM MiB |
|---|---|---|---|---|---|---|
| esci | words | 883,716 | 1,756 | docs 1,287, cblob 355, ckeys 82, sqlite_autoindex_docs_1 25, norms 7 | 156 | 3,612 |
| esci | vectors, 4,096 clusters | 883,716 | 5,019 | vf32 3,885, lcodes 879, lrabitq 123, lsigns 115, centroids 18 | 312 | 1,458 |
| cqa | words | 457,199 | 683 | docs 494, cblob 119, ckeys 53, sqlite_autoindex_docs_1 12, norms 3 | 53 | 1,447 |
| cqa | vectors, 2,048 clusters | 457,199 | 2,596 | vf32 2,010, lcodes 454, lrabitq 63, lsigns 59, centroids 9 | 119 | 834 |

The words' file keeps each document's text in `docs`; its postings are
`cblob` and `ckeys`. The vectors' file keeps every vector as given in
`vf32`, 4.5 KiB each, beside the lists' byte codes, RaBitQ codes and signs.

## What this round does not show

- A weight between 0.5 and 0.7, or one chosen per collection from a few
  judged questions; the lists' scores were not kept, so no other weight can
  be computed from this run.
- The words' candidates and the vectors' joined into one list, each scored
  by both: the words' BM25 and every candidate's exact cosine, then a convex
  sum. The round fused two separate hundreds and scored again only the
  words'.
- Questions that are an identifier alone, a part number or an error code.
- Why the words take 11.8 ms at the median on CQADupStack.
- A second model or dimension, a second seed of the questions, a second
  machine; ESCI's times beside a busy GPU.

## What follows

Fusion by rank is out: a convex sum of each list's scores, scaled to its own
range, beats it in every pair. The weight belongs to the collection and its
model, 0.5 for the products and 0.7 for StackExchange here, so it is the
program's to set when it opens an index, with 0.5 when it says nothing:
best where words and vectors tie and 0.025 behind where the vectors lead.
The words' hundred scored again on their vectors needs no vector index,
costs under a millisecond on top of the words and was never more than 0.04
behind the best fusion in either collection, which makes it a mode for an
index whose vectors are exact and many; with clusters it loses nothing of
what they miss.

## Environment and reproduction

Linux 6.18.33.2-microsoft-standard-WSL2 in Docker Desktop's WSL2 VM on an AMD
Ryzen 7 7700, the container limited to 2 CPUs and 6 GiB; Rust 1.99.0;
rusqlite 0.40.2 with SQLite 3.53.2, every SQLite file opened as TinyStore
opens one (4 KiB pages, WAL, `synchronous=full`, a 1 MiB cache). The words
are the compact blocks of search-compact-2026-10-16 with the window executor
and eight blocks read ahead; the lab's kernels are compiled for AVX2 and FMA.
The collections are checked against
[hybrid.sha256](../search-plan-lab/hybrid.sha256); the vectors were made on
the host and copied into the container from an external disk. ESCI ran
while CQADupStack's posts were embedded on the host's GPU, a core of
tokenizing beside it; CQADupStack ran with the host quiet.

Harness `933af65` ran ESCI with
[run-hybrid.sh](../search-plan-lab/run-hybrid.sh) from 15:02 to 15:12 UTC,
and `f66db32`, which names each line's cohort, ran CQADupStack after it
from 15:17:41 to 15:22:03, with `APPEND=1 SPECS=cqa:2048:rabitq:64:200`;
`summarize-hybrid.py` writes every table here from
[the raw lines](data/search-hybrid-2026-10-16/), each question's ten by
every mode among them, and from the questions and their judgments, which
are the corpus's and stay beside it. Both runs copied those into the data
and the raw lines unpacked; they were moved out and the lines packed after,
as run-hybrid.sh now does, and the tables came out the same byte for byte.

```sh
sh tinystore/search-plan-lab/fetch-hybrid.sh <fetched>
python tinystore/search-plan-lab/hybrid-corpus.py <fetched> <given>/hybrid/lab
# in the container: each cohort's documents and questions in the indexes' order
/target/release/lab export /hybrid/hybrid/lab /hybrid/vectors/esci esci
/target/release/lab export /hybrid/hybrid/lab /hybrid/vectors/cqa cqa
# on the host, with a CUDA torch and sentence-transformers
python tinystore/search-plan-lab/embed.py <given>/vectors/esci <model dir>
python tinystore/search-plan-lab/embed.py <given>/vectors/cqa <model dir>
MSYS_NO_PATHCONV=1 docker run --rm --cpus 2 --memory 6g -v <repo>:/src -v <given>:/hybrid \
  -v search-plan-cargo:/usr/local/cargo/registry -v search-plan-target:/target \
  -v search-plan-perf:/perf -e CARGO_TARGET_DIR=/target rust:1.99 \
  sh -c 'cd /src/tinystore/search-plan-lab && sh run-hybrid.sh'
python tinystore/search-plan-lab/summarize-hybrid.py tinystore/reports/data/search-hybrid-2026-10-16 <given>/judged
```
