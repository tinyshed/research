# Round 1z: what stands between a megabyte and a remote language model

Written at 15:05 UTC on 26 November 2026, before any network of this round
was trained and before any local language model was asked anything. By then
the round had only fetched: the runtime's release, three models' weights
(still downloading) and the Python packages of its own environment.

The key model's paper argues for "a model of about a megabyte against
everything larger", and so far it shows only gradient-boosted trees
(`eleven--wide`, lemmekey in the paper: 1 054 108 bytes, 3 ms a median
database) against large remote language models
([keys-paper-model-2026-11-14.md](../reports/keys-paper-model-2026-11-14.md)).
A reviewer will ask two things, and this round measures both:

1. **Would a small neural model on the same inputs do better than trees**, at
   a comparable or modestly larger size?
2. **Would a local open-weights language model, run on the user's own
   machine, close the gap** with no remote call?

Both are measured on the paper's two clean sets, plain A (718 GitHub
schemas, 16 399 keys) and style 3 (100 schemas, 1 762 keys), then on round
1p's 398 external databases and LiveSQLBench as time allows, with size, time
and memory beside F1.

## What is development data, and what a reading here is

No tree model is fitted, chosen or changed. `eleven--wide` stays as round 1v
froze it; its figures on plain A and style 3 are round 1v's one reading,
taken from the counts a database that reading kept
(`corpus/round-1v/heldout-<set>.json`), as round 1w's `lms.py` takes them.

Plain A and style 3 are development data for our tree models since round
1v read them. For the models of this round they are not: no network here
sees a schema of either set before it is frozen, and the local language
models are downloaded as published. Each network is **frozen by hash before
either set's candidates are built**, and each set is **read once by each
frozen model** (`clean.py read` refuses a second reading). A baseline read
once after freezing is a fair reading of these sets; it is not a clean
comparison in the other direction, since lemmekey's own design was not
changed by them either but everything after round 1v has looked at them.
The report says exactly this beside the figures. Plain B stays unread.

The 398 external databases and LiveSQLBench are development data for the
trees and public benchmarks; a network of this round reads them after it is
frozen, once.

## Part A: a small neural model on the same inputs

**The same training data.** The rows `eleven`'s first stage was fitted on,
read from what round 1v kept (`corpus/round-1v/first/eleven/train.pkl`): the
8 181 training schemas of SchemaPile-Perm's split by repository, 3 999 of
them renamed into a drawn style with the dictionary's shortenings and a key
word first; the same candidates (8 603 420 pairs); and of them the same
sample, every positive pair and the same tenth of the negatives (920 601
rows, 67 136 positive, the mask `sample` of that file). No pair is built
again for training. The validation split is that fit's
(`validation.pkl`: 2 060 schemas, 1 022 of them renamed copies beside their
originals, 1 674 439 pairs).

