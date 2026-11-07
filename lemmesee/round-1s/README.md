# Round 1s: the key model's numbers on fixed features and one protocol

**Status: specified on 7 November 2026, before any model of this round was
fitted, any language model was asked and any held-out corpus was fetched;
run the same day and [reported](../reports/keys-clean-2026-11-07.md).**
A round of [the plan](../plan.md) that needs no judges. It changes no
design: it fixes a defect in how the key model's features are computed,
fits the same model again, measures it where rounds 1p to 1r measured it,
carries it into Rust, asks the language models again under one protocol,
and prepares a held-out set that nothing so far has read.

## Why

A reader of the draft paper built on rounds 1b to 1r named four things that
would sink it:

1. **The letter features depend on the process.** Round 1b's `keys.py` and
   round 1p's `widen.py` take `list(sg)[:12]` of a column stem's set of
   letter trigrams. Python randomises the hash of a string per process, so
   for a stem with more than twelve trigrams the twelve kept differ from one
   process to the next — and the training features are built by a pool of
   worker processes, each with its own hash seed. Round 1p scored one model
   four times and read external F1 between 0.8910 and 0.8914; round 1r
   scored round 1p's `both` again and read 0.734 over every declared key
   where round 1p had read 0.736. Round 1p's report says a Rust port takes
   the trigrams sorted; no Rust port of the key model exists in this
   repository or in Dashbin's, so a port that sorts would compute features
   the model was not trained on.
2. **The language models ran under different protocols.** Claude Haiku ran
   as six agents reading files, because a Gemini quota ran out; Gemini 3
   Flash's pair prompts stopped at 1 217 of 2 636 unanswered; the cascade
   ran with Haiku instead of the models its specification named. No tokens,
   price or latency were recorded, nor the reasoning setting each model
   identifier implies, nor Haiku's version.
3. **Nothing was timed in Rust**, although small size and offline use are
   the paper's case.
4. **The external databases are development data.** The widening was
   designed after reading the base model's errors on Spider, BIRD and CTU,
   and round 1r's variant after reading `both`'s. A fresh set, evaluated
   once with a frozen model, is needed.

## The model

**Round 1p's `both`**, the widened variant the paper carries: a table's own
one-column key as a source, a pseudo key for a target with no primary key.
Round 1r's `both, unique, decimal` is not carried: its rule took it by a
margin inside round 1p's seed spread, and in the open setting it scores 0.727
against `both`'s 0.734 over every declared key (precision 0.824 against
0.845), which its own report reads as "keep `both` for it". Round 1q's
`both, roles` is a negative result and not carried either. Neither is
refitted here; their figures stay on the features as they were.

**The base configuration**, round 1b's candidates (`widen.py`'s `reported`),
is refitted beside it, because the paper compares the two throughout.

## 1. The trigrams in a fixed order

`letters.py` makes `keys.grams` return a frozenset that iterates in sorted
order, so `list(sg)[:12]` keeps the twelve smallest trigrams by code point,
what `sorted(sg)[:12]` keeps; nothing else changes. Round 1b's and round
1p's files are not edited: every process that builds features — the main
one and each worker of a pool, through the pool's initializer — imports
`letters.py` first. It is checked before any fit:

- the features of validation's pairs, built twice by pools in two processes
  (whose hash seeds differ), are identical, matrix for matrix;
- the share of candidate pairs whose stem has more than twelve trigrams, on
  training, validation and the external databases, is counted: the pairs
  the defect could move.

## 2. The refit

