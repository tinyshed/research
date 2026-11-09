# see's questions through a model's keys, 9 November 2026

Round 1u, as [its specification](../round-1u/README.md) sets it. The key
model's paper is framed by `see`: round 1c's analytical question
([aq.py](../round-1c/aq.py)) reaches a breakdown, a header's time and a
product's price through foreign keys, many to one. This round asks what
those questions become when the keys come from round 1t's model, the
paper's `seven-restyled` as frozen, rather than from the schema's
declarations, and against round 1's rules. It chooses no model; Spider,
BIRD, CTU and LiveSQLBench are development data here. Raw output in
[data/keys-questions-2026-11-09/](data/keys-questions-2026-11-09/).

| Question | Answer | What follows |
|---|---|---|
| **Of the questions that join through declared keys, how many does `see` still ask with the model's keys?** | A database's share, the mean over databases: Spider **0.886** [0.853, 0.919], BIRD **0.808** [0.734, 0.868], CTU **0.814** [0.756, 0.869], LiveSQLBench **0.398** [0.263, 0.537] | the model keeps four in five of a database's joining questions where names are `table_id`, two in five on LiveSQLBench |
| And with round 1's rules? | 0.687, 0.654, 0.615 and **0.007** | the model keeps 0.15 to 0.20 more on the three, and on LiveSQLBench the rules keep nothing |
| How many go through a key no declaration names? | Of the model run's questions: 5.4 %, 14.0 %, 13.1 % and 12.8 % (1 239, 7 023, 41 094 and 2 297 questions); of the rules': 6.6 %, 8.8 %, 15.7 % and 2.4 % | about one question in eight the model's keys ask goes through a key nobody declared, wrong or real: not told apart here |
| Does the ranker's five change? | Of the declared run's five, the model's run keeps 0.91, 0.85, 0.80 and 0.72; the rules' 0.84, 0.84, 0.75 and 0.64 | the five lean on a table's own columns, and move less than the questions that join |

## How it was run

On every database of the four sets, as round 1t reads them, the schema is
written three times as the snapshot `aq.load` reads (tables, columns, their
types, primary and unique keys; no statistics):

