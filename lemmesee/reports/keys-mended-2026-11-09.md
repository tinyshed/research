# The key model mended, and a fresh set in another house style, 9 November 2026

Round 1t continued, as [its specification](../round-1t/README.md)'s entries
of 9 November set it; the [house-style report](keys-style-2026-11-08.md)'s
§8 read the paper's model 0.030 below round 1s's on a clean set of GitHub
schemas whose keys are not named `table_id`. At the owner's approval of a
plan to mend that loss, this report gives, in order: a blind judge of the
keys the models draw there and no declaration names; two defects of the
collector found in its schemas, mended, and the set read again; one fit with
every known fix together, made the paper's model before any number of it was
seen; and a fresh clean set chosen by the same rule, read once. Raw output in
[data/keys-mended-2026-11-09/](data/keys-mended-2026-11-09/).

Every F1 is over every declared one-column key, as before; a precision
counted with a judge's answers stands beside it, never in its place.

| Question | Answer | What follows |
|---|---|---|
| **On a fresh clean set in another house style, read once, does the mended model beat round 1s's?** | 100 GitHub schemas, 2 016 keys, 1.5 % ending in `id`: `nine-mended` 0.715 [0.635, 0.791] against `1s`'s 0.738, **−0.023 [−0.062, +0.020]**; its macro F1 0.775 against 0.725, **+0.050 [+0.016, +0.092]** | not over every key pooled; over databases, yes: the pooled loss is three databases |
| What did the judge say of the "false" keys? | Seven in ten keys the models draw on the style set that no declaration names are real; counted so, precision 0.92–0.95 for all three models, F1 0.761 / 0.773 / 0.774 | the declared-key F1 counts many real keys false; it ranks the models the other way |
| What was wrong with the style set's reading? | The collector hid `decimal` keys (245 of 1 791) and lost primary keys; read again with both fixed, `seven-bounded` 0.779 against `1s`'s 0.707, level without its one `decimal` database | the −0.030 was in the main the collector's |
| Did each fix do what it was for? | Keys with a key word first: recall 0.760 → 0.808 (`seven-bounded` → `nine-mended`), on half the fresh set's keys; keys named after their target: 0.701 → 0.601, shortenings 0.275 → 0.217 | the drawn key-word-first styles help; the hard negatives cost the keys named after their target |

## 1. A blind judge of the keys no declaration names

Claude Sonnet 5.5 at medium effort, which taught none of `1s`,
`seven-restyled` and `seven-bounded` (it wrote only the teachers' schemas of
`cues-restyled-taught`), as Claude Code agents, one packet each
([judge.py](../round-1t/judge.py), [the prompt](../round-1t/judge-prompt.md)).
Every pair one of the three draws on the style set whose column no
declaration names (501), shuffled among up to three declared keys a database
(298) and up to three candidate pairs a database no model draws (288),
without saying which model drew a pair or which are controls; each database
shown as the tables its pairs name, with columns, types and primary keys and
no declared key ([judged-pairs.json](data/keys-mended-2026-11-09/judged-pairs.json),
[judge-count.json](data/keys-mended-2026-11-09/judge-count.json)):

| | yes | no | unsure |
|---|---|---|---|
| declared keys (controls) | 292 | 2 | 4 |
| candidate pairs no model draws (controls) | 0 | 285 | 3 |
| drawn by a model, no declaration | 352 | 138 | 11 |

| | drawn | declared | undeclared, the judge's yes | precision, declared | precision, with the judge | F1 with the judge [95 %] | minus `1s` |
|---|---|---|---|---|---|---|---|
| `1s` | 1 438 | 1 129 | 233 of 280 | 0.785 | **0.947** | 0.761 [0.670, 0.839] | |
| `seven-restyled` | 1 566 | 1 123 | 310 of 405 | 0.717 | **0.915** | 0.773 [0.679, 0.843] | +0.012 [−0.031, +0.051] |
| `seven-bounded` | 1 543 | 1 141 | 284 of 363 | 0.740 | **0.924** | 0.774 [0.680, 0.842] | +0.013 [−0.026, +0.051] |

The judge calls 98 % of the declared controls keys and none of the
candidates. Of the keys the models draw that no declaration names, it calls
seven in ten real: `accounts.userid` into `users`, Dolibarr's `fk_facture`
into `llx_facture` (Dolibarr declares its keys in files of their own; the
collector took one file). Counted with them, every model's precision is
0.92–0.95, and the newer models' F1 is 0.012 above `1s`'s, within its
interval. The keys still false: 76 of `1s`'s, 133 of `seven-restyled`'s,
118 of `seven-bounded`'s.

