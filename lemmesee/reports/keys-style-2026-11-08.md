# The key model in another house style, 8 November 2026

Round 1t, as [its specification](../round-1t/README.md) sets it. Round 1s's
key model ([report](keys-clean-2026-11-07.md)) read F1 0.185 on
LiveSQLBench, a set named in a house style SchemaPile hardly has. Looked
into, the loss was a threshold set on SchemaPile and names the model's cues
could not read. This round repairs it in two passes: first cues that read a
name against its target's key and a shortened word, and training on
SchemaPile renamed into drawn house styles; then, over the night of
8 November at the owner's goal of F1 0.80 on every benchmark with a model
that carries, wider candidates, run-together words split and shortenings
expanded from lexicons language models wrote offline, a graph stage read
against its own schema, and real schemas renamed by a language model. Every
fit is measured on the development sets beside round 1s's model; the one the
specification's rule chooses, and three beside it, were frozen and read once
on a set nobody here had read: 80 schemas on GitHub created after SchemaPile
was gathered. The reader in Rust carries the chosen model and draws exactly
the libraries' keys. The specification was written before any fit, each
later addition marked there with when it was made. Raw output in
[data/keys-style-2026-11-08/](data/keys-style-2026-11-08/).

Every F1 below is over every declared one-column key, a key right when its
column and its table are, as round 1s counted it. Every interval is a 95 %
percentile interval of 1 000 resamples of databases; one of a difference
resamples both arms together. LiveSQLBench is development data from round
1s's reading on; the renamed SchemaPile test is drawn from the styles the
training renamed into; BEAVER was read once by an earlier freeze and is seen
since. Only the GitHub reading is clean.

**Added on the morning of 8 November** (§7): a defect in the chosen model's
second stage, mended; features that read a schema's own house style from
its columns; and a second clean set, the 177 GitHub schemas the same rule
kept and did not select, read once. On it the chosen model's gain over
`1s` is +0.011 [−0.016, +0.042], and over both GitHub sets +0.022
[−0.002, +0.047]: smaller than the first reading's.

| Question | Answer | What follows |
|---|---|---|
| **On schemas nobody here had read, once?** | 80 GitHub schemas created 1 July 2025 to 6 November 2026, 1 600 tables, 2 022 keys: the chosen `seven-restyled` **0.792** [0.747, 0.829] against `1s`'s 0.744 [0.685, 0.798], **+0.048 [+0.014, +0.084]**; `seven-relative` 0.799, +0.055 [+0.020, +0.097]; `cues-restyled` 0.781, +0.037 [+0.001, +0.077] | the repaired model carries to new real schemas: the paper's model is `seven-restyled`, the paper's claims stand on this reading |
| Where did round 1s's 0.185 go? | Half was the threshold (0.453 at 0.5), the rest names: 1 % of LiveSQLBench's key columns end in `id` against Spider's 79 %, a key named after its target's key (`zoneref` into `sites`, keyed `zoneregistry`), shortened words | a model of names learns its training set's house style |
| What repaired it? | Cues against the target's key and shortenings (v5), SchemaPile renamed into drawn styles: `cues-restyled` takes LiveSQLBench to 0.556 (+0.371 [+0.237, +0.507]) and SchemaPile renamed from 0.515 to 0.781, at −0.016 on SchemaPile's own test | renaming the training data, and cues that need no style list, are the round's result |
| And the night? | Split words, Haiku-expanded shortenings and wider candidates (v7, `seven-restyled`): pooled development 0.748 against 0.700 (+0.048 [+0.002, +0.102]), CTU 0.670, LiveSQLBench 0.587, Spider 0.888, SchemaPile's test 0.726 (−0.010); the schema-relative graph stage takes LiveSQLBench to 0.639–0.652 and costs CTU | the rule written before the fits chose `seven-restyled` |
| What did not help? | LLM-written schemas (`1s` already reads them at 0.921); real schemas renamed by Haiku (394, against the same restyled once more: LiveSQLBench 0.528 against 0.547); the related-entity lexicon alone costs SchemaPile 0.012 to 0.020 | language models teach this model best as lexicons, not as schemas |
| Can a threshold reach 0.80 on CTU and LiveSQLBench? | No: a perfect threshold for every schema on `cues-restyled`'s scores reaches CTU 0.764–0.777 and LiveSQLBench 0.631–0.654; a perfect has-a-key decision with today's targets 0.868 and 0.782 | the rest is ranking: semantics, and CTU's `Bench` (101 keys into tables differing only in case) |
| Is SchemaPile's test a fair 0.80? | On its 1 821 schemas whose every rule-named key is declared F1 is 0.76–0.78; on the other 294 precision is 0.45–0.54: many false keys are undeclared real ones | SchemaPile's test caps any model of names near 0.78 |
| Does the reader in Rust carry it? | Yes: on 1 899 817 external and 1 502 214 test pairs no hashed feature differs, the first stage's margins are equal, the second's within 1.4e-4, the same 2 903 and 15 121 keys drawn | the model is a file and a reader of 2 400 lines; at 2.6 ms a median database through its reference path, a fast path still to write |
| **And on 177 more GitHub schemas, read once in the morning?** | 4 533 keys: `seven-restyled` 0.767 [0.715, 0.813] against `1s`'s 0.756, **+0.011 [−0.016, +0.042]**; `seven-bounded` 0.775, `seven-style` 0.762; both sets pooled afterwards, 252 schemas: +0.022 [−0.002, +0.047] | on `table_id` names the repair costs nothing and gains little; its gains are where the style shifts |
| What was wrong with the chosen model? | Its second stage holds 43 leaves over 50, up to 15 832, drawing pairs its first stage scores under 0.001 at 1.0; bounded, validation average precision 0.689 → 0.779 | a stacked second stage needs its leaves bounded |
| Does reading the schema's own style help? | On LiveSQLBench 0.619 → 0.671 (`seven-style`), at CTU −0.032; not on the second GitHub set (0.762) | it reads the style it was written for, on development data only |