- **declared**: its declared one-column keys, nothing guessed;
- **rules**: no declaration, and `aq._guess_keys` as `see` runs it, round 1's
  rule of [candidates.md](../round-1/candidates.md) (`<table>_id`, `<table>id`)
  widened as round 1c wrote it (`key`, `code`, `no` endings, a column named as
  a table's one-column key);
- **model**: no declaration, the keys `seven-restyled` draws at its threshold,
  its files checked by hash against round 1t's `frozen-final.json`, nothing
  guessed.

`aq.generate` makes each run's questions and `rank.veto` removes what it
removes, as `see.py` runs them. A question is its identity and its shape; it
joins when its time or its breakdown is reached through a key, or its
measure needs one (parents counted, rows a parent, a line's quantity by its
product's price). A question of another run is the declared run's when its
identity and shape are the same. The five are round 1c's learned ranker
(`ranker.txt`, configuration A, no statistics) and `rank.choose`.

## Results

([questions.json](data/keys-questions-2026-11-09/questions.json); intervals
from 1 000 resamples of databases, seed 20261101)

| | databases | joining questions, declared | still asked, rules: a database's mean / pooled | still asked, the model: a database's mean / pooled |
|---|---|---|---|---|
| Spider | 204 | 19 047 | 0.687 [0.632, 0.738] / 0.431 [0.241, 0.684] | **0.886** [0.853, 0.919] / 0.603 [0.346, 0.901] |
| BIRD | 76 | 32 355 | 0.654 [0.568, 0.738] / 0.585 [0.417, 0.720] | **0.808** [0.734, 0.868] / 0.720 [0.552, 0.864] |
| CTU | 118 | 170 542 | 0.615 [0.539, 0.685] / 0.310 [0.149, 0.628] | **0.814** [0.756, 0.869] / 0.683 [0.407, 0.859] |
| LiveSQLBench | 22 | 20 508 | 0.007 [0.002, 0.013] / 0.010 [0.003, 0.018] | **0.398** [0.263, 0.537] / 0.453 [0.269, 0.661] |

| | the run's questions through an undeclared key: rules | the model | the declared five kept: rules | the model |
|---|---|---|---|---|
| Spider | 1 329, 6.6 % [2.2, 13.1] | 1 239, 5.4 % [1.9, 11.5] | 0.835 [0.803, 0.863] | 0.908 [0.883, 0.934] |
| BIRD | 4 089, 8.8 % [2.2, 18.0] | 7 023, 14.0 % [5.4, 24.3] | 0.844 [0.793, 0.888] | 0.852 [0.798, 0.894] |
| CTU | 39 393, 15.7 % [5.7, 21.6] | 41 094, 13.1 % [8.0, 18.6] | 0.748 [0.698, 0.790] | 0.798 [0.751, 0.846] |
| LiveSQLBench | 185, 2.4 % [0.0, 5.7] | 2 297, 12.8 % [6.0, 20.6] | 0.636 [0.527, 0.746] | 0.718 [0.627, 0.809] |

A database's mean is the headline, not the pooled share: a question reaches
up to three keys deep, so a database with many keys makes thousands of
questions (CTU's largest 51 123 of its 170 542), and the pooled share is
that database's. One key missed loses every question through it, and every
chain that goes on from it: on Spider the model draws keys at F1 0.888
(round 1t) and a database keeps 0.886 of its joining questions; on
LiveSQLBench 0.587 and 0.398.

## What it says for the paper

- With the model's keys and no declaration, `see` asks a database's
  questions that join through declared keys 0.81 to 0.89 of the time on
  Spider, BIRD and CTU, against 0.62 to 0.69 with round 1's rules; on
  LiveSQLBench 0.40 against 0.01.
- About one in eight of the questions the model's keys ask goes through a
  key no declaration names (5 % on Spider, 13–14 % on the others). Round 1q
  found many such keys on the owner's database real; on these sets they
  are not judged, and the number is an upper bound on wrong ones.
- The five a person sees first change less: 0.72 to 0.91 of the declared
  five stay with the model's keys, 0.64 to 0.84 with the rules.

## Limits

- One ranker, trained on round 1c's teachers; the five are its five, without
  statistics. With statistics (`reads_n`) more questions would be vetoed.
- A question's identity says what it asks, not by which path: a question the
  model asks through a wrong path into the right table counts as still
  asked, and as through an undeclared key.
- The sets are those the model was chosen on (Spider, BIRD, CTU) and
  LiveSQLBench, development since round 1s; no held-out set has questions
  here.
- A composite key is no one's here: the declared run takes one-column keys
  only, as the model and the rules draw.

## Environment and reproduction

| | |
|---|---|
| machine | AMD Ryzen 7 7700, 31 GB, NVIDIA GeForce RTX 5070, Windows 11 Pro |
| Python | 3.13.15; xgboost 3.4.1, lightgbm 4.7.0, numpy 2.5.4 |
| model | round 1t's `seven-restyled`, hashes as in [frozen-final.json](data/keys-style-2026-11-08/frozen-final.json) |
| questions and ranker | round 1c's `aq.py`, `rank.py`, `learn.py`; `corpus/round-1c/learn/ranker.txt` and `words.json` |
| sets | Spider, BIRD, CTU and LiveSQLBench as round 1s read them (`corpus/round-1s/external`, `corpus/round-1s/heldout`) |
| research commit | `df6b50b` and this round's files |
| time (UTC) | 9 November 2026, specification 13:51, run 13:52–13:58 |

```sh
cd <repo>/lemmesee
round-1b/.venv/Scripts/python -I round-1u/questions.py
```
