# The paper's key model measured whole, 14 November 2026

Round 1w, as [its specification](../round-1w/README.md) set it before
anything was run. The owner chose on 14 November that the key model's paper
reports `eleven--wide`, round 1v's choice
([keys-size-2026-11-10.md](keys-size-2026-11-10.md)), in place of
`nine-mended`, and names it lemmekey there. This report gives `eleven--wide`
the numbers [keys-final-2026-11-09.md](keys-final-2026-11-09.md) gave
`nine-mended`, each through the code that measured it first, with
`nine-mended` read beside it in the same run; a reader in Rust for it; and
seven language models asked again, five of them on every set, the two clean
sets of round 1v among them. No model was fitted and none was chosen: the
model's three files were checked against round 1v's `frozen.json` before
every measurement. Raw output in
[data/keys-paper-model-2026-11-14/](data/keys-paper-model-2026-11-14/).

**Corrected in part by round 1z**
([keys-small-models-2026-11-26.md](keys-small-models-2026-11-26.md)): this
round's reader counted 1,881 of GPT-6 Luna's keys on plain A as naming
nothing, because their target was written with its column. Read with that
forgiven, Luna is at 0.841 on plain A where this report reads 0.766, 0.819
on style 3, 0.855 over the 398 databases and 0.940 on LiveSQLBench. No other
model moves. The figures below are left as this round read them.

Every F1 is over every declared one-column key, a key right when its column
and its table are; every interval a 95 % percentile interval of 1 000
resamples of databases (seed 20261101), a difference resampling both arms
together. Plain A and style 3 are the only clean sets: the tree models'
figures on them are round 1v's one reading, read again by nothing here.
Every other set is development data.

| Question | Answer | What follows |
|---|---|---|
| **Over the 398 external databases?** | `eleven--wide` **0.761** [0.671, 0.839], P 0.861, R 0.682, macro F1 0.841; `nine-mended` 0.755, the difference +0.006 [−0.007, +0.019]; round 1's rules 0.500, +0.261 [+0.213, +0.316] | on the benchmarks the two models are level; the gain of round 1v is on GitHub's schemas |
| **On LiveSQLBench?** | 0.562 [0.407, 0.707] against `nine-mended`'s 0.542, +0.020 [−0.073, +0.104] | the other house style stays where it was |
| **The pair task and related or not?** | Recall at one 0.947 on SchemaPile-Perm's test; on the paper's filtered pairs of Spider, BIRD and CTU 0.953, 0.924 and 0.963 over the whole database; related or not AUC 0.956, 2.7 % of unrelated pairs accepted at 80 % of related | as `nine-mended` (0.941; 0.956, 0.924, 0.957; 0.961, 1.6 %) |
| **Nexus's sets?** | TPC-DS 0.727 at its threshold and 0.802 at the set's best; BIRD's development set 0.785 | TPC-DS below `nine-mended`'s 0.809, BIRD's above its 0.707 |
| **The owner's database A?** | 108 keys drawn against `nine-mended`'s 55: 46 of the 52 declared (39), and 62 no declaration names (16), 40 of them never read | recall 0.885 against 0.750; whether the 62 are links is not known |
| **Does a reader in Rust carry it?** | Exactly: 1 899 817 external and 1 502 214 test pairs, no hashed feature and no second-stage feature differing, the first stage's margins equal, the second's within 1.5e-7, the same 2 669 and 14 779 keys | the model ships without Python |
| **How fast and how large?** | A database's median 3.05, 3.07 ms on one processor in the container, against `nine-mended`'s 2.90, 2.78 and `1s`'s 1.65, 1.66 in the same run; 6.7 MiB resident on a median database; CTU's `Bench`, 678 tables, 102 to 105 s and 1.76 GiB; the trees 960 820 bytes, with the lexicons 1 054 108 | version 10's and 11's cues and the wide stage cost under a tenth; time and memory grow with the square of the tables, and the reader holds every pair at once |
| **Against language models asked today?** | On plain A, clean, 0.757 against 0.823 to 0.860 for five models (GPT-6.1 Terra the best) and 0.766 for GPT-6 Luna, −0.009 [−0.044, +0.032]; on style 3, clean, 0.734 against 0.760 to 0.875 for seven; precision within 0.04 of five on plain A, recall 0.10 to 0.24 below all | the gap is recall, about 0.07 to 0.10 of F1 on plain A; the model is level with the cheapest of them |

## 1. The main table

`eleven--wide`'s keys from the scores round 1v kept, at its threshold
(0.798), against round 1's rules and round 1s's four language models from
their saved replies, as round 1t's `paper.py lms` counts them
([kept.py](../round-1w/kept.py), [main.json](data/keys-paper-model-2026-11-14/main.json)).
No language model was asked for this table. The check the specification
set holds: `nine-mended`, read through the same functions, gives round 1t's
0.755 [0.674, 0.829] and 0.542 [0.380, 0.682] to the last digit.