**The same decision.** A column keeps its best-scored table
(`whole.one_target`), and the key is drawn when the score passes a
threshold chosen for F1 on the validation split's pairs
(`keys.best_threshold`, the function that chose lemmekey's). The same open
setting, the same candidates on every set that is read, the same measures.

**What differs**, all of it stated before training:

- *The input is the pair as written*, not lemmekey's features: the column's
  name, type family and type, whether it is nullable, unique, part of the
  primary key, indexed, and its position; its table's name; the target
  table's name; the target's key column, its type and whether the key is
  the table's whole primary key; whether the two tables are one; and the
  number of tables of the schema. No hashed feature, no cue of versions 6
  to 11, no lexicon, no word list: the question is whether a network learns
  from the names what the cues were written by hand to say.
- *One stage.* **No second stage over the schema is added** to any network:
  a second stage needs every training pair's score out of fold, five more
  fits of each network, and the reviewer's question is about the pair
  scorer. So that like stands beside like, lemmekey's **first stage alone**
  (`eleven`, 829 trees, the same decision, its threshold read on the same
  validation scores) is reported beside the networks on every set; on the
  clean sets that figure is a development figure, as every tree figure
  there is. If time is left after everything else, the compact network's
  scores are also given to round 1v's `wide` second stage, fitted as round
  1v fitted it from scores out of fold; it is reported apart and marked as
  added.
- *No folds*: one fit of each network on the whole sample; the validation
  split chooses the epoch and the threshold.

**The models**, each a design a reviewer would call the obvious one:

| name | what it is | size |
|---|---|---|
| `char-s` | a character-level transformer read across the pair: the bytes of `column`, `type`, `table`, `target`, `key`, `key type` in one sequence of at most 128 with a segment embedding, 4 layers of 128 dimensions, 4 heads, feed-forward 256; the first position and a vector of ten flags and counts go through one hidden layer of 128 to a logit | about 0.6 million parameters, 2.4 MB as 32-bit floats |
| `char-m` | the same, 6 layers of 256 dimensions, 8 heads, feed-forward 512 | about 3.3 million parameters, 13 MB |
| `granite` | `ibm-granite/granite-embedding-small-english-r2`, revision `2ab6fa8ea2d674564defd37171ae19079b864b33`, Apache-2.0, 47.7 million parameters (a ModernBERT encoder of January 2026, IBM's), fine-tuned whole as a pair scorer: the column's side and the target's side as two segments of text (`table: … column: … type: …` and `table: … key: … type: …`), the first position's vector through a linear layer to a logit; the flags and counts are written into the text | about 190 MB as 32-bit floats, 95 MB as 16-bit |

Training: binary cross-entropy on the sampled rows, AdamW, a cosine schedule
after a warm-up of 5 % of the steps, seed 20261027 (round 1v's). `char-s`
and `char-m`: 10 epochs, batch 1 024, learning rate 1e-3, weight decay
0.01, dropout 0.1. `granite`: 2 epochs, batch 128, learning rate 5e-5,
weight decay 0.01, at most 96 tokens, mixed precision. After each epoch the
validation split is scored whole, and the epoch with the best F1 at its best
threshold is the model. Nothing else is tuned: a setting that fails to
train (a loss that does not fall) may be changed once, and the README says
so below.

**Reported for each**: parameters; bytes on disk as shipped (the weights as
32-bit floats in `safetensors`, with the tokenizer's files where there is
one) and with its linear layers quantised to 8 bits by PyTorch's dynamic
quantisation, whose F1 is read on the validation split only (a quantised
network is another model; it is not frozen and reads no clean set); the
time a database takes on one processor (below).

**Frozen**: `corpus/round-1z/frozen.json` holds each network's SHA-256 (its
weights, its configuration and its threshold) and the time, written before
`clean.py` builds a clean set's candidates.

**Time on one processor**: the network scoring a database's candidate
pairs, tokenising included, building the candidates not included (lemmekey's
3 ms includes it, so the comparison flatters the networks), PyTorch on one
thread, over round 1p's 398 external databases after one warm database. A
database of more than 20 000 pairs is timed on its first 20 000 and scaled;
that is under half of the databases, so the median is a timed one. In the
Linux container on one processor (`--cpuset-cpus 3`) if PyTorch installs
there; otherwise on Windows with one thread, and the report says which.

## Part B: local language models

**The runtime**: Ollama v0.40.18, the release's `ollama-windows-amd64.zip`
from `github.com/ollama/ollama` (SHA-256
`ca97292ac1e9acc77fcf473bb2741fa6acd12e3798f261b419680068b7cc4c16`, as the
release lists it), unpacked under `corpus/round-1z/runtime/` and run from
there on port 11435; nothing is installed into the system. Weights come
from Ollama's own registry, into `corpus/round-1z/ollama-models/`.

**The models**, chosen on 26 November 2026 from the registry's library for
being current general models of their makers at three sizes that the
machine's graphics card (12 GB) holds:

| tag | maker, licence | quantisation | file, as the registry lists it |
|---|---|---|---|
| `qwen3.5:2b-q4_K_M` | Alibaba's Qwen 3.5, Apache-2.0 | Q4_K_M | 1.9 GB |
| `qwen3.5:4b-q4_K_M` | the same | Q4_K_M | 3.3 GB |
| `gemma4:12b-it-q4_K_M` | Google's Gemma 4, Gemma's terms | Q4_K_M | 8.0 GB |

The registry has no Qwen 3.5 file between 4 and 5 GB at this quantisation;
3.3 GB is the middle size. If time is left, `qwen3.5:9b-q4_K_M` (6.6 GB) is
a fourth. Each tag's digest, parameters and licence are read from the
runtime after the pull (`ollama show`, the manifest) and written into the
report.

**The protocol** is round 1w's (`batch.py`, `lms.py`) with the road changed:
the prompt files round 1w wrote (`corpus/round-1w/lm/prompts/<set>/`,
checked against its `hashes.txt`), one user message a schema, no system
prompt, temperature 0, the model's own chat template as the runtime applies
it (`/api/chat`), seed 0, reasoning left at the runtime's default for the
tag as round 1w left each remote model's, at most 32 768 output tokens.
What a local model adds is a context window that costs memory:

- the window is the smallest of 16 384, 32 768 and 65 536 tokens that holds
  the prompt (estimated at one token for 2.5 bytes) twice over, so the
  reply has at least the prompt's own length; the output limit is 32 768
  tokens or what the window leaves after the prompt, whichever is smaller;
- a prompt the runtime reports as longer than the window (or one over
  65 536 tokens by the estimate) **does not fit**; a reply that stops for
  any reason but its own end is **cut**; a reply `lm.py` cannot read is
  **unreadable**. Each counts as no key predicted and is counted, by the
  number of tables of its schema. None is asked again. A request the
  runtime fails or answers with nothing is asked again, twice at most
  (teaching.md);
- replies are kept as they came in
  `corpus/round-1z/llm/<tag>/<set>/<stem>.raw` (the message's content) and
  `.json` (the runtime's whole answer: counts of tokens, durations, why it
  stopped, its reasoning where it gives one, the tag, the digest and the
  date).

**Order**: a pilot of the first 12 prompts of the fresh GitHub set in name
order (development data; it checks the harness and gives each model's
speed, no F1 of it is reported); then, a model at a time from the smallest,
style 3 whole and plain A; then the 398 external databases and LiveSQLBench
if time allows.

**If plain A whole is too slow**: a model whose pilot projects more than
four hours for plain A's 718 prompts reads a sample of it instead. The
sample is fixed here: seed **20261126**; within each of the paper's size
bands (5 to 9 tables, 10 to 19, 20 to 39, 40 to 99, 100 or more; 248, 246,
156, 56 and 12 schemas) the schemas are sorted by identifier, permuted by
`numpy.random.default_rng(20261126 + band index)`, and the first quarter,
rounded up, is taken: 62, 62, 39, 14 and 3, 180 schemas. Every figure of
that model on plain A is then on those 180, lemmekey beside it on the same
180, and the report says so in the table.

**Measured**: tokens in and out and seconds a schema as the runtime counts
them; the graphics card's memory at its highest while a model answers
(`nvidia-smi`, every two seconds, beside what it read before the model was
loaded: the card is shared with the desktop and another round's idle
processes) and how the runtime split the model between the card and the
processor (`ollama ps`); the resident memory of the runtime's processes at
its highest; the bytes of each model's weights file.

## The measures

Round 1v's and round 1w's, through their code (`whole.measure`,
`lm.whole_schema`, `lm.counts`): precision, recall and F1 over every
declared one-column key, a key right when its column and its table are;
macro F1; 95 % percentile intervals of 1 000 resamples of schemas (seed
20261101); each model minus lemmekey with both arms on the same resamples;
by the number of tables, and at 40 tables or more as the paper's Table 5.

**The check before anything is reported**: `eleven--wide`, read through
this round's functions from round 1v's kept counts, must give 0.757 on
plain A and 0.734 on style 3; and when `clean.py` builds a clean set's
candidates, lemmekey's two stages scored on them again must draw the keys
round 1v's reading kept (`heldout-<set>-drawn.json`), which says the
candidates the networks score are the ones lemmekey scored.

**One figure**: F1 on plain A against the bytes of the model on a
logarithmic axis, for lemmekey, the networks, the local language models and
the remote ones (`figure.py`, a PDF and a PNG beside the report's data). The
remote models' weights are not public, so they have no place on the axis;
they are drawn at its right edge, marked as such.

## What counts as an answer

There is no test to pass. The round reports whether a network or a local
model reads above lemmekey on plain A and on style 3 with the interval of
the difference clear of zero, at what size, time and memory; and where
none does, how far below it stands.

## Machine

One PC: Ryzen 7 7700, RTX 5070 12 GB, 31 GB, Windows 11 Pro. The card and
the memory are shared with another round: a file `gpu.lock` in the
session's lock folder is held around each training run and each model's
batch of inference and released between them, and `pairs.lock` around each
build of pairs.

The round's own Python environment, `corpus/round-1z/.venv` (Python 3.13,
PyTorch 2.11.0+cu128, transformers, matplotlib, psutil), holds what round
1b's lacks; everything that scores and counts runs in round 1b's, as before.

## Run

As the round ran in the end; the report has the order and the times.

```sh
cd <repo>/lemmesee
P=round-1b/.venv/Scripts/python
T=corpus/round-1z/.venv/Scripts/python
OLLAMA_MODELS=<models> OLLAMA_HOST=127.0.0.1:11435 OLLAMA_NUM_PARALLEL=1 OLLAMA_MAX_LOADED_MODELS=1   <ollama> serve
$T -I round-1z/local.py ask <tag> fresh-db --first 4                 # the pilot, reasoning on
$T -I round-1z/local.py ask <tag> style3-db --no-reasoning           # a local model, a set; asks only what has no record
$T -I round-1z/local.py ask <tag> plain-a-db --no-reasoning
$P -I round-1z/read.py threshold eleven-first                        # lemmekey's first stage alone
$P -I round-1z/read.py external                                      # the kept pairs of the benchmarks, no pair built
sh round-1z/with-lock.sh <locks> gpu $T -I round-1z/net.py fit char-s   # and granite
sh round-1z/part-a.sh <locks> <repo> external                        # threshold, freeze, candidates, scores, one reading a set, timing, quantisation
$P -I round-1z/score.py <out.json>                                   # every table of the report
$P -I round-1z/falsekeys.py <out.json> <tag folder> ...              # a model's false keys by cause
$P -I round-1z/forgiven.py <out.json>                                # the remote models under the forgiving reading
$T -I round-1z/figure.py <scores.json> <sizes.json> <out folder>
$T -I round-1z/pause.py <locks>                                      # stop cleanly and free the card
```

## Added while the round ran

- **Reasoning, two settings** (15:12 UTC, after one probe of the runtime
  and before the pilot or any prompt of a set): asked for a ten-word reply
  that the probe's prompt spelled out, `qwen3.5:2b-q4_K_M` at its default
  reasoning wrote 2 000 tokens of reasoning and was stopped at the probe's
  limit with no answer. Every tag here reasons by default and lets it be
  turned off (`ollama show`: levels false and true). So each model is asked
  under two settings, kept apart in `llm/<tag>/` and `llm/<tag>--no-reasoning/`:
  **the tag's default**, which is round 1w's protocol, and **reasoning
  off** (`"think": false`), which is what a person who wanted an answer on
  their own machine would try next. The rule for a sample of plain A (four
  hours by the pilot) is applied to each setting of each model apart. The
  tag's other defaults (`top_p`, `top_k`, a presence penalty of 1.5 for
  Qwen 3.5) stay as published; only the temperature and the seed are set.
- **A prompt that does not fit is refused by the runtime** (the same probe:
  HTTP 400, "exceeds the available context size"), so it is counted from
  that refusal and not from an estimate.
- **The licences as the runtime shows them** (`ollama show`, 15:11 UTC):
  Apache-2.0 for all three tags, Gemma 4 among them; the table above said
  "Gemma's terms" from memory of earlier Gemma releases.
- **What the pilot read** (15:12 to 15:58 UTC, the fresh set's first
  prompts, development data): with reasoning off, twelve prompts each, every
  reply ended by itself, a median of 0.2 s (`qwen3.5:2b`), 2.4 s
  (`qwen3.5:4b`) and 3.9 s (`gemma4:12b`) a schema. At the tag's default
  reasoning `qwen3.5:2b` reasoned until its window of 16 384 tokens was
  full on all six prompts it was given, about 64 s each and no answer;
  `qwen3.5:4b` on two of four, about 99 s each. `gemma4:12b` at its default
  was not reached before the plan below was set.
- **The owner's decisions** (about 16:10 UTC, before any prompt of a clean
  set was asked and before any network was trained), which replace the
  plan of two settings for every model:
  - *reasoning off is the measurement*: `qwen3.5:2b`, `qwen3.5:4b` and
    `gemma4:12b` read every schema of style 3 and plain A with reasoning
    off, no sample. The default reasoning stays what the pilot read, and is
    reported with its counts as a finding, not as a figure of F1;
  - *a fourth tag*, `qwen3.5:9b-q4_K_M` (6.6 GB as listed), the largest of
    its family a 12 GB card holds, under the same protocol; a schema its
    window cannot hold counts as no key predicted, by the number of tables;
  - *nothing hosted*: the same open-weights families as hosted models were
    priced on OpenRouter's public list (`corpus/round-1z/openrouter-prices.md`)
    and not run, the account having no balance. None of the three first
    tags is hosted there at all. Open models that do not fit 12 GB
    (Gemma 4 26B, gpt-oss 20B) are therefore not measured;
  - *the order*: the local models on the clean sets, then `char-s` and
    `char-m`, then `granite`, then the external databases if time is left;
    the lock on the card is given back between models.
- **The timing's cap is 2 000 pairs a database**, not 20 000 (set at about
  15:25 UTC, before any timing): 96 % of the 397 databases with pairs have
  fewer, their median is 44 pairs, and `granite` on one thread would
  otherwise take hours on CTU's largest.