## 1. What round 1s's reading lost

Looked into after the reading, at the owner's request to repair the model
(round 1s's frozen model, its 9 516 candidate pairs of LiveSQLBench's 22
databases):

| | |
|---|---|
| at the frozen threshold, 0.837 | P 0.681, R 0.107, **F1 0.185** |
| at 0.5, the same scores | P 0.636, R 0.351, **F1 0.453** |
| the second stage's AUC on its pairs; the name score's | 0.848; 0.784 |
| keys whose target is their column's best-scored | 152 of the 272 the candidates hold |
| key columns ending in `id`: Spider; LiveSQLBench | 79 %; 1 % (27 % `ref`, 20 % `link`, the rest `_tie`, `_join`, shortened words) |
| a rule, the column's stem equal to a target key's stem | F1 0.418 on LiveSQLBench |

## 2. What changed

**First pass** ([cues.py](../round-1t/cues.py), [restyle.py](../round-1t/restyle.py),
[teach.py](../round-1t/teach.py)):

- *version 5's cues*, fourteen, each from the pair's names and its schema:
  the common prefix of the column and the target's key, the stems equal, a
  stem being a name without the ending most of the schema's keys or 15 % of
  its columns share, the stem a shortening of the target or its words, how
  many tables carry the column's name, how often its last word ends a column
  and a key;
- *renamed SchemaPile*: a style drawn a schema shortens, cases and joins
  words, prefixes tables, ends keys and references in words drawn from 25,
  one shortening a word throughout; half the 8 024 training schemas with keys
  replaced by a renamed copy (4 012), every validation schema given one;
- *teachers' schemas*: Claude Haiku 5.5 (24 agents) and Claude Sonnet 5.5 at
  medium effort (18) wrote 336 schemas of 336 domains in twelve named styles,
  4 717 keys, all resolving.

**The night** ([six.py](../round-1t/six.py), [words7.py](../round-1t/words7.py),
[stack.py](../round-1t/stack.py), [rename.py](../round-1t/rename.py)):

- *version 6*: `longtext` (PostgreSQL's `text` as round 1b read it: 3.3 % of
  SchemaPile's test keys were out of the candidates for it) reads as text,
  `decimal` as an integer; a target through each column of a key of several
  columns and through each unique column; across families where the names
  say so; cues for tables that relate others. The candidates hold 96.8 %
  of SchemaPile's test keys (from 92.4 %), CTU's 99.5 % (94.7 %), Spider's
  98.4 %, BIRD's 96.9 %, LiveSQLBench's 95.7 % (91.0 %). Round 1r had tried
  unique targets and decimal keys and lost precision; so did this alone;
- *version 7*: a name's run-together words split by the 5 363 words of
  SchemaPile's training names seen 20 times or more (`custref` cust + ref,
  `prodcatlink` prod + cat + link), and 999 shortenings of its training names
  expanded by Haiku agents (`cust` customer, `dept` department), the column's
  words compared with its target's and its key's;
- *version 8*: 1 895 words mapped by Haiku agents to the entities a column
  so named refers to (author: user, person; pilot: driver; track: circuit);
- *the graph stage against its schema* (`-relative`): six features of a pair
  against its schema's scores (over the schema's best, its place among the
  columns' bests, its log lead), and the second stage fitted, for the renamed
  schemas, on a first stage that saw none of them;