| 398 external databases, 3 371 keys | P / R / F1 | macro F1 | `eleven--wide` minus this: P | R | F1 |
|---|---|---|---|---|---|
| **`eleven--wide`** | 0.861 / 0.682 / **0.761** [0.671, 0.839] | 0.841 | | | |
| `nine-mended` | 0.844 / 0.684 / 0.755 | 0.826 | +0.017 | −0.002 [−0.027, +0.028] | +0.006 [−0.007, +0.019] |
| round 1's rules | 0.874 / 0.350 / 0.500 | 0.515 | −0.013 | +0.332 [+0.261, +0.416] | **+0.261 [+0.213, +0.316]** |
| Gemini 3 Flash | 0.832 / 0.836 / 0.834 | 0.905 | +0.029 | −0.154 [−0.273, −0.065] | −0.073 [−0.147, −0.015] |
| Gemini 3.5 Flash-Lite | 0.875 / 0.843 / 0.859 | 0.911 | −0.014 | −0.161 [−0.281, −0.070] | −0.098 [−0.163, −0.047] |
| Claude Haiku 5.5 | 0.835 / 0.915 / 0.873 | 0.916 | +0.027 | −0.233 [−0.319, −0.156] | −0.111 [−0.178, −0.055] |
| Gemini 3.8 Flash | 0.886 / 0.894 / 0.890 | 0.934 | −0.025 | −0.212 [−0.303, −0.144] | −0.129 [−0.192, −0.082] |

By source `eleven--wide` reads Spider 0.885 [0.849, 0.916], BIRD 0.829
[0.763, 0.889] and CTU 0.664 [0.526, 0.805] (macro 0.891, 0.784, 0.792);
against `nine-mended` +0.005 [−0.005, +0.016], +0.014 [−0.009, +0.037] and
+0.001 [−0.024, +0.028]. Its distance from Gemini 3.8 Flash is −0.041 on
Spider, −0.089 on BIRD and −0.199 on CTU, every interval below zero.

| LiveSQLBench, 22 databases, 299 keys | P / R / F1 | `eleven--wide` minus this: P | R | F1 |
|---|---|---|---|---|
| **`eleven--wide`** | 0.806 / 0.431 / **0.562** [0.407, 0.707], macro 0.460 | | | |
| `nine-mended` | 0.850 / 0.398 / 0.542 | −0.044 | +0.033 [−0.061, +0.121] | +0.020 [−0.073, +0.104] |
| round 1's rules | no key drawn | | | +0.562 |
| Gemini 3 Flash | 0.928 / 0.866 / 0.896 | −0.122 | −0.435 [−0.580, −0.275] | −0.334 [−0.487, −0.194] |
| Gemini 3.5 Flash-Lite | 0.931 / 0.863 / 0.896 | −0.125 | −0.431 [−0.578, −0.272] | −0.334 [−0.489, −0.194] |
| Claude Haiku 5.5 | 0.927 / 0.890 / 0.908 | −0.121 | −0.458 [−0.620, −0.285] | −0.346 [−0.506, −0.194] |
| Gemini 3.8 Flash | 0.938 / 0.910 / 0.924 | −0.132 | −0.478 [−0.621, −0.327] | −0.362 [−0.507, −0.225] |

**The fresh GitHub set** is development data for `eleven--wide`: version
10's cues were written from its misses. There it reads 0.736 [0.656, 0.814],
P 0.868, R 0.639, macro 0.796, against `nine-mended`'s 0.715, +0.021
[+0.003, +0.042], and against the four models' replies of 9 November −0.113
to −0.175, every interval below zero, recall −0.284 to −0.312
([fresh.json](data/keys-paper-model-2026-11-14/fresh.json)).

**The four GitHub development sets pooled**, 452 schemas and 10 362 keys:
0.823 / 0.772 / 0.797 [0.766, 0.826], macro 0.800, against `nine-mended`'s
0.765, +0.031 [+0.018, +0.046], as round 1v's `choice.json` has it
([github.json](data/keys-paper-model-2026-11-14/github.json)).

## 2. By the number of tables

Round 1v's `sizes.py` with `eleven--wide` in it
([sizes.json](data/keys-paper-model-2026-11-14/sizes.json)); the language
models' figures are round 1s's and round 1t's saved replies. The sets the
four answered, pooled: Spider, BIRD, CTU, LiveSQLBench and the fresh set,
520 databases and 5 686 keys.

| tables | databases | keys | `nine-mended` | **`eleven--wide`** | Gemini 3 Flash | Gemini 3.5 Flash-Lite | Claude Haiku 5.5 | Gemini 3.8 Flash | 3.8 Flash minus `eleven--wide` |
|---|---|---|---|---|---|---|---|---|---|
| under 5 | 209 | 467 | 0.872 | 0.877 | 0.918 | 0.923 | 0.936 | 0.948 | 0.072 |
| 5 to 9 | 159 | 1 061 | 0.823 | 0.851 | 0.899 | 0.903 | 0.906 | 0.933 | 0.082 |
| 10 to 19 | 106 | 1 480 | 0.804 | 0.813 | 0.911 | 0.906 | 0.911 | 0.936 | 0.123 |
| 20 to 39 | 30 | 1 040 | 0.735 | 0.733 | 0.845 | 0.893 | 0.886 | 0.929 | 0.196 |
| 40 to 99 | 12 | 803 | 0.641 | 0.683 | 0.840 | 0.857 | 0.867 | 0.933 | 0.250 |
| 100 or more | 4 | 835 | 0.397 | 0.346 | 0.572 | 0.599 | 0.705 | 0.689 | 0.343 |
| all | 520 | 5 686 | 0.732 | 0.743 | 0.843 | 0.860 | 0.871 | 0.899 | 0.156 [0.118, 0.196] |

