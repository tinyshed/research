# The paper's final key model measured, 9 November 2026

Round 1t continued once more. The specification made `nine-mended` the
paper's model at 14:06 UTC on 9 November, before any of its numbers
([keys-mended-2026-11-09.md](keys-mended-2026-11-09.md)); this report gives
every other number the paper reports for it, each through the code that
measured it first for the earlier models: the reader in Rust, its size,
see's questions (round 1u), the GitHub sets read again with the fixed
collector, SchemaPile-Perm and the pair task (round 1s), Nexus's sets (round
1p), the recall bounds (round 1t §3), the owner's database A (round 1q), the
main table against round 1s's four language models, and those models asked
on the fresh set. No model is chosen here and none was fitted. Raw output in
[data/keys-final-2026-11-09/](data/keys-final-2026-11-09/).

Every F1 is over every declared one-column key unless a row says otherwise;
every interval a 95 % percentile interval of 1 000 resamples of databases
(seed 20261101), a difference resampling both arms together.

| Question | Answer |
|---|---|
| **Over the 398 external databases (Spider, BIRD, CTU)?** | `nine-mended` **0.755** [0.674, 0.829], P 0.844, R 0.684, macro F1 0.826 (round 1s's refit 0.731); round 1's rules 0.500, +0.256 [+0.209, +0.313] |
| **Against the four language models there?** | They read 0.834–0.890; `nine-mended` minus each −0.079 to −0.135, every interval below zero; the gap is recall, −0.15 to −0.23, precision −0.04 to +0.01 |
| **On the fresh clean GitHub set, against them?** | The four 0.849–0.911 (Haiku 0.864); `nine-mended` minus each −0.134 to −0.196, every interval below zero, recall −0.31 to −0.34; `1s` minus each −0.111 to −0.173 |
| **On LiveSQLBench?** | 0.542 [0.380, 0.682] against their 0.896–0.924; round 1's rules draw no key |
| **The four GitHub readings as they now stand?** | 452 schemas, 10 362 keys: `nine-mended` 0.765 [0.732, 0.795] against `1s`'s 0.711, +0.055 [+0.021, +0.095], macro F1 +0.093 [+0.068, +0.118] — carried by the three readings corrected after the fact; on the fresh set, read once, `1s` is ahead by 0.023 |
| **Does the reader in Rust carry it?** | Exactly: 1 899 817 external and 1 502 214 test pairs, no hashed feature differing, the first stage's margins equal, the second's within 1.5e-7, the same 2 731 and 14 104 keys; a database's median 2.66–2.79 ms against `1s`'s 1.57–1.60 in the same container run, through version 6's reference path |
| **How large is it?** | The trees 960 820 bytes, the three lexicons 93 288 (8.9 %), 1 054 108 in all; with `gzip -9` 619 676 |
| **see's questions through its keys?** | A database keeps 0.870 / 0.792 / 0.790 / 0.270 of its joining questions on Spider, BIRD, CTU and LiveSQLBench, against the rules' 0.687 / 0.654 / 0.615 / 0.007 |
| **The pair task, related or not, Nexus?** | Recall at one 0.941 on SchemaPile-Perm's test (round 1s 0.905); related or not AUC 0.961 (round 1s 0.931, T5 0.798); TPC-DS 0.809 and BIRD's development set 0.707 against Nexus's XGBoost 0.44 and 0.10 |

## 1. The reader in Rust

Version 9's cues added to round 1t's reader ([six.rs](../round-1t/reader/src/six.rs):
`roles`, `role_tokens` and `carries`, reading `roles.json` compiled into the
binary as the other lexicons are; `keys.bin`'s flags byte 8 for version 9),
and `nine-mended` packed with them (flags 9: version 7 and version 9).
**Parity** against XGBoost and LightGBM, as round 1t §6
([parity-nine-mended.json](data/keys-final-2026-11-09/parity-nine-mended.json)):

| | pairs | hashed features differing | first stage, largest margin difference | second stage, largest; pairs over 1e-4 | keys drawn, the same |
|---|---|---|---|---|---|
| `nine-mended`, external | 1 899 817 | 0 | 0 | 1.4e-7; 0 | 2 731 |
| `nine-mended`, SchemaPile-Perm's test | 1 502 214 | 0 | 0 | 1.5e-7; 0 | 14 104 |

The second stage's differences are a thousand times smaller than
`seven-restyled`'s 1.4e-4: its leaves are bounded.

**Time and memory**, in one run of the Linux container (`rust:1.99.1`, one
CPU, round 1s's settings — three warm runs and the median of 21, a schema
slower than a second of 5 — round 1s's reader with `1s` and round 1t's with
`nine-mended` alternating twice over round 1p's 398 databases;
[rust-v9-*.json](data/keys-final-2026-11-09/), 15:59–16:34 UTC):

| | round 1s's reader, `1s` | round 1t's, `nine-mended` |
|---|---|---|
| a database, median of 398, two passes | 1.57, 1.60 ms | **2.66, 2.79 ms** |
| the same, 90th percentile | 17.1, 16.5 ms | 28.3, 30.7 ms |
| over `1s`'s in the same run, a database's median ratio | | 1.58, 1.62 |
| CTU's `Bench`: pairs; seconds; resident at most | 622 512; 26.4, 27.5 s; 298 MiB | 1 717 591; 90.7, 91.9 s; 916 MiB |
| resident, CTU's `Walmart` (a median database), at most | 4.9 MiB | 6.5 MiB |
| resident, the model loaded | 3.7 MiB | 3.8 MiB |
| binary | 888 160 bytes | 1 250 680 bytes (the lexicons inside) |

Version 9's cues cost nothing measurable beside `seven-restyled`'s (round
1t §6: 2.56, 2.59 ms; `Bench` 83–84 s): the reader is as slow as version 6's
reference path makes it, half as slow again as `1s`'s, its candidates 2.8
times as many on `Bench`. A fast path for versions 6 to 9 is still to write.
`1s`'s own times here are 0.1 ms above round 1t's run of it: the two runs'
machine, not the readers.

## 2. Its size

([size.json](data/keys-final-2026-11-09/size.json); the lexicons as the
repository holds them, LF line ends; `gzip -9 -n`, gzip 1.14, each file
alone; zstd was not installed)

| file | bytes | gzip -9 |
|---|---|---|
| `keys.bin`, the two stages' trees | 960 820 | 583 398 |
| `vocab.txt`, the split words' vocabulary | 57 176 | 25 159 |
| `abbrev.json`, 999 shortenings | 29 406 | 8 564 |
| `roles.json`, version 9's roles | 6 706 | 2 555 |
| **in all** | **1 054 108** | **619 676** |

The trees alone are under a megabyte (0.96 MB, 0.92 MiB); with the lexicons,
which are 8.9 % of it, the model is 1.05 MB, 1.005 MiB, and 0.62 MB
compressed. `related.json` (67 575 bytes) is version 8's and not read. The
libraries' own files, which Python reads, are 4 074 349 bytes (XGBoost) and
1 417 619 (LightGBM's text).

## 3. see's questions through its keys

Round 1u's [questions.py](../round-1u/questions.py), `--models nine-mended
seven-restyled`, both in one run over the same snapshots
([questions-nine-mended+seven-restyled.json](data/keys-final-2026-11-09/questions-nine-mended+seven-restyled.json));
the share of a database's questions that join through declared keys still
asked, a database's mean:

| | rules | `nine-mended` | `seven-restyled` | the model's questions through an undeclared key: `nine-mended` | the declared five kept: `nine-mended` |
|---|---|---|---|---|---|
| Spider | 0.687 | **0.870** [0.834, 0.904] | 0.885 | 5.5 % | 0.909 |
| BIRD | 0.654 | **0.792** [0.713, 0.860] | 0.808 | 14.0 % | 0.830 |
| CTU | 0.615 | **0.790** [0.727, 0.850] | 0.814 | 8.3 % | 0.817 |
| LiveSQLBench | 0.007 | **0.270** [0.176, 0.384] | 0.398 | 7.9 % | 0.664 |

`seven-restyled`'s numbers are round 1u's to the third digit: the run
repeats. `nine-mended` keeps a little fewer, as its keys are a little fewer
(its LiveSQLBench 0.542 against 0.587), and goes through undeclared keys less
often on CTU, 8.3 % against 13.1 %.

## 4. The GitHub sets read again, and pooled

The first two GitHub sets (the 80 and the 177) rewritten by the fixed
collector (`github.py parse typed` and `parse rest-typed`: the primary keys
`fill_keys` reads and the training's type families; the same schemas and
keys), and the style set's fixed version read again, by all five models,
frozen in [frozen-corrected.json](data/keys-final-2026-11-09/frozen-corrected.json):
corrected readings of seen sets, no model chosen on them. On the 80 the fix
reads 288 key columns typed `text` as longtext and leaves 12 tables without
a key against 88; on the 177, 761 longtext key columns and 15
`decimal` ones.

| F1 | the 80, first | the 80, corrected | the 177, first | the 177, corrected | the style set, first | the style set, corrected | the fresh set, read once |
|---|---|---|---|---|---|---|---|
| `1s` | 0.744 | 0.694 | 0.756 | 0.707 | 0.700 | 0.707 | **0.738** |
| `seven-restyled` | 0.792 | 0.812 | 0.767 | 0.769 | 0.670 | 0.763 | 0.690 |
| `seven-bounded` | | 0.817 | 0.775 | 0.773 | 0.685 | **0.779** | 0.719 |
| `seven-style` | | **0.826** | 0.762 | 0.765 | 0.679 | 0.775 | |
| **`nine-mended`** | | 0.802 | | 0.770 | 0.665 | 0.762 | 0.715 |
| `nine-mended` minus `1s` | | +0.108 [+0.047, +0.181] | | +0.063 [+0.019, +0.122] | −0.035 [−0.089, +0.015] | +0.055 [−0.054, +0.195] | −0.023 [−0.062, +0.020] |

([heldout-github-2025-typed.json](data/keys-final-2026-11-09/heldout-github-2025-typed.json),
[heldout-github-2025-rest-typed.json](data/keys-final-2026-11-09/heldout-github-2025-rest-typed.json),
[heldout-github-style-typed-five.json](data/keys-final-2026-11-09/heldout-github-style-typed-five.json),
[style-nine-mended.json](data/keys-final-2026-11-09/style-nine-mended.json);
`nine-mended`'s first reading of the style set and the one with its keys
fixed, 0.678 against `1s`'s 0.709, −0.031 [−0.085, +0.019], in
[heldout-github-style-nine-mended.json](data/keys-final-2026-11-09/heldout-github-style-nine-mended.json)
and [heldout-github-style-fixed-nine-mended.json](data/keys-final-2026-11-09/heldout-github-style-fixed-nine-mended.json).)

**The four readings as they now stand**, pooled
([github-pooled.json](data/keys-final-2026-11-09/github-pooled.json)): 452
schemas, 10 362 keys —

| | P / R / F1 [95 %] | macro F1 | minus `1s`, F1 | minus `1s`, macro F1 |
|---|---|---|---|---|
| `1s` | 0.832 / 0.620 / 0.711 [0.665, 0.750] | 0.677 | | |
| `seven-restyled` | 0.779 / 0.747 / 0.763 [0.730, 0.795] | 0.777 | +0.052 [+0.017, +0.091] | +0.100 [+0.075, +0.125] |
| `seven-bounded` | 0.799 / 0.749 / 0.773 [0.744, 0.801] | 0.778 | +0.063 [+0.031, +0.102] | +0.101 [+0.077, +0.125] |
| **`nine-mended`** | 0.806 / 0.729 / **0.765** [0.732, 0.795] | 0.769 | **+0.055 [+0.021, +0.095]** | +0.093 [+0.068, +0.118] |

The pooled picture in one sentence: over every GitHub schema read, the
newer models are ahead of `1s` by 0.05 in F1 and 0.09–0.10 in macro F1, both
intervals above zero, but three of the four readings were corrected after
they were first read, the correction reading `text` keys as the training
reads them, which `1s` cannot take as candidates, and on the one set chosen,
fetched and read with the fixed collector from the start `1s` is ahead.

## 5. SchemaPile-Perm, the pair task, Nexus, the bounds, database A

**SchemaPile-Perm's test**, round 1s's `refit.score` run on `nine-mended`
([perm-nine-mended.json](data/keys-final-2026-11-09/perm-nine-mended.json)):

| | `nine-mended` | round 1s's refit `both` |
|---|---|---|
| over every declared key, P / R / F1 | 0.715 / 0.749 / **0.732** | 0.736 |
| over candidates, model / rules [difference] | 0.743 / 0.417 [+0.287, +0.363] | 0.764 / 0.420 |
| the hard cases (rules 0.544) | 0.875 | 0.871 |
| the pair task, recall at one / covered | **0.941** / 0.973 | 0.905 / 0.929 |

**The pair task on the external sets** and **related or not**, round 1p's
protocol ([external-score-nine-mended.json](data/keys-final-2026-11-09/external-score-nine-mended.json)):
the pair task, recall at one over the whole database under the paper's filters, Spider **0.956** [0.925, 0.979] (768 pairs), BIRD **0.924** [0.863, 0.967] (381), CTU **0.957** [0.920, 0.978] (1 267), against the refit's 0.913, 0.898 and 0.886; with two tables only, 0.953, 0.900 and 0.944; related or not, round 1p's 2 416 related and 2 197 unrelated
pairs, AUC **0.961** (Spider 0.976, BIRD 0.954, CTU 0.957), 1.6 % of
unrelated pairs accepted at 80 % of related, against the refit's 0.931 and
3.6 % and round 1p's T5's 0.798.

**Nexus's sets**, round 1p's [nexus.py](../round-1p/nexus.py)
([nexus-nine.json](data/keys-final-2026-11-09/nexus-nine.json)):

| | tables, in SchemaPile-Perm | keys | `nine-mended` at its threshold | at the set's best | round 1p's `both` | Nexus's XGBoost |
|---|---|---|---|---|---|---|
| TPC-DS | 24, 0 | 102 | **0.809** | 0.840 | 0.525 / 0.824 | 0.44 |
| BIRD's development set | 75, 5 | 104 | 0.707 | 0.733 | 0.787 / 0.796 | 0.10 |
| TPC-H | 8, 8 | 7 | 0.933 | 1.000 | | 0.35 |

TPC-H is in SchemaPile-Perm whole, so its figure is memory.

**The recall bounds**, as round 1t §3 drew them for `cues-restyled`
([bounds.json](data/keys-final-2026-11-09/bounds.json)): the candidates'
coverage of the declared keys (`nine-mended`'s candidates are version 6's,
the same pairs in the same order as `six` and `seven` on Spider's 15 003 and
BIRD's 35 266, [candidates-six.json](data/keys-final-2026-11-09/candidates-six.json)),
F1 at its threshold, at the best one threshold for the set, and with each
schema at the threshold best for its own F1, counts then pooled:

| | coverage | at its threshold | the best threshold for the set | each schema's best |
|---|---|---|---|---|
| SchemaPile's test | 0.968 | 0.732 | 0.732 (0.82) | 0.805 |
| Spider | 0.984 | 0.880 | 0.886 (0.72) | 0.924 |
| BIRD | 0.969 | 0.815 | 0.815 (0.74) | 0.853 |
| CTU | 0.995 | 0.663 | 0.675 (0.70) | 0.777 |
| LiveSQLBench | 0.957 | 0.542 | 0.607 (0.54) | 0.670 |

**The owner's database A**, round 1q's `owner.py draw` on its snapshot, counts
only ([owner-A.json](data/keys-final-2026-11-09/owner-A.json)): 52 declared
one-column keys; `nine-mended` draws 55, 39 of them declared (precision
counting declarations only 0.709, F1 0.729); of the 16 no declaration names,
11 are among the 36 round 1q's reading of the app's code judged and 5 were
not read. Round 1q kept that reading's counts, not its verdicts key by key,
so how many of the 11 are links is not given here.

## 6. The main table: against round 1s's four language models

`nine-mended`'s open predictions on round 1p's 398 external databases (from
§5's run) and on LiveSQLBench, against round 1s's four models' saved replies
under its one protocol, read by round 1s's `lm.py`, and round 1's rules (the
harness's guesses, declarations hidden) on the same footing; no model was
asked again ([lms.json](data/keys-final-2026-11-09/lms.json)):

| 398 external databases, 3 371 keys | P / R / F1 | `nine-mended` minus this: P | R | F1 |
|---|---|---|---|---|
| **`nine-mended`** | 0.844 / 0.684 / **0.755** [0.674, 0.829], macro 0.826 | | | |
| round 1's rules | 0.874 / 0.350 / 0.500 | −0.030 [−0.086, +0.020] | +0.334 [+0.273, +0.410] | **+0.256 [+0.209, +0.313]** |
| Gemini 3 Flash | 0.832 / 0.836 / 0.834 | +0.012 [−0.037, +0.058] | −0.152 [−0.271, −0.051] | −0.079 [−0.154, −0.018] |
| Gemini 3.5 Flash-Lite | 0.875 / 0.843 / 0.859 | −0.031 [−0.086, +0.025] | −0.160 [−0.280, −0.056] | −0.104 [−0.171, −0.053] |
| Claude Haiku 5.5 | 0.835 / 0.915 / 0.873 | +0.009 [−0.037, +0.053] | −0.231 [−0.316, −0.165] | −0.117 [−0.181, −0.066] |
| Gemini 3.8 Flash | 0.886 / 0.894 / 0.890 | −0.042 [−0.084, −0.008] | −0.210 [−0.307, −0.144] | −0.135 [−0.198, −0.089] |

By source, `nine-mended` reads Spider 0.880, BIRD 0.815, CTU 0.663 (macro
0.886, 0.756, 0.769); its distance from the best language model is −0.046
on Spider, −0.103 on BIRD, −0.200 on CTU, every interval below zero.

| LiveSQLBench, 22 databases, 299 keys | P / R / F1 | `nine-mended` minus this: P | R | F1 |
|---|---|---|---|---|
| **`nine-mended`** | 0.850 / 0.398 / **0.542** [0.380, 0.682], macro 0.430 | | | |
| round 1's rules | no key drawn | | | +0.542 |
| Gemini 3 Flash | 0.928 / 0.866 / 0.896 | −0.078 [−0.200, +0.010] | −0.468 [−0.597, −0.341] | −0.354 [−0.492, −0.225] |
| Gemini 3.5 Flash-Lite | 0.931 / 0.863 / 0.896 | −0.081 [−0.202, +0.006] | −0.465 [−0.599, −0.338] | −0.354 [−0.493, −0.225] |
| Claude Haiku 5.5 | 0.927 / 0.890 / 0.908 | −0.077 [−0.203, +0.016] | −0.492 [−0.638, −0.342] | −0.366 [−0.520, −0.225] |
| Gemini 3.8 Flash | 0.938 / 0.910 / 0.924 | −0.088 [−0.210, −0.006] | −0.512 [−0.644, −0.388] | −0.381 [−0.525, −0.255] |

## 7. The language models on the fresh set

At the owner's approval, the four asked under round 1s's protocol on the
fresh set's 100 schemas: round 1p's prompt (`gemini.render`, `DB_ASK`), one
reply a schema, [ask.py](../round-1s/ask.py) at its settings (temperature 0
where a model takes one, 32 768 tokens, four in flight), the replies read by
`lm.py` ([fresh-lms.json](data/keys-final-2026-11-09/fresh-lms.json),
`replies-*-fresh-db.jsonl`). The Gemini models through CLIProxyAPI, as round
1s asked them.

| | P / R / F1 [95 %] | macro F1 | `nine-mended` minus this: P | R | F1 | `1s` minus this, F1 |
|---|---|---|---|---|---|---|
| **`nine-mended`** | 0.853 / 0.616 / 0.715 [0.635, 0.791] | 0.775 | | | | |
| `1s` | 0.881 / 0.635 / 0.738 [0.680, 0.792] | 0.725 | | | | |
| Gemini 3 Flash | 0.786 / 0.924 / 0.849 [0.787, 0.903] | 0.870 | +0.067 [−0.027, +0.167] | −0.308 [−0.419, −0.188] | **−0.134 [−0.228, −0.037]** | −0.111 [−0.190, −0.029] |
| Gemini 3.5 Flash-Lite | 0.790 / 0.933 / 0.856 [0.795, 0.908] | 0.879 | +0.063 [−0.027, +0.159] | −0.317 [−0.429, −0.200] | **−0.140 [−0.231, −0.049]** | −0.118 [−0.193, −0.038] |
| Gemini 3.8 Flash | 0.882 / 0.942 / 0.911 [0.887, 0.940] | 0.928 | −0.029 [−0.086, +0.012] | −0.326 [−0.441, −0.202] | **−0.196 [−0.265, −0.129]** | −0.173 [−0.226, −0.125] |
| Claude Haiku 5.5 | 0.792 / 0.951 / 0.864 [0.805, 0.917] | 0.899 | +0.061 [−0.031, +0.161] | −0.335 [−0.438, −0.229] | **−0.149 [−0.238, −0.057]** | −0.126 [−0.200, −0.046] |

| | input tokens | output (reasoning) tokens | a reply, median / 90th percentile | list price |
|---|---|---|---|---|
| Gemini 3 Flash | 194 118 | 71 316 (0) | 1.4 / 3.2 s | $0.31 |
| Gemini 3.5 Flash-Lite | 194 118 | 65 212 (0) | 1.6 / 3.2 s | $0.22 |
| Gemini 3.8 Flash | 194 118 | 672 470 (612 361) | 13.4 / 142.7 s | $2.67 |
| Claude Haiku 5.5 | 275 308 | 190 775 (107 133) | 4.4 / 13.0 s | $0.12 |

The spend at list price, as round 1s priced it, is **$3.20** for the three
Gemini models, above the estimate of about $2 that the owner approved and
then approved again, and $0.12 for Haiku, through Anthropic's Messages API
with the owner's key: $3.32 in all. The Gemini requests went through
CLIProxyAPI on the owner's server, so what was really charged for them is
not known here.

On clean data, the language models stand where they stood on development
data: 0.85–0.91 against the model's 0.72, the gap recall, −0.31 to −0.34,
with the model's precision above the two fast Gemini models' and level with
3.8 Flash's.

## 8. The judge on the fresh set

§1 of [keys-mended](keys-mended-2026-11-09.md#1-a-blind-judge-of-the-keys-no-declaration-names)'s
protocol on the fresh set for `1s` and `nine-mended` (`judge.py pack fresh`,
13 packets, Claude Sonnet 5.5 at medium effort as Claude Code agents, blind;
[judge-fresh-count.json](data/keys-final-2026-11-09/judge-fresh-count.json),
[judged-pairs-fresh.json](data/keys-final-2026-11-09/judged-pairs-fresh.json)):
declared controls called yes 296 of 300, candidates no model draws 1 of 300;
of the 224 pairs one of the two draws that no declaration names, 133 yes.

| | drawn | declared | undeclared, the judge's yes | precision, declared | with the judge | F1 with the judge | minus `1s` |
|---|---|---|---|---|---|---|---|
| `1s` | 1 453 | 1 280 | 99 of 132 | 0.881 | **0.949** | 0.766 | |
| `nine-mended` | 1 456 | 1 242 | 93 of 163 | 0.853 | **0.917** | 0.741 | −0.025 [−0.076, +0.029] |

On the fresh set, fewer of `nine-mended`'s undeclared keys are real than
of `1s`'s, 57 % against 75 %; counted with the judge, both models'
precision is above 0.9.

**Continued** in [keys-size-2026-11-10.md](keys-size-2026-11-10.md): `nine-mended` by the number of tables of
a schema, what SchemaPile's declarations leave out, and a first stage read against the whole schema, 0.055 above it
on a clean set of plain GitHub schemas and not shown above it in another house style.

## Limits

- The corrected readings are of sets read before; their gain for the
  version 6 models is in part `1s`'s preparation, which cannot take a
  longtext key as a candidate, as it could not in training.
- Database A's code reading was not kept key by key.
- One judge, Sonnet, of the family that wrote the lexicons; its controls are
  easier than the pairs it judges.

## Environment and reproduction

| | |
|---|---|
| machine | AMD Ryzen 7 7700, 31 GB, NVIDIA GeForce RTX 5070, Windows 11 Pro |
| Python | 3.13.15; xgboost 3.4.1, lightgbm 4.7.0, scikit-learn 1.9.1, numpy 2.5.4 |
| container | Docker 29.6.2, `rust:1.99.1` (`09027f873d45`), `--cpuset-cpus 3` |
| language models | round 1s's four, as `lm.py` names them; Gemini through CLIProxyAPI, Claude Haiku 5.5 through Anthropic's Messages API (SDK 2.6.0, the owner's key), 9 November 2026; Claude Sonnet 5.5 (medium effort) as the judge |
| research commit | `f8d6e8a` and this report's files |
| times (UTC) | parity 14:58–15:23; the Python chain 15:24–15:57; the Gemini models 15:26–15:52, Haiku 16:35–16:37; the judge 15:42–15:43; the container 15:59–16:34 |

```sh
cd <repo>/lemmesee
P=round-1b/.venv/Scripts/python
G=corpus/round-1t/.venv-parse/Scripts/python
$P -I round-1t/parity.py pack corpus/round-1t/models/nine-mended
(cd round-1t/reader && cargo build --release)
$P -I round-1t/parity.py check corpus/round-1t/models/nine-mended corpus/round-1b corpus/round-1s/external \
  round-1t/reader/target/release/reader corpus/round-1t/parity/nine-mended
MSYS_NO_PATHCONV=1 docker run --rm --cpuset-cpus 3 -v <repo>:/src -v lemmesee-cargo:/usr/local/cargo/registry \
  -w /src/lemmesee rust:1.99.1 sh round-1t/reader/bench.sh corpus/round-1s/models/both <1s threshold> \
  corpus/round-1t/models/nine-mended <threshold> corpus/round-1t/bench9
$P -I round-1u/questions.py --models nine-mended seven-restyled
$G -I round-1t/github.py parse typed
$G -I round-1t/github.py parse rest-typed
$P -I round-1t/fit.py freeze-corrected
$P -I round-1t/fit.py heldout "github 2025, typed" frozen-corrected.json
$P -I round-1t/fit.py heldout "github 2025, rest, typed" frozen-corrected.json
$P -I round-1t/fit.py heldout "github style, typed, five models" frozen-corrected.json
$P -I round-1t/fit.py freeze-sets frozen-nine-style.json nine-mended "github style, nine-mended" "github style, fixed, nine-mended"
$P -I round-1t/fit.py heldout "github style, nine-mended" frozen-nine-style.json
$P -I round-1t/fit.py heldout "github style, fixed, nine-mended" frozen-nine-style.json
$P -I round-1t/paper.py perm
$P -I round-1t/paper.py lms
$P -I round-1t/paper.py nexus
$P -I round-1t/paper.py bounds
$P -I round-1t/paper.py owner
CLIPROXY_KEY=... $P -I round-1s/ask.py run cliproxy <endpoint>/v1 <model> corpus/round-1t/lm fresh-db 4
ANTHROPIC_API_KEY=... round-1s/.venv/Scripts/python -I round-1s/ask.py run anthropic - claude-haiku-5-5 corpus/round-1t/lm fresh-db 4
$P -I round-1t/paper.py fresh-lms
$P -I round-1t/judge.py pack fresh     # then one agent a packet, round-1t/judge-prompt.md; then:
$P -I round-1t/judge.py count fresh
```

The fresh set's prompts were written by `gemini.render` and `DB_ASK` into
`corpus/round-1t/lm/prompts/fresh-db/`, the pooled GitHub readings and the
style set's pairings by a few lines over the saved files, and the sizes by
`gzip -9 -n` over the files as committed.
