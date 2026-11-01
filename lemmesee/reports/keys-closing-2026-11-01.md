# Closing on the language models, 1 November 2026

Round 1q, as [its specification](../round-1q/README.md) sets it: whether
round 1p's key model ([report](keys-external-2026-11-01.md)) can come closer
to language models in the open setting — every column of a database, no
table named — by its threshold, by features for keys named after a role, by
a cascade that asks a language model only what the model is unsure of, and
by the values the database's planner keeps; then the owner's two production
databases, and a database's own declared keys as a dialect for its
undeclared ones. Its harness and specification are at research commit
`760041d`; the specification was written in stages, each stage marked with what
had and had not been read when it was written, an order the commit itself
cannot show. Raw output in [data/keys-closing-2026-11-01/](data/keys-closing-2026-11-01/).

Every F1 below is over every one-column key the databases declare, as round
1p counted it against Gemini, unless it says otherwise. Every interval is a
95 % percentile interval of an F1 difference over 1 000 resamples of
databases. **Two models appear, and each table says which**: `both`, round
1p's widened variant, the model this round ends with (sections 1, 2, 6 and
7); and `both, roles`, the variant section 3's rule took, on which sections 4
and 5 were built: by the author's account the cascade was cut minutes after
the variant's external scores were written and before they were read, which
the files' times are consistent with but cannot show.