The slope of round 1v §1 stands for the paper's model: 0.07 below the best
language model under five tables, 0.25 at 40 to 99. On SchemaPile's test
`eleven--wide` reads 0.881, 0.811, 0.788, 0.719 and 0.641 by the same sizes
(0.761 over all), where `nine-mended` read 0.876 to 0.601.

**What becomes of a declared key**, `eleven--wide`, as shares of the keys:

| SchemaPile's test, tables | targets a column is scored against | its table scored first | found | first, under the threshold | another table first | no candidate |
|---|---|---|---|---|---|---|
| under 5 | 2.3 | 0.95 | 0.90 | 0.05 | 0.02 | 0.03 |
| 5 to 9 | 4.7 | 0.93 | 0.87 | 0.06 | 0.04 | 0.03 |
| 10 to 19 | 9.4 | 0.91 | 0.83 | 0.08 | 0.05 | 0.04 |
| 20 to 39 | 17.2 | 0.88 | 0.73 | 0.15 | 0.08 | 0.04 |
| 40 to 99 | 37.8 | 0.85 | 0.68 | 0.17 | 0.14 | 0.01 |

At 40 to 99 tables the model draws 68 % of the keys where `nine-mended` drew
59 %, and leaves 17 % scored first and under the threshold where it left
25 %. On the fresh set's schemas of a hundred tables or more it finds 29 %
and leaves 45 % so: the decision to draw a key is still what a large schema
loses.

## 3. The recall bounds

Round 1t §3's bounds from the kept scores
([bounds.json](data/keys-paper-model-2026-11-14/bounds.json)): the
candidates' coverage of the declared keys, F1 at the model's threshold, at
the best one threshold for the set, and with each schema at the threshold
best for its own F1, counts then pooled. The candidates are `nine-mended`'s.

| | coverage | at its threshold | the best threshold for the set | each schema's best | `nine-mended`, each schema's best |
|---|---|---|---|---|---|
| SchemaPile's test | 0.968 | 0.761 | 0.761 (0.78) | 0.824 | 0.805 |
| Spider | 0.984 | 0.885 | 0.889 (0.68) | 0.925 | 0.924 |
| BIRD | 0.969 | 0.827 | 0.829 (0.84) | 0.856 | 0.851 |
| CTU | 0.995 | 0.664 | 0.679 (0.36) | 0.722 | 0.777 |
| LiveSQLBench | 0.957 | 0.562 | 0.635 (0.32) | 0.692 | 0.670 |

BIRD's figure at its threshold is 0.827 here and 0.829 in §1. The bound
counts a key by its candidate pair's label and §1 by the names drawn against
the names declared; which keys the two count differently was not looked
into.

## 4. SchemaPile-Perm, the pair task, Nexus and database A

Each through the earlier round's function as it is, the model's pairs built
again ([paper.py](../round-1w/paper.py)): `widen.build` gives version 11's
pairs and `graph.graph_features` the wide stage's 68 features. The open
task's keys over the external databases are the 2 669 of §1.

**SchemaPile-Perm's test**, round 1s's `refit.score`
([perm-eleven--wide.json](data/keys-paper-model-2026-11-14/perm-eleven--wide.json)):

| | `eleven--wide` | `nine-mended` |
|---|---|---|
| over every declared key, P / R / F1 | 0.727 / 0.799 / **0.761** | 0.715 / 0.749 / 0.732 |
| over candidates, model / rules [difference] | 0.773 / 0.417 [+0.316, +0.394] | 0.743 / 0.417 [+0.287, +0.363] |
| the schemas that declare every rule-named column, over candidates | 0.811 | |
| the hard cases (rules 0.544) | 0.883 | 0.875 |
| the first stage alone, over every declared key | 0.746 | |
| the pair task, recall at one / covered | **0.947** / 0.973 | 0.941 / 0.973 |

**The pair task on the external sets**, round 1p's protocol under the
paper's filters
([external-score-eleven--wide.json](data/keys-paper-model-2026-11-14/external-score-eleven--wide.json)):
recall at one over the whole database, Spider **0.953** [0.922, 0.977] (768
pairs), BIRD **0.924** [0.863, 0.964] (381), CTU **0.963** [0.927, 0.984]
(1 267); with two tables only, 0.954, 0.908 and 0.950. **Related or not**,
round 1p's 2 416 related and 2 197 unrelated pairs: AUC **0.956** (Spider
0.973, BIRD 0.951, CTU 0.952), 2.7 % of unrelated pairs accepted at 80 % of
related, against `nine-mended`'s 0.961 and 1.6 % and round 1p's T5's 0.798.

**Nexus's sets**, round 1p's `nexus.py`
([nexus-eleven.json](data/keys-paper-model-2026-11-14/nexus-eleven.json)):

| | tables, in SchemaPile-Perm | keys | `eleven--wide` at its threshold | at the set's best | `nine-mended` | Nexus's XGBoost |
|---|---|---|---|---|---|---|
| TPC-DS | 24, 0 | 102 | 0.727 (P 0.952, R 0.588) | 0.802 | 0.809 / 0.840 | 0.44 |
| BIRD's development set | 75, 5 | 104 | **0.785** (P 0.890, R 0.702) | 0.785 | 0.707 / 0.733 | 0.10 |
| TPC-H | 8, 8 | 7 | 1.000 | 1.000 | 0.933 / 1.000 | 0.35 |