- *real schemas renamed by a language model*: 394 of 400 SchemaPile training
  schemas renamed by Haiku agents in six semantic styles (synonyms, roles,
  the target's key, legacy codes, run-together words, another language), the
  keys following the map; the control is the same 394 drawn into
  `restyle.py`'s styles once more.

Every fit is round 1s's procedure at seed 20261027, on the graphics card,
8 to 14 minutes a fit.

## 3. The development sets

F1 over every declared key; the threshold the fit's own; in brackets each
minus `1s` on the same resamples.

| Fit | threshold | SchemaPile test (2 115 schemas, 13 455 keys) | renamed (2 055) | teachers' test (80) | Spider (204) | BIRD (76) | CTU (118) | LiveSQLBench (22) | pooled four (420, 3 670 keys) |
|---|---|---|---|---|---|---|---|---|---|
| `1s` | 0.837 | 0.736 | 0.515 | 0.921 | 0.869 | 0.830 | 0.609 | 0.185 | 0.700 |
| `cues` | 0.850 | 0.743 (+0.008 [+0.002, +0.013]) | 0.550 | 0.919 | 0.873 | 0.812 | 0.614 | 0.300 (+0.115) | 0.705 (+0.005) |
| `restyled` | 0.803 | 0.702 (−0.034) | 0.727 | 0.934 | 0.865 | 0.819 | 0.593 | 0.413 (+0.228) | 0.695 |
| `cues-restyled` | 0.807 | 0.720 (−0.016 [−0.025, −0.004]) | 0.781 | 0.937 | 0.867 | 0.845 | 0.633 | 0.556 (+0.371) | 0.731 (+0.031 [+0.005, +0.060]) |
| `cues-restyled-taught` | 0.823 | 0.720 | 0.787 | 0.951 | 0.869 | 0.817 | 0.615 | 0.540 | 0.717 |
| `six-restyled` | 0.780 | 0.712 (−0.024) | 0.783 | 0.937 | 0.885 | 0.810 | 0.604 | 0.552 | 0.710 |
| **`seven-restyled`** | 0.823 | **0.726 (−0.010 [−0.022, +0.004])** | 0.794 | 0.941 | **0.888** | 0.816 | **0.670 (+0.060 [−0.036, +0.161])** | 0.587 (+0.402 [+0.282, +0.516]) | **0.748 (+0.048 [+0.002, +0.102])** |
| `cues-relative` | 0.841 | 0.702 (−0.033) | 0.748 | 0.938 | 0.851 | 0.808 | 0.649 | 0.586 | 0.729 |
| `seven-relative` | 0.867 | 0.729 (−0.006 [−0.017, +0.005]) | 0.786 | 0.944 | 0.888 | 0.813 | 0.645 | **0.639 (+0.454 [+0.324, +0.575])** | 0.742 (+0.041 [+0.013, +0.072]) |
| `eight-restyled` | 0.808 | 0.724 (−0.012) | 0.798 | 0.949 | 0.891 | 0.803 | 0.567 | 0.632 | 0.695 |
| `eight-relative` | 0.825 | 0.716 (−0.019 [−0.030, −0.007]) | 0.771 | 0.951 | 0.889 | 0.819 | 0.654 | 0.652 (+0.467) | 0.746 (+0.045 [+0.017, +0.075]) |
| `cues-restyled-llm` | 0.811 | 0.721 | 0.783 | 0.942 | 0.866 | 0.824 | 0.641 | 0.528 | 0.728 |
| `cues-restyled-more` (its control) | 0.843 | 0.719 | 0.791 | 0.936 | 0.871 | 0.827 | 0.630 | 0.547 | 0.726 |
| `seven-relative-llm` | 0.873 | 0.729 | 0.783 | 0.946 | 0.882 | 0.811 | 0.637 | 0.652 | 0.737 |
| `seven-restyled` and a has-a-key head (`pi.py`) | 0.30 | 0.738 | 0.805 | 0.935 | 0.874 | 0.809 | 0.652 | 0.551 | |

([summary.json](data/keys-style-2026-11-08/summary.json): precision,
recall, the macro F1, F1 at 0.5 and every interval.)

Read across:

- **Renaming the training data is the largest single repair** (SchemaPile
  renamed 0.515 → 0.727, LiveSQLBench 0.185 → 0.413), and it costs
  SchemaPile's own test, in precision, at a lower threshold.
- **Cues that need no style list add to it** (`cues-restyled`): +0.054 on the
  renamed test and +0.143 on LiveSQLBench over renaming alone.
- **Words split and expanded (v7) are the night's gain**: CTU 0.633 → 0.670,
  Spider 0.867 → 0.888, the pooled four 0.731 → 0.748; SchemaPile's test back
  to 0.726.
- **The schema-relative graph stage trades CTU for LiveSQLBench**: 0.587 →
  0.639 on LiveSQLBench, 0.670 → 0.645 on CTU. CTU's Mooney_Family, whose 134
  keys are each the column's best target at scores of 0.12 to 0.17, stays at
  none drawn under every fit: the second stage learned on SchemaPile not to
  trust `name1`, `name2` into `person(name)`.
- **The related-entity lexicon (v8)** lifts LiveSQLBench and costs
  SchemaPile's test 0.012–0.020; **the LLM-renamed schemas** do no better
  than their control (LiveSQLBench 0.528 against 0.547, CTU 0.641 against
  0.630), so their scaling was not run; **the teachers' schemas** add nothing
  beyond their own easy test.
- **A has-a-key head** over `seven-restyled`'s scores, learned on half of
  SchemaPile's validation split: SchemaPile's test 0.726 → 0.738, Spider,
  CTU and LiveSQLBench down 0.014 to 0.036. Learned where names are
  SchemaPile's, it carries their habits; not taken.

**The rule** written before the fits — the best pooled development F1 among
fits within 0.01 of `1s` on SchemaPile's test — chose `cues` before the
night, and `seven-restyled` after it (pooled 0.748; SchemaPile's test 0.726,
0.010 below `1s`, on the line).

### Where 0.80 stops

Six design agents (Claude Opus 5.5 at extra-high effort) were each asked for
another architecture — a tiny neural name encoder, structured inference over
the schema, per-schema convention induction, ranking and positive-unlabeled
objectives, distillation, one model for all of `see` — and measured bounds on
`cues-restyled`'s saved scores (read-only scripts of their own, not kept):

| | Spider | BIRD | CTU | LiveSQLBench |
|---|---|---|---|---|
| `cues-restyled` | 0.867 | 0.845 | 0.633 | 0.556 |
| the best threshold for the set | 0.873 | 0.849 | 0.669 | 0.579 |
| the best threshold for each schema | 0.910 | 0.889 | 0.764–0.777 | 0.631–0.654 |
| a perfect has-a-key decision, today's targets | 0.945 | 0.938 | 0.868 | 0.782 |
| perfect targets, today's decision | 0.872 | 0.876 | 0.662 | 0.609 |

No calibration reaches 0.80 on CTU or LiveSQLBench; the ranking has to
change. Of CTU's misses, 320 of 913 are its one database `Bench`, where 101
keys point into one of two or three tables whose names differ only in case
(`Address`, `ADDRESS`), which names cannot tell apart; CTU without `Bench` is
0.708 under `cues-restyled`. On SchemaPile's test, the 1 821 schemas whose
every key the name rules find is declared read F1 0.759–0.781, the other 294
0.547–0.601 at precision 0.45–0.54.

## 4. BEAVER, read early

BEAVER (`target-benchmark/beaver-corpus` at revision `8b683135`, six
enterprise databases, 463 tables, 440 one-column keys, 58 composite keys left
out) was read once by `1s`, `cues` and `cues-restyled` as frozen at 00:09 UTC
on 8 November — a reading begun to be ready when the night's goal was set,
interrupted, found finished — and is reported as it came:

| | P / R / F1 [95 %] | macro F1 |
|---|---|---|
| `1s` | 0.450 / 0.257 / 0.327 [0.093, 0.536] | 0.440 |
| `cues` | 0.549 / 0.307 / 0.394 [0.103, 0.630] | 0.488 |
| `cues-restyled` | 0.600 / 0.266 / 0.369 [0.100, 0.625] | 0.471 |

Its candidates held 230 of the 440 keys (one database's 200 none at all):
coverage, which version 6 widened afterwards. BEAVER is seen from then on and
not read again; six databases make its intervals as wide as they are.

## 5. The GitHub set, once

Built by [github.py](../round-1t/github.py) on 8 November before any of its
schemas was read, by rules fixed in its collector's instructions: code search
for SQL declaring foreign keys gave 28 045 repositories; 15 612 created on or
after 1 July 2025, none forks or archived; of the first 400 by SHA-256 of
`owner/name`, the one `.sql` file each declaring the most keys at the default
branch's head, parsed by sqlglot, kept with at least five tables and three
resolving keys and no copy of a sample database (257); the first 80 by
the same hash. 80 schemas, 1 600 tables, 2 022 one-column keys (34 composite
left out), created 1 July 2025 to 6 November 2026; 63 without a licence, 11
MIT, 2 each GPL-3.0, AGPL-3.0, Apache-2.0 — named here by count only. Its
hash was frozen at 00:34 UTC; the four models at 02:57 UTC
([frozen-final.json](data/keys-style-2026-11-08/frozen-final.json)); read
once ([heldout-github-2025.json](data/keys-style-2026-11-08/heldout-github-2025.json)):

| | P / R / F1 [95 %] | minus `1s` | macro F1 | F1 at 0.5 | keys among candidates |
|---|---|---|---|---|---|
| `1s` | 0.858 / 0.656 / 0.744 [0.685, 0.798] | | 0.757 | 0.767 | 1 960 |
| `cues-restyled` | 0.843 / 0.727 / 0.781 [0.741, 0.815] | +0.037 [+0.001, +0.077] | 0.753 | 0.772 | 1 960 |
| **`seven-restyled`**, the rule's | 0.832 / 0.756 / **0.792** [0.747, 0.829] | **+0.048 [+0.014, +0.084]** | 0.783 | 0.778 | 1 977 |
| `seven-relative` | 0.857 / 0.749 / 0.799 [0.759, 0.835] | +0.055 [+0.020, +0.097] | 0.778 | 0.776 | 1 977 |

On real schemas no round had read, the repaired model draws more keys at
nearly the same precision: recall 0.656 → 0.756, precision 0.858 → 0.832.
The relative graph stage, chosen against by the rule on CTU, reads 0.007
higher here, within its interval.

## 6. The reader in Rust

Round 1s's reader ([round-1t/reader/](../round-1t/reader/)) with version 5's
cues in both its paths (`cues.rs`, 194 lines) and versions 6 to 8 and the
relative graph stage in a reference path (`six.rs`, 493 lines); the three
lexicons, 170 KB, compiled into the binary. `keys.bin`'s fifth byte says what
a model reads: 1 round 1s's, 2 version 5, 3 versions 6–8 with a byte of
flags after it. A model file stays 960 819 or 960 820 bytes.

**Parity** against XGBoost and LightGBM ([data](data/keys-style-2026-11-08/)):

| | pairs | hashed features differing | first stage, largest margin difference | second stage, largest; pairs over 1e-4 | keys drawn, the same |
|---|---|---|---|---|---|
| `cues-restyled`, external | 712 774 | 0 | 0 | 5.4e-7; 0 | 2 514 |
| `cues-restyled`, SchemaPile test | 1 101 364 | 0 | 0 | 5.4e-7; 0 | 14 408 |
| `seven-restyled`, external | 1 899 817 | 0 | 0 | 1.4e-4; 2 988 | 2 903 |
| `seven-restyled`, SchemaPile test | 1 502 214 | 0 | 0 | 1.4e-4; 829 | 15 121 |
| `seven-relative`, LiveSQLBench and the teachers' test | 40 143 | 0 | 0 | 2.4e-7; 0 | 199 and 1 050 |

**Time and memory**, in one run of the Linux container (`rust:1.99.1`, one
CPU, round 1s's settings, the readers alternating twice):

| | round 1s's reader, `1s` | round 1t's, `cues-restyled` | round 1t's, `seven-restyled` |
|---|---|---|---|
| a database, median of 397 declaring keys, two passes | 1.50, 1.53 ms | 1.53, 1.55 ms | 2.56, 2.59 ms |
| the same, 90th percentile | 15.4–16.2 ms | 15.6, 16.1 ms | 28.1, 28.0 ms |
| over `1s`'s in the same run, a database's median ratio | | 0.995, 0.978 | 1.54, 1.57 |
| CTU's `Bench`: pairs; seconds; resident | 622 512; 25.6–26.3 s; 298 MiB | 622 512; 24.8–25.4 s; 312 MiB | 1 717 591; 83–84 s; 916 MiB |
| resident, CTU's `Walmart` (median size) | 4.9–5.0 MiB | 5.2 MiB | 6.5 MiB |
| binary | 888 160 bytes | 919 512 bytes | 1 227 024 bytes (the lexicons inside) |

([rust-v5-*.json](data/keys-style-2026-11-08/) from the first run, 22:57–23:14 UTC on
7 November; [rust-v7-*.json](data/keys-style-2026-11-08/) from the second, 03:08–03:41 UTC.)
Version 5's cues cost nothing in time. `seven-restyled` reads every pair
through the reference path, which names each feature as a string: half as
fast again for a median database, three times on `Bench`, whose candidates
version 6 widened 2.8 times. A fast path for versions 6 to 8, as round 1s
wrote one for its features, is the next piece of work.

## 7. The morning: a defect, the schema's own style, a second GitHub set

Added on the morning of 8 November, at the owner's request to keep going; the
specification's entries of the morning say when each step was fixed.
[errors.py](../round-1t/errors.py) sorted `seven-restyled`'s declared and
drawn keys on two development sets
([errors-*.json](data/keys-style-2026-11-08/), 60 of each kind shown):

| | found | missed: under the threshold | missed: another target won | missed: no candidate | drawn, no key declared | drawn, another target declared |
|---|---|---|---|---|---|---|
| CTU, 1 854 keys | 1 096 | 437 | 311 | 10 | 241 | 82 |
| LiveSQLBench, 299 keys | 138 | 80 | 68 | 13 | 18 | 15 |

On CTU, `Bench` makes 442 of the 1 081 errors; a logic program's facts
(Mooney_Family, PTE, Mesh, SAT, Pima, Bupa: columns `arg1`, `name2`, `tm`)
most of the rest; and many keys drawn where none is declared read as real
undeclared ones (`lineitem.l_partkey` into `part` in TPC-H, `teamID` into
`teams` in lahman). On LiveSQLBench the model keeps its precision and loses
recall: keys named in one schema's own style (`acct_mon`, `acct_node`,
`acct_gate`, `acct_anchor` into `accounts`) each score 0.46 to 0.72, under
the threshold.

**A defect.** In AdventureWorks the model drew `PasswordSalt` into
`SalesPersonQuotaHistory` and `SalesTerritory.Name` into
`SpecialOfferProduct` at 1.0, pairs its first stage scores at 1e-5 to 3e-4.
`seven-restyled`'s second stage holds 43 leaves of absolute value over 50,
the largest 15 832, where `seven-relative`'s largest is 7.9 and
`cues-restyled`'s 10.3 ([second-stage-leaves.json](data/keys-style-2026-11-08/second-stage-leaves.json));
one pair's margin took +2 082 from its own score's leaves. The second stage
is LightGBM at its defaults for a leaf's least hessian (0.001) and its
largest output (none), which lets a leaf on a few near-certain pairs run
away. `seven-bounded` is the same fit with each leaf at most 1.0, a leaf's
hessian at least 1 and an L2 of 1 (`BOUNDED` in fit.py): its largest leaf
2.6, validation F1 0.740 → 0.749 and average precision 0.689 → 0.779.

**The schema's own style** ([style.py](../round-1t/style.py)): six features
that read a pair against the schema's other columns named alike, from the
first stage's scores and the names alone — of the columns sharing the
pair's lead word (its first word not a key word), the mean of their best
score into the pair's target and the share of them whose best target it is,
and how many they are; of those sharing its last word, how many and their
mean best over the schema's; and how many columns the target is the best
of. `seven-style` adds them to the relative stage with shift-aware stacking
and bounded leaves; `seven-style2` gives a name of one word no lead word,
after `seven-style` lost CTU's logic-program databases.

| | threshold | SchemaPile test | renamed | Spider | BIRD | CTU | LiveSQLBench | pooled development |
|---|---|---|---|---|---|---|---|---|
| `1s` | 0.837 | 0.736 | 0.515 | 0.869 | 0.830 | 0.609 | 0.185 | 0.700 |
| `seven-restyled`, the rule's | 0.823 | 0.726 | 0.794 | 0.888 | 0.816 | 0.670 | 0.587 | **0.748** |
| `seven-bounded` | 0.818 | 0.734 | 0.809 | **0.891** | 0.812 | 0.654 | 0.619 | 0.745 |
| `seven-relative-bounded` | 0.832 | 0.719 | 0.775 | 0.886 | 0.811 | 0.666 | 0.643 | 0.748 |
| `seven-style` | 0.846 | 0.725 | 0.797 | 0.885 | 0.810 | 0.622 | **0.671** | 0.733 |
| `seven-style2` | 0.862 | 0.725 | 0.806 | 0.879 | 0.818 | 0.618 | 0.645 | 0.728 |
| `seven-bounded` and `seven-style` averaged ([blend.py](../round-1t/blend.py)) | 0.830 | 0.732 | **0.812** | 0.886 | 0.811 | 0.641 | 0.639 | 0.740 |

The rule, unchanged, still chooses `seven-restyled`. The schema's own style
takes LiveSQLBench from `seven-bounded`'s 0.619 to 0.671 (keys found 138 →
171 against `seven-restyled`, drawn false 33 → 40) and costs CTU 0.032;
giving one-word names no lead word costs LiveSQLBench 0.026 and mends
nothing on CTU. What CTU's differences are made of
([mooney-family-scores.txt](data/keys-style-2026-11-08/mooney-family-scores.txt)):

| Mooney_Family, 134 keys | first stage, keys' 10th–90th percentile | second stage, the same | threshold | keys drawn |
|---|---|---|---|---|
| `seven-restyled` | 0.185–0.249 | 0.792–0.846 | 0.823 | 73 |
| `seven-bounded` | 0.142–0.211 | 0.548–0.655 | 0.818 | 0 |
| `seven-style` | 0.142–0.211 | 0.744–0.801 | 0.846 | 0 |

Every key of a logic program's facts scores alike, so each such database is
drawn whole or not at all; three of them (Mooney_Family, SAT, Dunur, 117 keys
`seven-restyled` drew) make most of CTU's difference between the morning's
fits, and `seven-restyled` drew Mooney_Family through its runaway leaves.
`seven-bounded`'s first stage, refitted on the same data by the same
procedure, scores those keys lower too; the fits run on the graphics card,
whose histogram sums need not repeat exactly, a hypothesis not separated
here. CTU's interval is 0.47 to 0.78 for every one of these fits.

**The second GitHub set, once.** The 177 schemas the collector's rule kept
and did not select (every one after the first 80 by SHA-256 of
`owner/name`), written by `github.py parse rest` and frozen by hash at
06:52 UTC before any was read ([frozen-rest-set.json](data/keys-style-2026-11-08/frozen-rest-set.json));
3 908 tables, 4 533 one-column keys (258 composite left out), created
2 July 2025 to 4 November 2026, 141 without a licence
([github-rest-summary.json](data/keys-style-2026-11-08/github-rest-summary.json),
[github-rest-manifest.json](data/keys-style-2026-11-08/github-rest-manifest.json)).
Four models frozen at 08:33 UTC ([frozen-rest.json](data/keys-style-2026-11-08/frozen-rest.json))
and read once ([heldout-github-2025-rest.json](data/keys-style-2026-11-08/heldout-github-2025-rest.json)):

| | P / R / F1 [95 %] | minus `1s` | macro F1 | F1 at 0.5 | keys among candidates |
|---|---|---|---|---|---|
| `1s` | 0.783 / 0.731 / 0.756 [0.687, 0.813] | | 0.750 | 0.720 | 4 447 |
| **`seven-restyled`**, the rule's | 0.762 / 0.771 / **0.767** [0.715, 0.813] | +0.011 [−0.016, +0.042] | 0.761 | 0.718 | 4 467 |
| `seven-bounded` | 0.786 / 0.764 / 0.775 [0.728, 0.816] | +0.019 [−0.010, +0.055] | 0.762 | 0.730 | 4 467 |
| `seven-style` | 0.771 / 0.752 / 0.762 [0.715, 0.806] | +0.006 [−0.033, +0.045] | 0.750 | 0.718 | 4 467 |

Both GitHub sets together, pooled after each was read once and not fixed
before either reading ([heldout-github-both-pooled.json](data/keys-style-2026-11-08/heldout-github-both-pooled.json)):
252 schemas with a one-column key, 6 555 keys; `1s` 0.752 [0.705, 0.795],
`seven-restyled` 0.774 [0.736, 0.807], **+0.022 [−0.002, +0.047]**.

On new schemas from GitHub, which mostly name keys `table_id` as SchemaPile
does, the repaired model gains less than the first reading said: +0.011 on
the larger second set, its interval over zero, and +0.022 over both, the
interval reaching zero. The first set's +0.048 was the higher draw. The
repair's large gains are where the house style differs — LiveSQLBench,
SchemaPile renamed — and where names are `table_id` it costs nothing and
gains a little. The mended leaves read 0.008 above `seven-restyled` on the
second set and the schema's own style 0.005 below it, both within their
intervals: neither is shown to carry to new GitHub schemas, and the schema's
own style stands on LiveSQLBench, development data, alone.

## Limits

- One seed a fit. Round 1s found a seed can move the pooled external F1 by
  0.05 through CTU's `Bench` alone; the night's differences between its own
  fits are mostly smaller than that.
- The night chose among fourteen fits on development data, twice looked
  into; the rule was fixed before, but the more fits, the more one wins by
  chance on development data. Only the GitHub reading guards against that.
- The GitHub set is 80 schemas, real but one file a repository, and SQL
  files on GitHub since mid-2025 are many of them written with language
  models' help; their style leans to `_id`, which the old model reads too.
  BEAVER, the enterprise set, was read only by the first pass's models.
- The teachers are of one family, Claude, at the owner's request.
- The lexicons were written by Haiku from SchemaPile's training words; a word
  of another set they do not hold is read as before.
- `seven-restyled` reads SchemaPile's test 0.010 below `1s`, on the rule's
  line.
- `seven-restyled`, the model the rule chose and the paper's, carries the
  runaway leaves of §7; the mended `seven-bounded` was not the rule's choice
  on development data and is not shown better on the second GitHub set.
- CTU's differences between fits are mostly a few databases drawn whole or
  not at all (§7); a difference there under 0.05 says little.

## What the paper changes

- **The paper's model becomes `seven-restyled`**, and its claim the GitHub
  reading: on 80 schemas created after SchemaPile, F1 0.792 against the old
  model's 0.744, +0.048 [+0.014, +0.084].
- **The mechanism is the contribution**: a model of names learns its
  training set's house style; renaming the training data into drawn styles,
  cues that need no style list, and words split and expanded from lexicons
  that language models wrote once, offline, repair it — LiveSQLBench 0.185 →
  0.587, SchemaPile renamed 0.515 → 0.794 — at no cost on Spider and BIRD.
- **What does not help belongs in it too**: schemas written or renamed by
  language models teach less than drawn styles; a threshold cannot fix a
  shift of style (the bounds above); SchemaPile's test caps any model of
  names near 0.78.
- The model is the same 938 KiB with 170 KB of lexicons, and the Rust reader
  draws exactly the libraries' keys.
- **Since the morning** (§7), the claim on new GitHub schemas is smaller:
  over both sets, 252 schemas, `seven-restyled` 0.774 against 0.752,
  +0.022 [−0.002, +0.047]. The paper's claim is then the shift of style —
  large gains where names are not `table_id`, none lost where they are — and
  the GitHub sets show the second half.

## Environment and reproduction

| | |
|---|---|
| machine | AMD Ryzen 7 7700, 31 GB, NVIDIA GeForce RTX 5070 (driver 610.88), Windows 11 Pro |
| Python | 3.13.15; xgboost 3.4.1, lightgbm 4.7.0, scikit-learn 1.9.1, numpy 2.5.4, scipy 1.18.1; sqlglot for the GitHub set in its own environment |
| container | Docker `rust:1.99.1` (digest `09027f87…0137`), `--cpuset-cpus 3`, WSL2; rustc 1.99.1 |
| SchemaPile; Spider, BIRD, CTU; LiveSQLBench | as round 1s read them (`corpus/round-1b`, `corpus/round-1s/external`, `corpus/round-1s/heldout` at revision `e33469b1`) |
| BEAVER | `target-benchmark/beaver-corpus` at `8b683135f567ed6bef6a6d74cc90dc8de37fc997` |
| GitHub | 80 repositories, each file pinned by commit in [github-manifest.json](data/keys-style-2026-11-08/github-manifest.json); their contents are not published, most carry no licence |
| language models | Claude Haiku 5.5 and Claude Sonnet 5.5 (medium effort) as Claude Code agents for schemas, lexicons and renamings; Claude Opus 5.5 (extra-high effort) as design agents; 8 November 2026 |
| research commit | `590eb79` and this round's files |
| times (UTC) | first fits 22:02–22:50 on 7 November; the night's fits 23:00 to 02:50; GitHub reading 02:58; the morning's fits and scorings 06:57 to 08:32, the second GitHub set written 06:52 and read 08:34–08:36 |

```sh
cd <repo>/lemmesee
P=round-1b/.venv/Scripts/python
$P -I round-1t/teach.py assign corpus/round-1t       # then the agents; then:
$P -I round-1t/teach.py collect corpus/round-1t
$P -I round-1t/rename.py prepare corpus/round-1t     # then the agents; then:
$P -I round-1t/rename.py apply corpus/round-1t
$P -I round-1t/fit.py fit cues restyled cues-restyled cues-restyled-taught six-restyled seven-restyled \
  cues-relative seven-relative eight-restyled eight-relative cues-restyled-llm cues-restyled-more seven-relative-llm
$P -I round-1t/fit.py score 1s cues restyled cues-restyled cues-restyled-taught six-restyled seven-restyled \
  cues-relative seven-relative eight-restyled eight-relative cues-restyled-llm cues-restyled-more seven-relative-llm
$P -I round-1t/fit.py summarize
corpus/round-1t/.venv-parse/Scripts/python -I round-1t/github.py find      # from corpus/round-1t
corpus/round-1t/.venv-parse/Scripts/python -I round-1t/github.py fetch 400
corpus/round-1t/.venv-parse/Scripts/python -I round-1t/github.py parse
$P -I round-1t/fit.py freeze-set "github 2025"
$P -I round-1t/fit.py freeze-final seven-restyled 1s cues-restyled seven-relative
$P -I round-1t/fit.py heldout "github 2025" frozen-final.json
for m in cues-restyled seven-restyled seven-relative; do $P -I round-1t/parity.py pack corpus/round-1t/models/$m; done
(cd round-1t/reader && cargo build --release)
$P -I round-1t/parity.py check corpus/round-1t/models/seven-restyled corpus/round-1b corpus/round-1s/external \
  round-1t/reader/target/release/reader corpus/round-1t/parity/seven-restyled
MSYS_NO_PATHCONV=1 docker run --rm --cpuset-cpus 3 -v <repo>:/src -v lemmesee-cargo:/usr/local/cargo/registry \
  -w /src/lemmesee rust:1.99.1 sh round-1t/reader/bench.sh corpus/round-1s/models/both <1s threshold> \
  corpus/round-1t/models/seven-restyled <threshold> corpus/round-1t/bench7
# the morning (§7)
$P -I round-1t/errors.py seven-restyled ctu "livesqlbench (development)" "schemapile test"
$P -I round-1t/errors.py seven-style ctu "livesqlbench (development)"
$P -I round-1t/fit.py fit seven-bounded seven-style seven-relative-bounded seven-style2
$P -I round-1t/fit.py score seven-bounded seven-style seven-relative-bounded seven-style2
$P -I round-1t/blend.py seven-bounded seven-style
$P -I round-1t/fit.py summarize
corpus/round-1t/.venv-parse/Scripts/python -I round-1t/github.py parse rest
$P -I round-1t/fit.py freeze-rest seven-restyled 1s seven-bounded seven-style
$P -I round-1t/fit.py heldout "github 2025, rest" frozen-rest.json
```

The set's hash was frozen at 06:52 UTC by a few lines run once, written
into `frozen-rest-set.json` beside the fit's other freezes; the pooled
reading of both sets, and the leaf counts, by a few lines over the saved
files whose output is in the data folder.