- **A pause at 20:00 UTC** (the owner needs the machine; set up at 18:50
  UTC): `pause.py` writes `corpus/round-1z/stop`, which `local.py` reads
  before each prompt and `net.py` before a fit and every hundred steps of
  one, then unloads every model, stops the runtime's server and removes the
  round's locks. `local.py` asked only what had no record from the start,
  so a set continues from the schemas not asked yet; a fit caught by the
  pause keeps nothing and starts again. Timings are not mixed across the
  pause: a model's seconds a schema are reported with the run they came
  from.
- **A reply without its closing brace** (18:50 UTC, from `qwen3.5:4b`'s
  replies on plain A, read once already): 97 of its 718 replies end
  `{"keys": [...]` with no `}` and are unreadable to round 1p's reader,
  which is the protocol's and stays the reported reading. Beside it
  `score.py` reads every local model once more with that one brace added
  (`closing brace mended`), so that a figure lost to a brace is told from a
  figure lost to the model. The remote models' replies are not read again
  this way.
- **The graphics card was lost** (26 November, about 18:42 UTC): while
  `gemma4:12b-it-q4_K_M` read plain A, at its 581st prompt, the card left
  the system (`nvidia-smi`: "GPU is lost. Reboot the system to recover this
  GPU"); the runtime went on answering on the processor at 16 tokens a
  second where the card gave 177, holding 12.8 GB of host memory. The
  coordinator stopped the round at 18:56:59 UTC, before the pause planned
  for 20:00. Kept: every reply written, 586 of Gemma's 718 on plain A among
  them. The five written after the loss are answers of the same model at
  the same settings and are counted as answers; their seconds, and that
  run's memory, are not used (`corpus/round-1z/cpu-replies.json` names
  them). That run wrote no record of its memory, having been ended from
  outside. Whether the round's load caused the loss is not known: the card
  was shared with round 1y's fits at the time. The round resumes after a
  reboot from files alone (`corpus/round-1z/paused.txt`).