TPC-H is in SchemaPile-Perm whole, so its figure is memory. On TPC-DS the
model is more careful than `nine-mended` at its own threshold and reaches
0.802 at the set's best.

**The owner's database A**, counts only
([owner-A.json](data/keys-paper-model-2026-11-14/owner-A.json)): 63 tables,
52 declared one-column keys. `eleven--wide` draws 108 keys, 46 of them
declared (recall 0.885; precision counting declarations only 0.426, F1
0.575), where `nine-mended` drew 55 and 39. Of the 62 no declaration names,
22 are among the 36 that round 1q's reading of the app's code judged, and 40
were never read. Round 1q kept that reading's counts, not its verdicts key
by key, so how many of the 62 are links is not given here. A hypothesis,
not measured: the 46 more undeclared keys are columns named for someone,
which version 11's cues were written to draw. A reading of the 62 against
the app's code would say.

## 5. The reader in Rust

Round 1t's reader copied to [round-1w/reader](../round-1w/reader) and
extended: version 10's and 11's cues ([ten.rs](../round-1w/reader/src/ten.rs)),
the six features of round 1t's `stack.py` and the 43 of round 1v's `wide.py`
([wide.rs](../round-1w/reader/src/wide.rs)). `keys.bin`'s flags byte takes
16 for version 10, 32 for version 11 and 64 for the wide stage beside the
earlier 1, 2, 4 and 8: `nine-mended` is 9, `eleven--wide` 125. No lexicon
was added: the nine words for "by" are 36 bytes of the code, the rest reads
round 1t's three files.

**Parity** against XGBoost and LightGBM over every pair
([parity.py](../round-1w/parity.py),
[parity-eleven--wide.json](data/keys-paper-model-2026-11-14/parity-eleven--wide.json)),
on Windows, both from the same schemas:

| | pairs | hashed features differing | first stage, largest margin difference | second-stage features differing, of 68 a pair | second stage, largest margin difference; pairs over 1e-4 | keys drawn, the same |
|---|---|---|---|---|---|---|
| external, 398 databases | 1 899 817 | 0 | 0 | 0 | 1.4e-7; 0 | 2 669 |
| SchemaPile-Perm's test | 1 502 214 | 0 | 0 | 0 | 1.4e-7; 0 | 14 779 |

The second stage's difference is its leaves packed as 32-bit floats, as
`nine-mended`'s 1.5e-7 was. `nine-mended`, packed and checked through the
same code on Spider and 200 test schemas, still reads exact, and its
`keys.bin` is round 1t's byte for byte.

**Time and memory**, in one run of the Linux container (`rust:1.99.1`, one
CPU, round 1s's settings: three warm runs and the median of 21, a schema
slower than a second of 5), the three readers in turn, twice, over round
1p's 398 databases ([bench-*.json](data/keys-paper-model-2026-11-14/), 11:01
to 12:06 UTC):

| | round 1s's reader, `1s` | round 1t's, `nine-mended` | round 1w's, `eleven--wide` |
|---|---|---|---|
| a database, median of 398, two passes | 1.65, 1.66 ms | 2.90, 2.78 ms | **3.05, 3.07 ms** |
| the same, 90th percentile | 17.4, 18.0 ms | 32.2, 29.8 ms | 33.2, 30.6 ms |
| the same, 99th percentile | 151, 164 ms | 429, 427 ms | 455, 427 ms |
| a pair, a database's median | 50.8, 53.9 µs | 61.7, 62.8 µs | 66.8, 65.6 µs |
| over `1s`'s in the same run, a database's median ratio | | | 1.75, 1.64 |
| over `nine-mended`'s, the same | | | 1.08, 1.04 |
| CTU's `Bench`: pairs; seconds | 622 512; 27.7, 29.2 s | 1 717 591; 94.6, 98.3 s | 1 717 591; 105.3, 102.0 s |
| resident at most, the model loaded | 4.3 MiB | 4.5 MiB | 4.4 MiB |
| resident at most, CTU's `Walmart` (a median database) | 5.0 MiB | 6.7 MiB | 6.7 MiB |
| resident at most, CTU's `Bench` | 298 MiB | 916 MiB | **1 755 MiB** |
| binary | 888 160 bytes | 1 250 680 bytes | 1 357 680 bytes |

The new cues and the wide stage cost 4 to 8 % of a database's time beside
`nine-mended`, and nothing in memory on a median database. What makes a
large schema slow is the number of pairs, which grows with the square of its
tables: `Bench`'s 678 tables give 1.7 million pairs and 102 to 105 seconds
on one processor, through the reference path that names each feature as a
string. A fast path for versions 6 to 11 is still to write, and the pairs
are scored one after another. `Bench` also takes 1.76 GiB resident, nearly
twice `nine-mended`'s 916 MiB: the reader holds every pair's features until
both stages have scored them, and the wide stage reads 68 a pair where the
earlier one read 19. That is a reader's choice to mend, not the model's
size.

A measurement beside other work: a script asked two services for the state
of the language models' batches every 150 seconds while the container ran,
a process of under a second each time on other processors.

**Its size** ([pack.json](data/keys-paper-model-2026-11-14/pack.json), as
`parity.py pack` writes it): `keys.bin` 960 820 bytes, 829 trees of 63 leaves at most and 400
of 31, 51 398 and 12 000 inner nodes, 52 227 and 12 400 leaves, as
`nine-mended`'s to the byte count. With round 1t's three lexicons (93 288
bytes) the model is 1 054 108 bytes, 1.005 MiB.