**What was not specified before it ran**: the Haiku arms (section 2's agents
and section 4's cascade), section 5's drop against any unique column,
section 6's reading of database A and its drop, and section 7. Each is
marked where it appears.

| Question | Answer | What follows |
|---|---|---|
| Is the threshold the gap? | **No.** At its own threshold `both` reaches 0.736 over the 398 external databases; the best of a 0.01 grid of thresholds, 0.735 | the gap is the model's candidates and scores, not its cut |
| How far are the fast language models? | Over the 398 databases: Gemini 3.5 Flash-Lite 0.844, Gemini 3 Flash 0.846, Haiku run as agents 0.888, Gemini 3.8 Flash 0.891, against `both`'s 0.736; `both` minus each, four intervals all below zero (Gemini 3 Flash's [−0.175, −0.042] the closest). Its precision, 0.848, is the fast models' (0.830–0.852), below 3.8's 0.893; its recall, 0.649, against 0.840–0.942. Against 3.8, recall raised alone would close 85 % of the gap, precision alone 10 % | the gap is mostly recall, and most of the missing keys are CTU's |
| Do features for keys named by a role help? | Taken by the rule on validation, +0.0046 against a spread of 0.0062 among three seeds of `both` and one fit of their own — not distinguishable from a refit. Off SchemaPile, 0.732 against 0.736 in the open task; in the paper's pair task on CTU 0.903 against 0.886. The features were written after round 1p's failure analysis of these databases | not the model; `both` stays |
| How close does a cascade come? | Haiku, asked about the 2 040 candidates `both, roles` is unsure of, lifts it from 0.732 to 0.787 over all 398 databases. On the 369 with statistics, the lift is +0.019 [+0.009, +0.031] on Spider, +0.070 [+0.017, +0.146] on BIRD's 47 and +0.086 [+0.033, +0.163] on CTU, and Haiku given the whole schema is ahead of the cascade by +0.030 on Spider and +0.171 on CTU and level on BIRD's 47, −0.003 [−0.036, +0.031] | a cascade is bounded by the model's candidates; asking about everything is better |
| Do the planner's own values help? | Range inclusion as specified **fails** on Spider and BIRD: 0.837 against 0.864, −0.026 [−0.072, +0.004]. On CTU — held out from the values, though not from the role features `both, roles` carries — value keys add +0.017 [+0.001, +0.042]; the catalog's drop is estimated at −0.008 [−0.028, +0.003] for the model and −0.005 to −0.007 for each language model, every interval containing zero; a drop against any unique column of the target, defined after CTU had been read, is within half a point of each arm alone | values add little on these benchmarks |
| What do the owner's databases say? | On database A, of 36 keys the two models drew that its schema does not declare, a read of the app's code (one agent's, read-only) finds 8 links through the target's primary key, 21 through another unique column of it — a messaging platform's id — and 7 wrong. Counting links right, `both`'s precision there is **0.922** (0.627 counting only declarations), the reported model's 0.933 (0.583); a drop against any unique column, suggested by A itself, removes 3 of the 5 wrong keys with statistics and none of the 11 links with statistics. Database B: F1 0.909 against its declarations | declarations are a floor on precision; references into alternate keys occur in A and in BIRD |
| Does a database's own dialect help? | Exploratory. With half of each database's declared keys shown, drawing for a column the target of a shown key of the same name lifts F1 over the hidden half from 0.673 to 0.738 over 195 databases, +0.060 to +0.070 in three splits, each interval clear of zero; CTU +0.105 to +0.122; Spider +0.006 to +0.021 and BIRD +0.002 to +0.019, not clear of zero in every split | worth a specified round of its own |

## The databases

As round 1p's: Spider's 206, BIRD's 80, CTU's 142 — 398 declaring at least
one key, 3 371 one-column keys. Planner statistics were computed for all of
Spider and CTU and for the 49 BIRD databases unpacked on disk, 47 of which
declare a key; the arms that read statistics are measured on those 369
databases and say so.

## 1. The threshold

[threshold.json](data/keys-closing-2026-11-01/threshold.json): `both`'s F1
at thresholds 0.05 to 0.95 in steps of 0.01. `both`'s own threshold is
0.809, between two steps of the grid, so its F1 at the grid's 0.81 is a
little below its F1 at its threshold.

| | best of the grid | P / R / F1 there | at 0.81 |
|---|---|---|---|
| validation | 0.81 | 0.710 / 0.672 / 0.690 | the same |
| validation, complete schemas | 0.81 | 0.754 / 0.710 / 0.731 | the same |
| Spider | 0.84 | 0.920 / 0.830 / 0.872 | 0.911 / 0.834 / 0.871 |
| BIRD | 0.73 | 0.868 / 0.805 / 0.836 | 0.886 / 0.785 / 0.833 |
| CTU | 0.62 | 0.666 / 0.600 / 0.631 | 0.781 / 0.513 / 0.619 |
| all | 0.79 | 0.833 / 0.657 / 0.735 | 0.845 / 0.649 / 0.734 |

At a threshold of 0.05 recall is 0.775 over all, 0.680 on CTU: past that no
threshold reaches.

## 2. The fast models

Round 1p's prompts unchanged at temperature 0 through the same endpoint for
the Gemini models; Haiku, not in the specification, as six agents at high
effort, each given one file of 1 to 139 databases' prompts and reading
nothing else ([haiku-agents.json](data/keys-closing-2026-11-01/haiku-agents.json)).

| | Spider P / R / F1 | BIRD | CTU | all |
|---|---|---|---|---|
| `both` | 0.911 / 0.835 / 0.872 | 0.896 / 0.785 / 0.837 | 0.784 / 0.513 / 0.621 | 0.848 / 0.649 / 0.736 |
| Gemini 3.5 Flash-Lite | 0.880 / 0.937 / 0.908 | 0.819 / 0.934 / 0.873 | 0.807 / 0.797 / 0.802 | 0.830 / 0.859 / 0.844 |
| Gemini 3 Flash | 0.890 / 0.943 / 0.916 | 0.856 / 0.932 / 0.893 | 0.829 / 0.758 / 0.792 | 0.852 / 0.840 / 0.846 |
| Haiku, agents | 0.879 / 0.965 / 0.920 | 0.834 / 0.959 / 0.892 | 0.824 / 0.926 / 0.872 | 0.840 / 0.942 / 0.888 |
| Gemini 3.8 Flash (round 1p) | 0.913 / 0.963 / 0.937 | 0.935 / 0.949 / 0.942 | 0.868 / 0.832 / 0.850 | 0.893 / 0.888 / 0.891 |

| `both` minus | Spider | BIRD | CTU | all |
|---|---|---|---|---|
| Gemini 3.5 Flash-Lite | [−0.061, −0.015] | [−0.097, +0.030] | [−0.301, −0.080] | [−0.168, −0.054] |
| Gemini 3 Flash | [−0.071, −0.021] | [−0.109, −0.004] | [−0.311, −0.033] | [−0.175, −0.042] |
| Haiku, agents | [−0.074, −0.026] | [−0.123, +0.010] | [−0.346, −0.160] | [−0.211, −0.088] |
| Gemini 3.8 Flash | [−0.090, −0.040] | [−0.160, −0.052] | [−0.333, −0.140] | [−0.209, −0.106] |

Of the true keys a model finds and `both` misses, CTU holds 75 % for
Gemini 3.8 and 77 % for Haiku. In the paper's pair task Flash-Lite
reaches 0.952 / 0.916 / 0.931 against `both`'s 0.913 / 0.900 / 0.886.
Gemini 3 Flash answered every whole-schema prompt but stopped on the
account's quota, refused with HTTP 429 from 10:44, with 1 217 of its 2 636
pair prompts unanswered; its pair task is not reported.

## 3. Keys named by a role

`both, roles` adds five features: the column's name less its digits is the
target's key name, the key's name is inside the column's, the column ends
with it, how many targets share that key name, how many columns carry it.
They were written after round 1p's failure analysis of the external
databases.

| | `both` | `both, roles` |
|---|---|---|
| validation, F1 over every declared key | 0.690 | 0.694 |
| validation, the paper's task | 0.853 | 0.854 |
| test, F1 over its candidates (the rules') | 0.761 (0.420) | 0.770 (0.420) |
| the hard cases | 0.871 | 0.874 |
| external, F1 over every declared key | 0.736 | 0.732 |
| external, the paper's task, Spider / BIRD / CTU | 0.913 / 0.900 / 0.886 | 0.913 / 0.898 / 0.903 |
| database A, keys drawn its schema does not declare | 19 | 47 |

The rule took the variant (+0.0046 against the +0.003 it asked). Three seeds
of `both` spread over 0.6875–0.6937 on the same measure and `both, roles`
was fitted once, so the gain cannot be told from a refit's. On CTU's
Mooney_Family both variants put the right target first for all 134 keys and
score them at a median of 0.005 (`both`) and 0.025 (`both, roles`)
([mooney-family.json](data/keys-closing-2026-11-01/mooney-family.json)):
the model takes `name1` → `person` to be unlikely a key at all. Database A's
extra draws were not read against its code and are no evidence either way.

## 4. The cascade

Built on `both, roles` (alone 0.870 / 0.828 / 0.615, 0.732 over all): it
draws what it scores at or above its threshold, 0.851; a candidate in
[0.2, 0.851) is asked, all of a database's questions in one prompt with its
schema — 2 040 questions in 207 databases, 2 457 keys drawn without asking.
The specification asked for the cascade with each fast model and Gemini 3.8;
Flash-Lite's first three cascade prompts were refused for the quota and no
Gemini model was asked further. **Haiku, as six agents, replaced them**, an
unplanned substitute; it answered every question, 738 yes. 452 of the 2 040
candidates are declared keys and it said yes to 450, so the cascade reaches
nearly all it can.

| | Spider | BIRD | CTU | all |
|---|---|---|---|---|
| `both, roles` alone | 0.870 | 0.828 | 0.615 | 0.732 |
| the cascade with Haiku | 0.890 | 0.887 | 0.701 | 0.787 |
| Haiku given the whole schema | 0.920 | 0.892 | 0.872 | 0.888 |

On the 369 databases with statistics: the cascade minus the model alone,
Spider +0.019 [+0.009, +0.031], BIRD +0.070 [+0.017, +0.146], CTU +0.086
[+0.033, +0.163]; Haiku given the whole schema minus the cascade, Spider
+0.030 [+0.014, +0.052], BIRD −0.003 [−0.036, +0.031], CTU +0.171 [+0.056,
+0.261]. The cascade's prompts hold 842 285 characters against 982 750 for
the whole-schema prompts of all 398 databases; the agents took 1.18 million
tokens for the cascade's 207 databases and 1.52 million for the 398 whole
schemas, an agent's overhead included. With the band's floor at 0.2, CTU's
role-named keys, scored near zero, are never asked.

## 5. Whose values: the planner's sample

`stats.py` computes what PostgreSQL's `ANALYZE` keeps at
`default_statistics_target` 100 — a sample of up to 30 000 rows a table, the
null fraction, the distinct count, up to 100 most common values, up to 101
distinct histogram bounds — for Spider's and BIRD's SQLite files, and
`ctu_stats.py` for CTU's tables through its server, all 142 databases, none
failing. The arms are applied to `both, roles`.

**Range inclusion, as specified**, on Spider and BIRD (251 databases with
statistics, 1 236 keys): a candidate in the band drawn when every sampled
value lies in the target key's range and its distinct count fits, a key
above the threshold dropped when under half do. F1 0.837 against 0.864,
−0.026 [−0.072, +0.004]; precision 0.816 against 0.916. Of the band's 462
candidates with statistics 88 are keys; inclusion raises their share to 28 %
— 49 % against text keys, 22 % against other keys, whose range 1 … N holds
any small integer ([whose.json](data/keys-closing-2026-11-01/whose.json)).
**The rule fails**, its interval just reaching zero.

**Value keys**: a column whose sampled values are found, 90 % of them at
least, among a unique text column's sampled values, that column's sample
holding half its distinct values at least, the column not unique itself and
its distinct count at most the other's rows, drawn for a column the model
drew no key for. A column with several such anchors takes the first found,
not the one the model scored higher as the specification said. Thresholds
chosen on Spider and BIRD, where every point of the grid scores 0.866–0.867
against the model's 0.864
([values-tune-bird-spider.json](data/keys-closing-2026-11-01/values-tune-bird-spider.json)),
then measured on CTU.

| CTU, 118 databases, 1 854 keys | P / R / F1 | minus the model alone |
|---|---|---|
| `both, roles` alone | 0.823 / 0.490 / 0.615 | |
| with value keys | 0.778 / 0.532 / 0.632 | +0.017 [+0.001, +0.042] |
| with the catalog's drop | 0.832 / 0.478 / 0.607 | −0.008 [−0.028, +0.003] |
| with both | 0.784 / 0.520 / 0.626 | +0.011 [−0.014, +0.036] |
| the cascade with Haiku | 0.733 / 0.671 / 0.701 | +0.086 [+0.033, +0.163] |
| the cascade with Haiku, and both | 0.708 / 0.681 / 0.694 | |
| Haiku given the whole schema | 0.824 / 0.926 / 0.872 | |
| Haiku, with the catalog's drop | 0.830 / 0.903 / 0.865 | −0.007 [−0.025, +0.002] against Haiku alone |
| Gemini 3.8 Flash, with the catalog's drop | 0.873 / 0.817 / 0.844 | −0.006 [−0.018, +0.001] against 3.8 alone, 0.850 |

Value keys add 78 true keys and 87 false ones on CTU (987 true of 1 269 drawn, against 909 of 1 104). The catalog's
drop is estimated to cost each language model on CTU 0.5 to 0.7 points,
every interval containing zero. CTU is held out from the values, not from
the role features `both, roles` carries, which were written after round 1p's
failure analysis of the same databases. That benchmarks lack what the values catch is a hypothesis,
not measured here.

**How CTU was read.** The specification held CTU out until its statistics
were complete; a filter meant to hide CTU's block of the arms' output
failed, and its numbers were first read with 139 of 142 databases sampled.
The three left declare no key, and the complete run gave the same figures.
Nothing was changed between the two.

**The drop against any unique column**, exploratory, defined after that
first reading. On BIRD the drop as specified cost each language model 8.6
points of recall, the same 28 keys: keys into another unique column of the
target than its primary key — `Match.away_player_1` →
`Player.player_api_id`, `legalities.uuid` → `cards.uuid`,
`set_translations.setCode` → `sets.code`. Compared against any unique column
of the target — its declared key, a declared unique column, or one every
sampled value of which is distinct — the drop moves no arm by more than
half a point on Spider, BIRD or CTU (on BIRD +0.003 [0.000, +0.008] for the
model alone; Gemini 3.8 Flash 0.937 as alone).

## 6. The owner's databases

Their snapshots of round 1k — schema and statistics, read-only — with the
declarations hidden; counts only, no table's or column's name. A has 63
tables; B 22, and 5 views the model does not read. **Exploratory**: not in
the specification.

| | keys declared | `both`: found | drawn and not declared | F1 against the declarations |
|---|---|---|---|---|
| B | 40 | 35 | 2 | 0.909 |
| A | 52 | 32 | 19 | 0.621 |

**What A's undeclared keys are.** In a first run the reported model and
`both` drew 36 keys A's schema does not declare. An agent read the app's
source, read-only, and for each of those 36 said whether the column holds its target's primary key, holds another unique
column of the target, or is wrong: **8, 21 and 7** — the 21 hold a messaging
platform's id, which the target keeps as a unique column beside its serial
key, or a forum topic id matched with its chat in a configuration table; the
7 are five of the platform's message ids taken for a billing table's key,
one id of another table, one key declared elsewhere. This is one agent's
reading, not the owner's; the data keep its counts, not its names. Every
count below comes from a second run, `owner.py draw`, which keeps both
models' keys once and is read by each later step; its 36 undeclared keys
share 35 with the first run's, so one key drawn now was not read and one of
the 7 wrong ones is not drawn now. Counted on the server, read-only, all 36
statements answering: the 7 primary-key
links with values have no distinct value outside the target's primary key;
of the 15 alternate-key links with values, 14 have 82–100 % outside it and
one 0 %; of the 5 wrong keys with values, 0, 58, 65, 76 and 100 %
([owner-A.json](data/keys-closing-2026-11-01/owner-A.json)). No value
outside the primary key picks 9 columns, the 7 primary-key links and one of
each other kind; and the share outside the primary key does not separate a
link through an alternate key from a wrong key.

| A, from the same run | drawn | declared | links, primary key | links, another unique column | wrong | not read | precision counting links right | counting only declarations |
|---|---|---|---|---|---|---|---|---|
| the reported model | 60 | 35 | 7 | 14 | 3 | 1 | 0.933 | 0.583 |
| with the drop against any unique column | 59 | 35 | 7 | 14 | 2 | 1 | 0.949 | 0.593 |
| `both` | 51 | 32 | 8 | 7 | 4 | 0 | 0.922 | 0.627 |
| with the drop against any unique column | 49 | 32 | 8 | 7 | 2 | 0 | 0.959 | 0.653 |

The drop, from `pg_stats` alone and server-side, removes 3 of the 5 wrong
keys that have a sample on both sides and none of the 11 links that do. A is
the database that suggested this drop, so these figures are in-sample.
Neither database has `pg_stat_statements` installed.

## 7. A database's own dialect

**Exploratory**: not specified before it ran. `dialect.py`: of each database
declaring four keys or more (195), half its keys at random are shown and
half hidden; `both` guesses with every declaration hidden, as before; then a
column not shown takes the target of a shown key whose column has the same
name, unless it is its own table's key or that target is its own table. Over
the hidden half, three splits, mean P / R / F1:

| | databases | hidden keys | `both` alone | with the dialect | difference by split |
|---|---|---|---|---|---|
| Spider | 81 | 353 | 0.824 / 0.834 / 0.829 | 0.806 / 0.886 / 0.844 | +0.006 [−0.013, +0.028], +0.021 [+0.003, +0.043], +0.018 [+0.001, +0.043] |
| BIRD | 45 | 278 | 0.861 / 0.778 / 0.817 | 0.845 / 0.812 / 0.828 | +0.002 [−0.032, +0.040], +0.019 [−0.001, +0.054], +0.011 [−0.003, +0.033] |
| CTU | 69 | 887 | 0.646 / 0.483 / 0.553 | 0.647 / 0.689 / 0.668 | +0.117 [+0.039, +0.252], +0.122 [+0.039, +0.257], +0.105 [+0.027, +0.242] |
| all | 195 | 1 518 | 0.739 / 0.619 / 0.673 | 0.719 / 0.757 / 0.738 | +0.063 [+0.015, +0.131], +0.070 [+0.026, +0.149], +0.060 [+0.014, +0.134] |

Its first form, without the two exceptions, drew more and scored 0.645 over
all, a figure kept only here; the exceptions were added after that run, on
every source at once.
How common partially declared databases are is not measured here.

## Limits

- The language models' training data are unknown; Spider and BIRD are on the
  web. Haiku ran as agents reading a file of prompts, not as an API call at
  temperature 0, and its token counts include an agent's overhead.
- One fit of `both, roles`; one production database read against its code,
  by one agent.
- Planner statistics for Spider, BIRD and CTU were computed as `ANALYZE`
  would keep them, not read from PostgreSQL; only A's are PostgreSQL's own.
- The model reaches a key into another unique column as a pair of a column
  and a table, and so draws it; but in the paper's pair task it names the
  target's primary key, and a value check against the primary key misreads
  such keys, as section 5's first drop did on BIRD.
- The cascade runs a language model where lemmesee's rules
  (`lemmesee/teaching.md`) allow one only offline; it is measured as an
  alternative, not proposed for the product.

## Environment and reproduction

| | |
|---|---|
| research commit | `760041d` |
| models | round 1p's `both` and `both, roles` in `corpus/round-1p/variants/` |
| machine | AMD Ryzen 7 7700, RTX 5070, Windows 11 Pro 26300; Python 3.13.15 with round 1b's environment; PyMySQL 1.2.3 for CTU |
| language models | `gemini-3.5-flash-lite`, `gemini-3-flash`, `gemini-3.8-flash-high` through a CLIProxyAPI on 1 November 2026; Claude Haiku as agents at high effort, the prompts as `cascade.py cut` and `gemini.py cut` write them, one file a batch |

```sh
cd <repo>/lemmesee
P=round-1b/.venv/Scripts/python
$P -I round-1q/threshold.py corpus/round-1b corpus/round-1p/variants/both corpus/round-1p both
CLIPROXY_KEY=... $P -I round-1p/gemini.py ask corpus/round-1p <endpoint>/v1 <model> 3 corpus/round-1q/llm/<model>
$P -I round-1p/gemini.py measure corpus/round-1p -both corpus/round-1q/llm/<model> <model>
$P -I round-1p/gemini.py measure corpus/round-1p -both,-roles corpus/round-1q/llm/<model> <model>
$P -I round-1q/roles.py corpus/round-1b corpus/round-1p/variants
$P -I round-1p/widen.py final corpus/round-1b corpus/round-1p/variants "both, roles"
$P -I round-1p/widen.py external corpus/round-1p/variants "both, roles" corpus/round-1p
$P -I round-1q/cascade.py cut corpus/round-1p/variants/both,-roles corpus/round-1p "both, roles" corpus/round-1q/cascade-roles
$P -I round-1q/cascade.py cut corpus/round-1p/variants/both corpus/round-1p both corpus/round-1q/cascade-both
CLIPROXY_KEY=... $P -I round-1q/cascade.py ask corpus/round-1q/cascade-roles <endpoint>/v1 <model> 3
$P -I round-1q/cascade.py measure corpus/round-1p corpus/round-1q/cascade-roles <model>
$P -I round-1q/stats.py corpus corpus/round-1q/stats
CTU_PWD=... round-1q/.venv/Scripts/python -I round-1q/ctu_stats.py <host> <port> corpus/ctu-schema/databases.txt corpus/round-1q/stats
$P -I round-1q/whose.py corpus/round-1p corpus/round-1q/cascade-roles corpus/round-1q/stats corpus/round-1q spider,bird
$P -I round-1q/mooney.py corpus/round-1p corpus/round-1q
$P -I round-1q/values.py tune corpus/round-1p corpus/round-1q/cascade-roles corpus/round-1q/stats spider,bird
$P -I round-1q/arms.py corpus/round-1p corpus/round-1q/cascade-roles corpus/round-1q/stats corpus/round-1q/llm corpus/round-1q all
$P -I round-1q/prod.py corpus/prod corpus/round-1p corpus/round-1q <database>...
$P -I round-1q/owner.py draw corpus/prod corpus/round-1p <database> corpus/round-1q
$P -I round-1q/owner.py orphans corpus/prod <database> <ssh target> <container> corpus/round-1q
$P -I round-1q/owner.py inside corpus/prod <database> <ssh target> <container> corpus/round-1q
$P -I round-1q/owner.py links <database> <verdicts> <guesses> corpus/round-1q <data out> A
$P -I round-1q/dialect.py corpus/round-1p corpus/round-1q/cascade-both corpus/round-1q
```

For the Haiku agents, `corpus/round-1q/llm/haiku/` and
`corpus/round-1q/cascade-roles/replies/haiku/` hold their answers as one
reply a prompt, the form `gemini.py` and `cascade.py` read. Gemini 3.8
Flash's whole-schema replies are round 1p's, copied into
`corpus/round-1q/llm/gemini-3.8-flash-high/` for `arms.py`.