- **Resumed after a reboot** (the machine rebooted at 19:00 UTC on
  26 November; the coordinator restarted the server and the job at 22:24
  UTC with the card under a power limit of 187 W set by the owner, the
  round's processes at below-normal priority). Read that night: Gemma's
  last schemas of plain A and `qwen3.5:9b-q4_K_M` on both sets. Seconds a
  schema are reported by run, the evening's at 250 W apart from the
  night's at 187 W; the five replies written on the processor are in
  neither.
- **The first fit of `char-s` was stopped** (27 November, 05:35 UTC, by the
  coordinator): 500 steps in 68 s, then the validation split's scoring took
  an hour with the card's memory at 11.8 GiB, spilled into the host's.
  Nothing of it is kept. The fault and its mend are in the next entry; no
  hyperparameter of the specification changes.
- **A target written with its column** (27 November, 05:45 UTC, from
  Gemma 12b's false keys on plain A, read already): 10 557 of its 14 127
  false keys there name the target as `users(id)` or `users.id`, which the
  protocol's reader takes for a table the schema does not hold, so the key
  is counted false and the declared one missed. `falsekeys.py` counts a
  model's false keys by cause. Beside the protocol's reading, which stays
  the reported one, `score.py` reads every language model's replies once
  more with two faults of form forgiven (`form mended`): the missing
  closing brace of the entry above, and a target written with its column
  where what stands before the bracket or the last dot is a table of the
  schema. The remote models' saved replies are read the same second way,
  so that the forgiveness is not the local models' alone; this replaces
  the earlier entry's "not read again".
- **The fault of the first fit, and its mend** (27 November, 06:00 UTC, no
  fit run since): `net.py` scored the validation split shortest pair
  first, 4 096 a batch at full precision; each new length asked for a
  block larger than any the allocator had kept, the kept blocks added up to
  the whole card, and the driver let the process spill into the host's
  memory. Mended: longest first, 1 024 a batch, half precision on the card
  in scoring and training for every network, a cap on what the process may
  allocate (9.5 GiB) and a guard that stops a fit or a scoring with a
  message when the card's reserved memory passes 9 GiB. No hyperparameter
  of the specification changes. That this was the fault is a diagnosis
  from the code and the log; a second run confirms it or not.
- **Part B is written up; Part A waits for the owner** (27 November, 06:10
  UTC): the report gives the four local models on both clean sets and an
  estimate for the networks, 4 to 5.5 hours of the card as specified and
  about 1.5 hours for `char-s` as specified and `granite` with one epoch,
  `char-m` dropped. The external databases and LiveSQLBench were not asked
  of any local model.
- **Part A, cut down, by the owner's word** (27 November, 05:58 UTC, before any fit of it is started): `char-s` as
  specified; `granite` with one epoch in place of two and so one scoring
  of the validation split, which then only chooses its threshold;
  `char-m` dropped. Why: the whole of Part A was estimated at 4 to 5.5
  hours of a card that was lost under long load on 26 November, and the
  two networks kept bracket the reviewer's question, one of the trees' own
  size and one a hundred times larger. Both are frozen, the clean sets'
  candidates built, each set read once, both timed on one processor in the
  container; the external databases only if that is done by 09:30 UTC. The
  card runs under the owner's limit of 187 W, at normal priority.
- **The forgiving reading as data** (27 November, 06:05 UTC, at the
  coordinator's word): `forgiven.py` reads every remote model's saved
  replies of rounds 1s, 1t and 1w both ways on every set, and the pair
  task's replies for the same fault; the paper is to report the forgiving
  reading, named as one introduced after the replies were seen.
- **`char-s` is fitted** (27 November, 05:58 to 06:13 UTC, 870 s under
  the 187 W limit, at most 2.9 GiB of the card allocated): the mend holds.
  Its best epoch on the validation split is the ninth, F1 0.590 at
  threshold 0.870, where lemmekey's first stage alone reads 0.772 on the
  same pairs.
- **`granite`'s first fit was stopped by the guard** (06:13 to 06:24 UTC):
  its one epoch trained in 630 s, and the scoring of the validation split
  then passed 9 GiB, its blocks coming on top of the 7.5 GiB the training
  had cached; the guard stopped it as written and nothing spilled, but the
  epoch's weights had not been saved. `net.py` now saves an epoch's weights
  before its scoring, gives the training's cached memory back first and
  scores the encoder 256 pairs a batch. The fit is run again from the
  start with the same seed; nothing of the first is used.
- **`granite` is fitted, both are frozen, every set is read** (27
  November): the second fit 06:24 to 06:39 UTC, 895 s with its scoring at
  7.4 GiB of the card, validation F1 0.736 at threshold 0.759. `part-a.sh`
  then ran alone from 06:39 to 07:48: the freeze at 06:39:30; the
  candidates of style 3 and plain A, on which lemmekey draws round 1v's
  keys in 100 of 100 and 718 of 718 schemas; both networks' scores of the
  two clean sets, of Spider, BIRD, CTU and LiveSQLBench; each set's one
  reading at 06:58; the timing in the container, 06:59 to 07:35; the
  quantised readings. The external databases were read because all of it
  had begun well before 09:30. Docker Desktop was not running after the
  reboot and was started for the timing.
- **The agent was cut off** by a usage limit at about 08:00 UTC, after the
  script had ended, and came back at 12:00; nothing ran in between. On its
  return: `char-s`'s quantised reading, which had saved its file and failed
  at its first forward (PyTorch's fast path of a transformer layer reads a
  quantised layer's weight as a tensor), was run again with that path off;
  everything was scored, the figure drawn with the networks on it and the
  report finished for both parts.
- **What was not done of the specification**: `char-m`; `granite`'s second
  epoch; the compact network under round 1v's second stage, which was
  offered "if time is left"; any local model on the external databases; the
  default reasoning beyond the pilot; plain A's sample, which no model
  needed.