## 6. `see`'s questions through its keys

Measured in round 1v and not again
([questions.json](data/keys-size-2026-11-10/questions.json)); given here
whole because the paper carries it. The share of a database's questions that
join through declared keys still asked with the model's keys in place of the
declarations, a database's mean; the share of the model run's questions that
go through a key no declaration names; and how many of the declared run's
five the ranker keeps.

| | databases | joining questions, declared | kept, rules | kept, `eleven--wide` | through an undeclared key: rules / `eleven--wide` | the five kept: rules / `eleven--wide` |
|---|---|---|---|---|---|---|
| Spider | 204 | 19 047 | 0.687 | **0.884** [0.850, 0.917] | 6.6 % / 6.2 % | 0.835 / 0.912 |
| BIRD | 76 | 32 355 | 0.654 | **0.825** [0.751, 0.883] | 8.8 % / 11.0 % | 0.844 / 0.844 |
| CTU | 118 | 170 542 | 0.615 | **0.802** [0.746, 0.859] | 15.7 % / 10.0 % | 0.748 / 0.824 |
| LiveSQLBench | 22 | 20 508 | 0.007 | **0.365** [0.221, 0.526] | 2.4 % / 9.9 % | 0.636 / 0.709 |

## 7. The language models, asked again

Seven language models were asked for every key of a schema, one reply a
schema, with round 1p's whole-schema prompt unchanged
([batch.py](../round-1w/batch.py),
[batch_anthropic.py](../round-1w/batch_anthropic.py),
[lms.py](../round-1w/lms.py), [lms.json](data/keys-paper-model-2026-11-14/lms.json)).
Five answered every set: Claude Sonnet 5.5, GPT-6.1 Terra, GPT-6 Luna,
Gemini 3.5 Flash-Lite and DeepSeek V4.1 Flash. Claude Haiku 5.5 answered the
two clean sets and Gemini 3.8 Flash style 3 alone. None had seen plain A or
style 3 through this repository; what their makers trained them on is not
known.

**This is not the protocol the specification named.** It asked for round
1s's four models through round 1s's road. The owner dropped that proxy on
14 November and chose these models and their batch interfaces in its place,
so §1's and §2's comparison with round 1s's saved replies stays as it is and
this section stands beside it, not in its place.

On plain A, the larger clean set, `eleven--wide` reads 0.757 and five of the
six models asked read 0.823 to 0.860, the model 0.066 to 0.103 below them
with every interval clear of zero. GPT-6 Luna reads 0.766, and the model is
level with it, −0.009 [−0.044, +0.032]. The distance is recall: the model's
precision is within 0.04 of five of them and 0.078 above Luna's, and its
recall is 0.104 to 0.236 below every one. On style 3 the model is below all
seven by 0.026 to 0.141, Gemini 3.5 Flash-Lite's and Luna's intervals
holding zero. On
LiveSQLBench four models read 0.900 to 0.932 against the model's 0.562, and
Luna 0.572.

In each table a model's F1 carries its interval, the model minus it is on
the same resamples, and the column of 40 tables or more is the language
model's F1 there with the model minus it. An unreadable reply counts as no
key drawn; a key naming no table or column of the schema counts as a false
one.

**Plain A (clean)**, 718 databases, 16 399 keys:

| | P | R | F1 | macro F1 | `eleven--wide` minus it, F1 | in recall | 40 tables or more, F1 | unreadable | keys naming nothing |
|---|---|---|---|---|---|---|---|---|---|
| gpt-6.1-terra | 0.767 | 0.977 | 0.860 [0.835, 0.883] | 0.900 | −0.103 [−0.122, −0.082] | −0.236 [−0.266, −0.204] | 0.806, −0.155 [−0.199, −0.110] | 0 | 1 |
| claude-sonnet-5-5 | 0.797 | 0.912 | 0.850 [0.810, 0.884] | 0.896 | −0.093 [−0.120, −0.063] | −0.171 [−0.234, −0.097] | 0.766, −0.115 [−0.184, −0.035] | 5 | 0 |
| claude-haiku-5-5 | 0.784 | 0.917 | 0.845 [0.808, 0.875] | 0.886 | −0.088 [−0.110, −0.063] | −0.176 [−0.239, −0.100] | 0.772, −0.121 [−0.183, −0.053] | 2 | 7 |
| gemini-3.5-flash-lite | 0.742 | 0.932 | 0.826 [0.789, 0.856] | 0.868 | −0.069 [−0.093, −0.042] | −0.191 [−0.242, −0.128] | 0.755, −0.104 [−0.162, −0.040] | 1 | 340 |
| deepseek-v4.1-flash-1017 | 0.801 | 0.845 | 0.823 [0.781, 0.859] | 0.873 | −0.066 [−0.090, −0.037] | −0.104 [−0.172, −0.029] | 0.696, −0.045 [−0.123, +0.040] | 19 | 8 |
| gpt-6-luna | 0.695 | 0.853 | 0.766 [0.718, 0.811] | 0.839 | −0.009 [−0.044, +0.032] | −0.112 [−0.170, −0.044] | 0.664, −0.013 [−0.094, +0.082] | 0 | 1892 |
| `eleven--wide` | 0.774 | 0.741 | **0.757** [0.727, 0.787] | 0.803 | | | | | |
| `nine-mended` | 0.755 | 0.656 | **0.702** [0.658, 0.744] | 0.756 | | | | | |