## 2. Two defects of the collector, and the set read again

**Primary keys the parser lost.** Of `1s`'s 314 keys outside the candidates
on the style set, 208 pointed into tables with no parsed primary key: T-SQL's
`PRIMARY KEY CLUSTERED (col ASC)` under PostgreSQL's dialect, tried first; a
`CONSTRAINT pk PRIMARY KEY (col)` with its comma missing; MySQL's
`col INT PRIMARY KEY AUTO_INCREMENT`; phpMyAdmin's `ALTER TABLE t ADD
PRIMARY KEY (id), ADD KEY …`. `fill_keys` reads them from the statements'
text for a table left with none (a table with no key in its file keeps
none: a key taken from a declared reference would hand the models the
answer). Tables without a key: 320 → 106 of 2 020; keys into them: 225 → 8.

**Types in families the training does not use.** The collector wrote `text`
as text, where round 1b's preparation of SchemaPile writes it longtext, and
`decimal`, `numeric`, floats, booleans and times as other: a `decimal` key,
which version 6 reads as an integer, was no candidate. 245 of the style
set's 1 791 key columns are `decimal`, 86 `text`. `training_family` writes
round 1b's families. Both fixes are off for every set written before them;
the three earlier GitHub sets were read with the first collector's types,
which hid their `decimal` keys too.

The same 100 schemas and the same keys, rewritten twice and read by the
same four frozen models — corrected readings of a seen set, no model chosen
on them ([heldout-github-style-fixed.json](data/keys-mended-2026-11-09/heldout-github-style-fixed.json),
[heldout-github-style-typed.json](data/keys-mended-2026-11-09/heldout-github-style-typed.json),
[githubstyle-corrected-sensitivity.json](data/keys-mended-2026-11-09/githubstyle-corrected-sensitivity.json)):

| F1 | as read on 9 November | primary keys fixed | and the training's families | the same, without `ETL_HTS` |
|---|---|---|---|---|
| `1s` | 0.700 | 0.709 | 0.707 | 0.750 |
| `seven-restyled` | 0.670 | 0.682 | 0.763 | 0.741 |
| `seven-bounded` | 0.685 | 0.694 | **0.779** | 0.756 |
| `seven-style` | 0.679 | 0.689 | 0.775 | 0.750 |
| `seven-restyled` minus `1s` | −0.030 [−0.060, −0.002] | −0.027 [−0.057, +0.000] | +0.056 [−0.040, +0.185] | −0.009 [−0.050, +0.040] |
| `seven-bounded` minus `1s` | −0.015 [−0.043, +0.012] | −0.015 [−0.042, +0.010] | +0.072 [−0.022, +0.204] | +0.006 [−0.032, +0.053] |

With the types as the training writes them the sign turns, but mostly
through one database: `ETL_HTS`, a data vault whose 182 keys are
`decimal(12)`, which the version 6 models read and `1s` does not. Without
it the four are level. The style set's −0.030 was then, in the main, the
collector's: `decimal` keys hidden, and real keys no declaration names.

## 3. One fit with every fix

`nine-mended` ([fit.py](../round-1t/fit.py)'s FITS), not a search; every part
written into the specification at 14:06 UTC before the fit:

- the second stage's leaves bounded, as `seven-bounded`'s;
- hard negatives from SchemaPile's training split: every ordinary column (no
  key of its schema, no key word first or last) that shares its first three
  letters with a candidate target's name or is a shortening of it, kept
  rather than sampled — 41 411, of them 37 267 not in the sample before
  (`looks_like`);
- the drawn styles gaining a key word first (`fk_`, `id_`, `fk_id_`, `cod_`,
  `codigo_`, `num_`) in 30 % of them, drawn from a generator of its own so
  every other draw is as `seven-bounded`'s ([restyle.py](../round-1t/restyle.py));
- version 9's cues ([roles.py](../round-1t/roles.py)): a lexicon Claude Haiku
  5.5 wrote once, offline, of 193 words naming a reference into a row's own
  table, 178 naming a creator or an owner and 184 naming a table of people,
  across twelve languages ([roles.json](../round-1t/roles.json)), read
  against the pair's target.

| | threshold | validation F1 / AP | SchemaPile test | renamed | Spider | BIRD | CTU | LiveSQLBench | pooled development |
|---|---|---|---|---|---|---|---|---|---|
| `1s` | 0.837 | | 0.736 | 0.515 | 0.869 | 0.830 | 0.609 | 0.185 | 0.700 |
| `seven-restyled` | 0.823 | 0.740 / 0.689 | 0.726 | 0.794 | 0.888 | 0.816 | 0.670 | 0.587 | 0.748 |
| `seven-bounded` | 0.818 | 0.749 / 0.779 | 0.734 | 0.809 | 0.891 | 0.812 | 0.654 | 0.619 | 0.745 |
| **`nine-mended`** | 0.822 | **0.762 / 0.794** | 0.732 | 0.797 | 0.880 | 0.815 | 0.663 | **0.542** | 0.741 |

([fit-nine-mended-graph-final.json](data/keys-mended-2026-11-09/fit-nine-mended-graph-final.json),
[summary.json](data/keys-mended-2026-11-09/summary.json).) The best validation
of the round, and LiveSQLBench 0.077 below `seven-bounded`: LiveSQLBench names
a key as a shortening of its target and a word of its own (`acct_mon` into
`accounts`), the very shape the hard negatives teach to refuse when it is no
key. That is a trade the fit made, read here and not mended: nothing was
tuned after it.

## 4. The fresh set, once

The specification's rule of 14:06 UTC (with the families of 14:11), applied
by `github.py parse-style fresh` after the four models were frozen at
14:34:57 ([frozen-fresh.json](data/keys-mended-2026-11-09/frozen-fresh.json)):
the style pool's repositories after its first 1 500 by SHA-256 fetched to
3 100, 113 passing, the first 100 taken
([github-fresh-summary.json](data/keys-mended-2026-11-09/github-fresh-summary.json),
[github-fresh-manifest.json](data/keys-mended-2026-11-09/github-fresh-manifest.json)):
2 042 tables, 2 016 one-column keys (113 composite left out), 1.5 % of the
key columns ending in `id`; 63 created before July 2025, the earliest in
2010. The set frozen at 14:51:38 and read once at 14:52
([heldout-github-fresh.json](data/keys-mended-2026-11-09/heldout-github-fresh.json)):

| | P / R / F1 [95 %] | minus `1s` | macro F1 | F1 at 0.5 | keys among candidates |
|---|---|---|---|---|---|
| `1s` | 0.881 / 0.635 / **0.738** [0.680, 0.792] | | 0.725 | 0.743 | 1 880 |
| **`nine-mended`**, the paper's model | 0.853 / 0.616 / 0.715 [0.635, 0.791] | **−0.023 [−0.062, +0.020]** | **0.775** | 0.723 | 1 994 |
| `seven-bounded` | 0.845 / 0.625 / 0.719 [0.647, 0.798] | −0.019 [−0.074, +0.032] | 0.783 | 0.711 | 1 994 |
| `seven-restyled` | 0.813 / 0.600 / 0.690 [0.601, 0.783] | −0.048 [−0.136, +0.024] | 0.777 | 0.695 | 1 994 |

**The mended model does not beat round 1s's over every key pooled: −0.023,
its interval across zero.** Over databases it does: macro F1 +0.050 [+0.016,
+0.092], and `seven-bounded` +0.058 [+0.022, +0.101]
([githubfresh-diagnosis.json](data/keys-mended-2026-11-09/githubfresh-diagnosis.json)).

Looked into after the reading, the set development data from then on
([errors-*--githubfresh.json](data/keys-mended-2026-11-09/),
[githubfresh-shapes.json](data/keys-mended-2026-11-09/githubfresh-shapes.json)):

- **Three databases make the pooled loss.** `1s` finds 213 keys
  `nine-mended` does not, and in three databases `nine-mended` finds 117
  fewer keys than `1s` in all: a gym's management system (257
  keys, `codusu` into `usuario`, `tipequipo` into `dpmggtipequipo`), a
  city's open data in German (173 keys, `hersteller` into
  `hersteller_versenkpoller`) and an accounting schema (82 keys,
  `fund_debit` into `fund_debit`, `purchase_order` into `purchase_order`).
  Without them, `nine-mended` is +0.017 [−0.003, +0.039] and `seven-bounded`
  +0.024 [+0.003, +0.050].
- **What each fix did**, recall by the shape of a key's name (shapes.py):

  | a key's name | share | `1s` | `seven-bounded` | `nine-mended` |
  |---|---|---|---|---|
  | a key word first (`id_x`, `cod_x`) | 48.5 % | 0.793 | 0.760 | **0.808** |
  | any other (shortenings, words) | 27.5 % | **0.309** | 0.275 | 0.217 |
  | the target's name | 15.0 % | **0.711** | 0.701 | 0.601 |
  | a key word last (`ref`, `key`, `code`) | 7.6 % | 0.658 | **0.882** | 0.862 |
  | ending in `id` | 1.5 % | 0.677 | 0.710 | 0.677 |

  The key-word-first styles did what they were drawn for, 0.760 → 0.808 on
  half the keys. The hard negatives cost what they resemble: a key named
  after its target or a shortening of it shares the target's first letters,
  as the ordinary columns they teach to refuse do, and those keys fall 0.10
  and 0.06. The drawn styles' gain on a key word last, 0.658 → 0.86–0.88,
  holds, on 7.6 % of the keys.

What this says for the paper: the repair's gain is real per database on the
fresh set (+0.050, interval over zero) and on the styles it was trained for,
and over every key pooled it is not shown on schemas of these styles: three
large databases of keys named after their targets carry the pooled F1 the
other way. Of today's fixes, the key-word-first styles and the bounded
leaves carry; the hard negatives, as written, cost more than they mend. No
fit is made after this reading.

**Continued** in [keys-final-2026-11-09.md](keys-final-2026-11-09.md): every other number the paper
reports for `nine-mended` — the reader in Rust, its size, see's questions, the GitHub sets read again with
the fixed collector, SchemaPile-Perm, Nexus, the bounds, database A, and the language models on the fresh set.

## Limits

- One judge, of the family that wrote the lexicons of `seven-restyled` and
  `nine-mended` (Haiku), though not the model that wrote them; its controls
  are easier than the pairs it judged, a declared key and a candidate no
  model draws.
- The corrected readings are of a set read before; they explain the first
  reading and prove nothing alone.
- The three earlier GitHub sets were read with the first collector's types;
  their `decimal` keys were no candidates for any model.
- One fit, one seed; its LiveSQLBench loss is development data, and no clean
  set in LiveSQLBench's style has been read.

## Environment and reproduction

| | |
|---|---|
| machine | AMD Ryzen 7 7700, 31 GB, NVIDIA GeForce RTX 5070, Windows 11 Pro |
| Python | 3.13.15; xgboost 3.4.1, lightgbm 4.7.0, scikit-learn 1.9.1, numpy 2.5.4; sqlglot for the collector in its own environment |
| language models | Claude Sonnet 5.5 (medium effort) as the judge, Claude Haiku 5.5 for the role lexicon, both as Claude Code agents, 9 November 2026 |
| research commit | `f74ea6f` and this round's files |
| times (UTC) | the judge's packets 14:01, its answers 14:02–14:04; corrected readings 14:05–14:10; the fresh rule 14:06 and 14:11; `nine-mended` fitted 14:15–14:27, scored by 14:34, frozen with the three others 14:34:57; the fresh set fetched 14:35–14:50, frozen 14:51:38, read once 14:52 |

```sh
cd <repo>/lemmesee
P=round-1b/.venv/Scripts/python
G=corpus/round-1t/.venv-parse/Scripts/python
$P -I round-1t/judge.py pack       # then one agent a packet, round-1t/judge-prompt.md; then:
$P -I round-1t/judge.py count
$G -I round-1t/github.py parse-style fixed
$G -I round-1t/github.py parse-style typed
$P -I round-1t/fit.py freeze-style fixed
$P -I round-1t/fit.py heldout "github style, fixed" frozen-style-fixed.json
$P -I round-1t/fit.py freeze-style typed
$P -I round-1t/fit.py heldout "github style, typed" frozen-style-typed.json
$P -I round-1t/fit.py fit nine-mended
$P -I round-1t/fit.py score nine-mended
$P -I round-1t/fit.py freeze-fresh models
$G -I round-1t/github.py fetch-style 3100
$G -I round-1t/github.py parse-style fresh
$P -I round-1t/fit.py freeze-fresh set
$P -I round-1t/fit.py heldout "github fresh" frozen-fresh.json
$P -I round-1t/shapes.py fresh    # after the reading
```