As round 1p fitted `both` (`widen.py`'s `fit`, `seeds.py` for other seeds):
round 1b's `hidden.jsonl` and its split by repository from seed 20261027;
five folds by repository for the first stage's out-of-fold scores; the
first stage XGBoost on the graphics card, 829 trees of at most 63 leaves,
learning rate 0.05; the second LightGBM, 400 trees; a tenth of the negatives
and every positive; the threshold the validation split's best F1 over
candidates, one target a column.

| fit | seed |
|---|---|
| `both`, the fit the paper carries | 20261027, round 1p's chosen fit |
| `both` | 1 |
| `both` | 2 |
| base | 20261027 |

Seeds 1 and 2 are what `seeds.py` set: the negatives kept and both stages'
random states. **The fit at 20261027 replaces round 1p's `both` in the paper
whatever it scores**; the other two say how far a refit moves. Nothing is
chosen by these results.

## 3. The measures

All of them in one run, after the four fits, each model scored by the same
code; intervals are 95 % percentile intervals of 1 000 resamples of
databases (external, seed 20261101, as round 1p) or of repositories
(SchemaPile-Perm, seed 20261027); an interval of a difference resamples
both arms together.

**Round 1p's `both` and round 1b's model as saved** are scored beside the
refits: five times with the trigrams in process order, under hash seeds
0 to 4 each in a process of its own, and once with them sorted — what a
sorted port would compute on a model trained otherwise.

- **Validation**, every fit: P / R / F1 over candidates; over every
  one-column key the validation schemas declare (7 633, as round 1r counts
  them); the paper's task on validation schemas, column and referred column
  as round 1r scores it, and column only as round 1p did.
- **Test, once**, the two fits at 20261027 and the two saved models: over
  candidates against the rules, with the difference's interval; over every
  declared key (13 455); the paper's task on test schemas; the hard cases;
  and for `both` the first stage alone at its own validation threshold, and
  the name score alone at its, over every declared key.
- **Spider, BIRD and CTU** (206, 80 and 142 databases, 398 declaring a key,
  3 371 one-column keys): P / R / F1 over every declared key, by source, all
  and novel, each with its interval and the refit minus round 1p's `both`
  with its interval; over candidates against the rules; the paper's task,
  recall at one with intervals, and why a pair has no candidate; F1 at
  every threshold from 0.05 to 0.95 (round 1q's `threshold.py`); related or
  not (round 1p's `separate.py`).

## 4. The reader in Rust

`reader/`, a crate depending on `serde` and `serde_json` only, reads
`keys.bin` as round 1p's `pack.py` writes it and a schema as a line of
`hidden.jsonl`, and computes what `widen.py`'s `both` computes with the
trigrams sorted: the targets and candidates, the hashed features
(scikit-learn's `FeatureHasher`: MurmurHash3 x86 32-bit, seed 0, the
absolute value modulo 2¹⁵, zero values dropped, duplicates summed), the first
stage over present features, the graph stage's nineteen features as
`graph.py` computes them, the second stage, one target a column, the
threshold.

**Parity**, on every pair of the 428 external databases and of the test
split, the refit at 20261027:

- hashed features: a digest of each pair's indices and f32 values, Rust
  against Python; pairs that differ are counted and shown;
- margins: the largest difference of each stage from XGBoost's and
  LightGBM's;
- keys: those drawn by one and not the other, which should be none; each one
  that differs is shown.

**Time and memory, in the Linux container** (Docker on WSL2, the official
Rust image at a pinned version, a release build, one thread, nothing else
running on the machine):

- loading `keys.bin`;
- for each of the 428 external databases and the test split's schemas, the
  time from a parsed schema to its keys: three runs to warm, then the median
  of 21; reported as median, 90th and 99th percentile and largest, and
  against the schema's candidate pairs; reading the JSON timed apart;
- the peak resident set (`VmHWM`) of a process that loads the model and
  reads one schema, for the median schema and for the largest (CTU's
  `Bench`, 678 tables), each a process of its own; and the binary's size.

## 5. The language models, one protocol

| | identifier | through |
|---|---|---|
| Gemini 3.5 Flash-Lite | `gemini-3.5-flash-lite` | CLIProxyAPI v8.0.28 on the owner's server, Antigravity's account |
| Gemini 3 Flash | `gemini-3-flash` | the same |
| Gemini 3.8 Flash | `gemini-3.8-flash-high` | the same |
| Claude Haiku | `claude-haiku-5-5`, if the owner approves its cost | Anthropic's Messages API |

**The prompts are round 1p's and round 1q's, byte for byte**: the pair
prompt for each of the 2 636 pairs (scored on the 2 416 kept), the
whole-schema prompt for each of the 398 databases, and two sets of cascade
prompts — round 1q's own, cut from `both, roles`, so that its specification
is run with the models it named, and the same cut from the refit `both`
(band [0.2, threshold), round 1q's question text), the model the paper
carries.

**The settings**: one user message, no system prompt, temperature 0, at most
32 768 output tokens, four requests in flight a model. Claude Haiku 5.5
refuses any temperature but its default, so it is asked without one, at
its defaults otherwise (adaptive thinking, effort medium); that is the one
difference, said wherever its numbers are. An empty reply or a failed
request is asked again, up to eight times, longer waits after a refusal for
too many requests; what is unanswered at the end is counted as no answer,
by number.

**Recorded for every request**, beside its reply as it came: the identifier
asked, the model the response names, input, output, reasoning and cached
tokens as the response counts them, the time from sending to the whole body,
the attempts and their errors, the finish reason, the time in UTC. A reply is
read for its last whole JSON object, as round 1p reads it. **Price** is the
tokens at each provider's list price on the day, with its source; through
the proxy the owner pays a subscription, not this. **The reasoning setting**
each identifier implies is read from the proxy's code at v8.0.28 and
checked against the reasoning tokens counted.

**Measured**: over every declared key, P / R / F1 by source, all and novel;
the refit `both` minus each model in F1, precision and recall, each with
its interval; the macro F1 over databases beside the micro F1; the pair
task's recall at one with intervals; each cascade, alone and minus its
first pass; tokens, latency and price a schema — median and total — for the
whole-schema prompts, the cascade's, and the whole-schema prompts of the
cascade's own databases.

## 6. A held-out set, once

From this round on Spider, BIRD, CTU and SchemaPile-Perm's three splits are
development data. **A held-out set is chosen before it is looked at**
(`teaching.md`): the report proposes public corpora with declared foreign
keys that no round has read, with what fetching each takes, and the owner
chooses. Then, in this order:

1. the frozen model is named: the refit `both` at 20261027, its `keys.bin`'s
   SHA-256 and its threshold written into the report;
2. the corpus is fetched into `corpus/round-1s/heldout/`, its files' hashes
   pinned, and read as `external.py` reads Spider's and CTU's, the pairs
   whose two tables occur together in a SchemaPile-Perm schema dropped for
   the pair task as before;
3. the frozen model is evaluated once, in Python and in Rust, over every
   declared key and in the paper's task, beside the rules. Nothing is
   changed after that reading; what it shows is reported as it is.

Language models are asked on it only if the owner asks for them, under §5's
protocol.

## Added while the round ran

Each was written after this specification and before the measure it
concerns was read; none changes a model or a rule above.

- **The owner's choices**, asked on 7 November 2026 after the four fits had
  started: LiveSQLBench-Base-Full-v1 as the held-out set, among three
  candidates; Claude Haiku 5.5 at its defaults through the API, rather than
  Haiku 4.5 at temperature 0 or no Haiku. To describe the candidates, an
  agent fetched their schema files into a scratch folder and counted their
  databases, tables and keys; no model of any round read them, and the set
  was fetched again, pinned, for the evaluation.
- **The frozen model's hashes** were written to `corpus/round-1s/frozen.json`
  at 16:20 UTC, before the held-out set was fetched; `heldout.py score`
  refuses to run if they differ, or twice.
- **The reader's fast path.** The first port, `pairs_named` and
  `graph_features`, mirrors the Python line by line and was checked against
  `widen.py`'s features by name; `fast.rs` computes the same without a
  string a pair, and `reader selfcheck` compares the two on every pair, graph
  feature and key drawn before any time is taken.
- **Fewer runs where one run is long.** A first container run took over 25
  minutes before finishing the external databases: CTU's `Bench` alone holds
  622 512 of their 718 210 pairs, and a version of the reader that marked
  absent features by NaN was four times slower on this processor than one
  that marks present ones by a generation, which the reader keeps. It was
  stopped before any of its numbers was read. The run reported times a
  schema slower than a second (`Bench`) five times after one warm run, every
  other external database as §4 says, and the test split's schemas five times
  after one.
- **The language models on the held-out set**, asked at the owner's request
  after the frozen model's one reading had been reported to the owner:
  §5's protocol, round 1p's prompts cut from the held-out `hidden.jsonl` and
  `bench.jsonl`, Haiku one request at a time; measured by `heldout_lm.py`.
  Nothing about the frozen model changed.
- **Retries.** The proxy refused part of Gemini 3.5 Flash-Lite's and Gemini
  3 Flash's pair prompts with 503s and 429s, and Anthropic's API part of
  Haiku's with 403s; each is asked again in later runs, fewer at once, and
  what stays unanswered is counted.

## What counts as an answer

- The defect is fixed when two processes build identical features and the
  Rust reader's features match Python's pair for pair.
- The refit's numbers replace round 1p's in the paper, whichever way they
  move; the report says whether they move by more than round 1p's model
  moves between scorings and the refits between seeds.
- The language models' numbers replace the earlier ones where the protocol
  is now the same; where it is not (Haiku's temperature), the report says
  so.

## Run

```sh
cd <repo>/lemmesee
P=round-1b/.venv/Scripts/python
# 1-3: the defect, the four fits, every scoring and its summary
$P -I round-1s/refit.py check corpus/round-1b corpus/round-1p corpus/round-1s
$P -I round-1s/refit.py fit corpus/round-1b corpus/round-1s
$P -I round-1s/refit.py measure corpus/round-1b corpus/round-1p corpus/round-1s
# 4: the reader, packed and checked against the libraries, then timed
$P -I round-1s/parity.py pack corpus/round-1s/models/both
(cd round-1s/reader && cargo build --release)
round-1s/reader/target/release/reader selfcheck corpus/round-1s/models/both/keys.bin <threshold> corpus/round-1s/bench/external.jsonl
$P -I round-1s/parity.py check corpus/round-1s/models/both corpus/round-1b corpus/round-1s/external round-1s/reader/target/release/reader corpus/round-1s/parity
MSYS_NO_PATHCONV=1 docker run --rm --cpuset-cpus 3 -v <repo>:/src -v lemmesee-cargo:/usr/local/cargo/registry \
  -w /src/lemmesee rust:1.99.1 sh round-1s/reader/bench.sh corpus/round-1s/models/both <threshold> corpus/round-1s/bench
# 5: the language models
$P -I round-1s/ask.py prompts corpus/round-1p corpus/round-1q/cascade-roles corpus/round-1s
$P -I round-1s/ask.py cascade corpus/round-1s/models/both corpus/round-1p corpus/round-1s
CLIPROXY_KEY=... $P -I round-1s/ask.py run cliproxy <endpoint>/v1 <model> corpus/round-1s <set> 4
ANTHROPIC_API_KEY=... round-1s/.venv/Scripts/python -I round-1s/ask.py run anthropic - claude-haiku-5-5 corpus/round-1s <set> 4
$P -I round-1s/lm.py corpus/round-1s corpus/round-1p corpus/round-1q
# 6: the held-out set, after the frozen model's hashes were written
$P -I round-1s/heldout.py prepare corpus corpus/round-1s/heldout
round-1/harness/target/release/guess corpus/round-1s/heldout/snapshots.jsonl corpus/round-1s/heldout/guesses.jsonl
$P -I round-1s/heldout.py score corpus/round-1s/models/both corpus/round-1s/heldout round-1s/reader/target/release/reader
# then, at the owner's request, the language models on it: round 1p's
# gemini.py cut on corpus/round-1s/heldout, its db-*.txt copied into
# corpus/round-1s/prompts/heldout-db/ and pair-*.txt into heldout-pair/
CLIPROXY_KEY=... $P -I round-1s/ask.py run cliproxy <endpoint>/v1 <model> corpus/round-1s heldout-db 4
ANTHROPIC_API_KEY=... round-1s/.venv/Scripts/python -I round-1s/ask.py run anthropic - claude-haiku-5-5 corpus/round-1s heldout-pair 1
$P -I round-1s/heldout_lm.py corpus/round-1s
$P -I round-1s/collect.py corpus/round-1s reports/data/keys-clean-2026-11-07
```

`<set>` is `db`, `pair`, `cascade-roles` or `cascade-both`; `round-1s/.venv`
holds only the `anthropic` package. The held-out set's files are
LiveSQLBench-Base-Full-v1 at Hugging Face revision `e33469b`, each
`<db>/<db>_schema.txt` fetched from
`https://huggingface.co/datasets/birdsql/livesqlbench-base-full-v1/resolve/<revision>/<path>`
into `corpus/round-1s/heldout/raw/`, checked against the report's
`heldout-schema.sha256`.