**Style 3 (clean)**, 100 databases, 1 762 keys:

| | P | R | F1 | macro F1 | `eleven--wide` minus it, F1 | in recall | 40 tables or more, F1 | unreadable | keys naming nothing |
|---|---|---|---|---|---|---|---|---|---|
| gemini-3.8-flash | 0.853 | 0.898 | 0.875 [0.818, 0.937] | 0.914 | −0.141 [−0.219, −0.075] | −0.228 [−0.335, −0.120] | 0.758, −0.297 [−0.463, −0.122] | 0 | 0 |
| claude-sonnet-5-5 | 0.810 | 0.920 | 0.862 [0.808, 0.921] | 0.906 | −0.128 [−0.215, −0.049] | −0.250 [−0.364, −0.131] | 0.753, −0.292 [−0.480, −0.093] | 0 | 0 |
| gpt-6.1-terra | 0.763 | 0.960 | 0.850 [0.802, 0.907] | 0.899 | −0.117 [−0.212, −0.027] | −0.289 [−0.433, −0.142] | 0.752, −0.292 [−0.498, −0.025] | 0 | 0 |
| deepseek-v4.1-flash-1017 | 0.778 | 0.874 | 0.823 [0.755, 0.893] | 0.868 | −0.090 [−0.185, −0.001] | −0.204 [−0.379, −0.028] | 0.662, −0.201 [−0.436, +0.167] | 3 | 1 |
| claude-haiku-5-5 | 0.767 | 0.881 | 0.820 [0.755, 0.895] | 0.874 | −0.086 [−0.159, −0.015] | −0.211 [−0.314, −0.107] | 0.690, −0.229 [−0.378, −0.079] | 0 | 0 |
| gemini-3.5-flash-lite | 0.761 | 0.874 | 0.814 [0.712, 0.907] | 0.885 | −0.080 [−0.171, +0.014] | −0.204 [−0.298, −0.105] | 0.655, −0.194 [−0.388, +0.039] | 0 | 15 |
| gpt-6-luna | 0.695 | 0.838 | 0.760 [0.659, 0.853] | 0.835 | −0.026 [−0.146, +0.103] | −0.168 [−0.311, −0.002] | 0.585, −0.124 [−0.387, +0.256] | 0 | 156 |
| `eleven--wide` | 0.810 | 0.670 | **0.734** [0.621, 0.841] | 0.810 | | | | | |
| `nine-mended` | 0.807 | 0.654 | **0.722** [0.615, 0.829] | 0.794 | | | | | |

**The 398 external databases**, 398 databases, 3 371 keys:

| | P | R | F1 | macro F1 | `eleven--wide` minus it, F1 | in recall | 40 tables or more, F1 | unreadable | keys naming nothing |
|---|---|---|---|---|---|---|---|---|---|
| claude-sonnet-5-5 | 0.858 | 0.916 | 0.886 [0.819, 0.937] | 0.931 | −0.125 [−0.183, −0.073] | −0.234 [−0.318, −0.149] | 0.792, −0.310 [−0.589, −0.169] | 1 | 1 |
| gpt-6.1-terra | 0.835 | 0.935 | 0.882 [0.830, 0.928] | 0.928 | −0.121 [−0.186, −0.061] | −0.253 [−0.345, −0.159] | 0.822, −0.339 [−0.602, −0.162] | 0 | 1 |
| deepseek-v4.1-flash-1017 | 0.861 | 0.830 | 0.845 [0.738, 0.916] | 0.908 | −0.084 [−0.148, −0.029] | −0.148 [−0.263, −0.053] | 0.678, −0.195 [−0.568, +0.097] | 5 | 10 |
| gemini-3.5-flash-lite | 0.846 | 0.842 | 0.844 [0.723, 0.922] | 0.909 | −0.083 [−0.152, −0.029] | −0.160 [−0.268, −0.075] | 0.630, −0.148 [−0.586, +0.136] | 0 | 2 |
| gpt-6-luna | 0.776 | 0.862 | 0.817 [0.740, 0.886] | 0.875 | −0.056 [−0.123, +0.013] | −0.180 [−0.264, −0.097] | 0.741, −0.259 [−0.584, −0.134] | 0 | 174 |
| `eleven--wide` | 0.861 | 0.682 | **0.761** [0.670, 0.840] | 0.841 | | | | | |
| `nine-mended` | 0.844 | 0.684 | **0.755** [0.674, 0.830] | 0.826 | | | | | |

**LiveSQLBench**, 22 databases, 299 keys:

| | P | R | F1 | macro F1 | `eleven--wide` minus it, F1 | in recall | 40 tables or more, F1 | unreadable | keys naming nothing |
|---|---|---|---|---|---|---|---|---|---|
| gpt-6.1-terra | 0.919 | 0.946 | 0.932 [0.877, 0.971] | 0.917 | −0.370 [−0.529, −0.216] | −0.515 [−0.657, −0.352] |  | 0 | 0 |
| claude-sonnet-5-5 | 0.930 | 0.926 | 0.928 [0.875, 0.966] | 0.900 | −0.366 [−0.530, −0.213] | −0.495 [−0.650, −0.318] |  | 0 | 0 |
| deepseek-v4.1-flash-1017 | 0.927 | 0.896 | 0.912 [0.864, 0.949] | 0.874 | −0.349 [−0.508, −0.209] | −0.465 [−0.607, −0.304] |  | 0 | 0 |
| gemini-3.5-flash-lite | 0.926 | 0.876 | 0.900 [0.828, 0.952] | 0.869 | −0.338 [−0.487, −0.205] | −0.445 [−0.586, −0.294] |  | 0 | 0 |
| gpt-6-luna | 0.572 | 0.572 | 0.572 [0.352, 0.770] | 0.614 | −0.010 [−0.282, +0.266] | −0.140 [−0.406, +0.151] |  | 0 | 115 |
| `eleven--wide` | 0.806 | 0.431 | **0.562** [0.407, 0.707] | 0.460 | | | | | |
| `nine-mended` | 0.850 | 0.398 | **0.542** [0.380, 0.682] | 0.430 | | | | | |

**The fresh GitHub set**, 100 databases, 2 016 keys:

| | P | R | F1 | macro F1 | `eleven--wide` minus it, F1 | in recall | 40 tables or more, F1 | unreadable | keys naming nothing |
|---|---|---|---|---|---|---|---|---|---|
| claude-sonnet-5-5 | 0.799 | 0.977 | 0.879 [0.807, 0.931] | 0.920 | −0.143 [−0.227, −0.034] | −0.337 [−0.446, −0.217] | 0.806, −0.229 [−0.443, +0.016] | 0 | 0 |
| gpt-6.1-terra | 0.790 | 0.970 | 0.871 [0.797, 0.925] | 0.904 | −0.135 [−0.218, −0.032] | −0.331 [−0.440, −0.209] | 0.793, −0.216 [−0.442, +0.024] | 1 | 0 |
| gemini-3.5-flash-lite | 0.781 | 0.921 | 0.845 [0.777, 0.900] | 0.882 | −0.109 [−0.197, −0.006] | −0.282 [−0.399, −0.154] | 0.787, −0.210 [−0.428, +0.025] | 0 | 7 |
| deepseek-v4.1-flash-1017 | 0.861 | 0.790 | 0.824 [0.709, 0.915] | 0.879 | −0.088 [−0.196, +0.033] | −0.151 [−0.357, +0.045] | 0.652, −0.075 [−0.418, +0.335] | 5 | 0 |
| gpt-6-luna | 0.722 | 0.903 | 0.802 [0.708, 0.875] | 0.858 | −0.066 [−0.171, +0.069] | −0.263 [−0.397, −0.103] | 0.718, −0.141 [−0.400, +0.170] | 0 | 156 |
| `eleven--wide` | 0.868 | 0.639 | **0.736** [0.658, 0.814] | 0.796 | | | | | |
| `nine-mended` | 0.853 | 0.616 | **0.715** [0.637, 0.791] | 0.775 | | | | | |

**How they were asked.** Every request carried 32 768 output tokens at
most, temperature 0 where the model's page lists the parameter, and the
model's default reasoning. The replies are kept whole with their usage in
`corpus/round-1w/lm/llm/`; the prompts' hashes are in
[prompts-sha256.txt](data/keys-paper-model-2026-11-14/prompts-sha256.txt)
and every batch's record in `batches-openrouter/` and `batches-anthropic/`
beside it.

| model | through | replies | a request at a time | input tokens | output tokens | of them reasoning | cut at 32 768 | temperature | charged |
|---|---|---|---|---|---|---|---|---|---|
| Claude Sonnet 5.5 | Anthropic's Message Batches | 1 338 | 0 | 3 373 933 | 1 115 151 | 215 034 | 2 | default | $8.95 |
| Claude Haiku 5.5 | Anthropic's Message Batches | 818 | 0 | 2 610 710 | 1 427 935 | 718 833 | 2 | default | $0.49 |
| GPT-6.1 Terra | OpenRouter, served by OpenAI | 1 338 | 816 | 2 194 374 | 775 648 | 262 592 | 0 | default | $5.20 |
| GPT-6 Luna | OpenRouter's batches, OpenAI | 1 338 | 0 | 2 194 374 | 1 164 185 | 668 534 | 0 | default | $0.40 |
| Gemini 3.5 Flash-Lite | OpenRouter, Google | 1 338 | 518 | 2 396 069 | 811 302 | 0 | 1 | 0 | $2.12 |
| Gemini 3.8 Flash | OpenRouter's batches, Google | 100 | 0 | 181 810 | 343 591 | 293 711 | 0 | 0 | $0.71 |
| DeepSeek V4.1 Flash | OpenRouter's batches, Sail Research | 1 338 | 0 | 2 324 305 | 9 346 828 | 8 854 936 | 32 | 0 | $1.52 |

Anthropic charged $9.44 at half its list price, and OpenRouter $9.95 of the
$14 the owner set: $4.42 for 4 118 replies by batch and $5.53 for 1 334
asked a request at a time (298 of Terra's on the external databases, 518 of
Terra's and 518 of Flash-Lite's on plain A). The same prompt, model and
parameters went either way; whether a model answers a batch as it answers a
request was not measured.

Thirty-seven replies stopped at 32 768 output tokens, 32 of them DeepSeek's,
whose reasoning took 95 % of its output. A reply cut there is unreadable and
counts as no key drawn, so DeepSeek's recall on plain A (19 replies cut) is
its recall under this limit.

## Limits

- **Every tree figure but plain A's and style 3's is on development data**,
  and those two are round 1v's one reading. The fresh GitHub set is the set
  version 10's cues were written from.
- **The language models were not asked as the specification said**, and not
  one way: two interfaces, and 1 334 of OpenRouter's 5 452 replies a request
  at a time. Each model was asked once;
  a second asking would differ by an amount nobody measured here.
- **Claude Haiku 5.5 and Gemini 3.8 Flash did not answer every set**, so
  the five sets do not compare the same seven models.
- **A reply cut at the limit counts against its model**, 37 in all. Raising
  the limit was not tried.
- **F1 over declared keys counts a key nobody declared as false**, for the
  tree model and the language models alike; round 1v's judge put four in
  five of the model's false keys on plain A among real links.
- **The requests held the owner's production balance twice.** OpenRouter
  reserves a batch's worst case at submit, every reply at 32 768 tokens;
  batches of 250 prompts reserved about $20 each against cents charged, and
  ordinary requests on the same key were refused until the reserves were
  released, the second time from about 15:08 to 15:44 UTC. Nothing was
  measured wrong by it; `batch.py feed` now sums its own open reserves.
- **§1's interval over the 398 databases is [0.671, 0.839] and §7's
  [0.670, 0.840]** for the same keys and the same seed, through `kept.py`
  and `lms.py`; why they differ in the third digit was not looked into.
- **The reader holds every pair of a schema at once**: 1.76 GiB and over
  100 s on 678 tables. Nothing here tried to bound it.
- **Database A's 62 undeclared keys were judged by nobody.**
- **The timings ran while this round's batches were polled** from the host,
  as §5 says.
- **Nothing here is committed**: the round's code ran from the working tree
  on `49f394f`.

## What follows

- **For the paper**: `eleven--wide` is its model. Its clean figures are
  plain A's 0.757 and style 3's 0.734; beside them stand six and seven
  language models asked on the same schemas. What it can claim is a megabyte
  and three milliseconds a database against 0.07 to 0.10 of F1 on plain A,
  all of it recall, and nothing against the cheapest of them.
- **Large schemas and recall** are still where the model loses: 0.705 at 40
  to 99 tables and 0.568 at 100 or more on plain A, against Terra's 0.845
  and 0.750.
- **The reader** needs its pairs bounded before a schema of hundreds of
  tables is asked of it in the product; that is product work, not a round.
- **Plain B stays clean**, unread, for the next model.

## Environment and reproduction

| | |
|---|---|
| machine | AMD Ryzen 7 7700, 31 GB, Windows 11 Pro; the reader's timings in `rust:1.99.1` on one processor, [environment.txt](data/keys-paper-model-2026-11-14/environment.txt) |
| Python | 3.13.15; xgboost 3.4.1, lightgbm 4.7.0, scikit-learn 1.9.1, numpy 2.5.4, scipy 1.18.1 |
| model | `eleven--wide` as round 1v froze it, checked against `corpus/round-1v/frozen.json` before every measurement; threshold 0.798 |
| language models | `claude-sonnet-5-5` and `claude-haiku-5-5` through Anthropic's Message Batches; `openai/gpt-6.1-terra`, `openai/gpt-6-luna`, `google/gemini-3.5-flash-lite`, `google/gemini-3.8-flash` and `deepseek/deepseek-v4.1-flash-1017` through OpenRouter; all on 14 November 2026, 10:58 to 18:33 UTC; Claude Fable 5.2 as the round's agent, which wrote the code and this report |
| research commit | `49f394f` with the round's files in the working tree, uncommitted |
| seeds | 20261101 for the resamples; no model fitted |

```sh
cd <repo>/lemmesee
P=round-1b/.venv/Scripts/python
$P -I round-1w/kept.py corpus/round-1w/kept
$P -I round-1w/paper.py model
$P -I round-1w/paper.py perm
$P -I round-1w/paper.py nexus
$P -I round-1w/paper.py owner
$P -I round-1w/parity.py pack <models>
$P -I round-1w/parity.py check <models> <round 1b out> <external out> <reader> <out>
MSYS_NO_PATHCONV=1 docker run --rm --cpuset-cpus 3 -v <repo>:/src -v lemmesee-cargo:/usr/local/cargo/registry \
  -w /src/lemmesee rust:1.99.1 sh round-1w/reader/bench.sh <1s models> <1s threshold> \
  <1t models> <1t threshold> <1w models> <1w threshold> <bench dir>
$P -I round-1w/batch.py prompts
OPENROUTER_API_KEY=... $P -I round-1w/batch.py feed <dollars held at most> <model> <set> <chunk>
OPENROUTER_API_KEY=... $P -I round-1w/batch.py sync <model> <set> <in flight>
OPENROUTER_API_KEY=... $P -I round-1w/batch.py collect
ANTHROPIC_API_KEY=... round-1s/.venv/Scripts/python -I round-1w/batch_anthropic.py submit <model> <set>
ANTHROPIC_API_KEY=... round-1s/.venv/Scripts/python -I round-1w/batch_anthropic.py collect
$P -I round-1w/lms.py reports/data/keys-paper-model-2026-11-14/lms.json
```
