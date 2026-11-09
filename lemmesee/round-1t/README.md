# Round 1t: keys in another house style

Written on 8 November 2026, before any fit of this round. Round 1s's model,
frozen and read once on LiveSQLBench-Base-Full-v1, reached F1 0.185 where
language models reached 0.896 to 0.924. Looked into afterwards, at the
owner's request to repair the model, the loss had two parts:

- **the threshold**: 0.837, set on SchemaPile's validation split; at 0.5 the
  same scores reach F1 0.453 on LiveSQLBench;
- **the house style**: 79 % of Spider's key columns end in `id`, 1 % of
  LiveSQLBench's do (27 % `ref`, 20 % `link`, the rest `_tie`, `_join`,
  shortened words); a key there is often named after its target's primary
  key (`zoneref` into `sites`, keyed `zoneregistry`), not its table, and a
  shortened word (`SEC_JOIN` into `SecurityProfile`) is not one round 1b's
  cues read.

LiveSQLBench is development data from that reading on. The questions here:

1. Do cues that read a name against its target's key, and a shortening of a
   word, without a list of styles, help where names are not `table_id`?
2. Does training on SchemaPile renamed into drawn house styles help, and
   does it cost anything on SchemaPile as it is?
3. Do schemas written by language models in named house styles add to that?
4. On sets nobody here has read — schemas on GitHub created after
   SchemaPile was gathered, and BEAVER's enterprise warehouses — does the
   chosen model beat round 1s's, and by how much?

## The model

Round 1s's refit `both` (seed 20261027, trigrams sorted) is the baseline,
`1s`, scored as saved. Every fit here is its procedure, `widen.fit`, seed
20261027: features once, five folds by repository for the first stage's
scores, the second stage on them, the threshold the validation split's best
F1 one target a column. What changes is listed in `fit.py`'s FITS:

| Fit | Cues | Training data |
|---|---|---|
| `cues` | version 5 (`cues.py`) | SchemaPile as round 1s |
| `restyled` | round 1s's | half of SchemaPile's training schemas renamed; every validation schema beside a renamed copy |
| `cues-restyled` | version 5 | as `restyled` |
| `cues-restyled-taught` | version 5 | as `restyled`, and the teachers' training and validation domains |

### Version 5's cues (`cues.py`)

Each from the pair's names and its schema alone, none from a list of house
styles: the longest common prefix of the column and the target's key over
each's length, and of the column and the target's name; the column's stem
equal to the key's stem, a stem being a name without the ending most of the
schema's keys share, or 15 % of its other columns; the stem a shortening of
the target's name or one of its words (its first letter, then its letters in
order); how many tables carry a column of the same name; how often the
column's last word ends a column, and a key, of the schema.

### Renamed schemas (`restyle.py`)

A drawn style shortens words (kept, first three to five letters, or the
consonants), joins and cases them (snake, camel, Pascal, upper, run
together), prefixes tables, and ends keys and references in words drawn from
25 (`id`, `key`, `no`, `code`, `ref`, `link`, `fk`, `ptr`, `uid`, `handle`,
`registry`, `sk`, `tie`, `join` ...), one for keys and one for references,
or one a reference; a reference is built from its own words, its target's
entity or its target's key. One shortening serves a word throughout a
schema. The declared keys are renamed with their columns. Each schema draws
its own style from a seed and its id; 0.5 of the training schemas are
replaced by their renamed copy, every validation schema gains one, and the
test split is renamed whole with another seed for the renamed test.

### The teachers' schemas (`teach.py`, `teacher.md`)

Claude Haiku 5.5 (24 agents) and Claude Sonnet 5.5 at medium effort (18
agents), asked as Claude Code agents, not through an API, on 8 November
2026: each agent eight (domain, house style) assignments, 336 domains of
every kind, none of LiveSQLBench's 22, twelve styles written for this round
(teacher.md). A schema is kept when its primary keys are among its columns
and at least one declared key resolves; domains are split 7:1:2 by a hash
into training, validation and test. Both teachers are of one family; that is
a limit of the set, kept because the owner asked that no Gemini be used.

## The measures

Over every declared one-column key, a key right when its column and its
table are, as round 1s: precision, recall, F1 with a 95 % interval from
1,000 resamples of databases (seed 20261101), macro F1 over databases, F1 at
0.5 beside F1 at the model's own threshold, and each fit minus `1s` on the
same resamples. On: SchemaPile's test split; the same, renamed; the
teachers' test domains; Spider, BIRD and CTU; LiveSQLBench (development).

**A fit is chosen** on SchemaPile's validation split and the development
sets above, before the held-out sets are read: the one with the highest F1
over Spider, BIRD, CTU and LiveSQLBench pooled whose SchemaPile test F1 is
within 0.01 of `1s`'s. It and `1s` are then frozen by hash and read once.

## The held-out sets

Chosen on 8 November 2026 before any of their schemas was read, by rules
fixed in the collectors' instructions:

- **GitHub, created from 1 July 2025** (`github.py`): repositories found by
  code search for SQL declaring foreign keys, created on or after 2025-07-01,
  not forks, not archived, not a copy of a well-known sample database; per
  repository the one `.sql` file at the default branch's head declaring the
  most keys; kept with at least five tables and three resolving keys; exact
  duplicates dropped; the first 80 by SHA-256 of `owner/name`. SchemaPile was
  gathered before that date, so none of these can be in it.
- **BEAVER** (`beaver.py`): the enterprise warehouses of arXiv 2409.02038 as
  published with their keys, pinned by revision.

The collectors report counts only; nobody looks at a schema before the one
reading. The language models are not asked on them in this round.

## What counts as an answer

A fit helps where it beats `1s` with its interval above zero on a set where
names are not `table_id`, and costs nothing where its SchemaPile test F1 is
within 0.01. The held-out reading is the only one that is not development
data; the report says so first.

## Added while the round ran

- **Three models read on the held-out sets, not two.** The rule above, run
  on the development sets, chose `cues`: the only fit within 0.01 of `1s`
  on SchemaPile's test split (+0.008). `cues-restyled` was 0.016 below `1s`
  there ([−0.025, −0.004]) and above it on the development sets pooled
  (+0.031 [+0.005, +0.060]), on SchemaPile renamed (+0.266) and on
  LiveSQLBench (+0.371). The 0.01 was a guess written before any number; the
  two fits trade off on development data, and choosing between them there
  would choose by a margin set blind. So `1s`, `cues` and `cues-restyled`
  are frozen by hash before the held-out sets are read, all three are read
  once, and all three are reported; nothing is chosen after the reading.
  `restyled` and `cues-restyled-taught` are not read: the first is below
  `cues-restyled` everywhere but nowhere by more than its interval, the
  second within its intervals of `cues-restyled` everywhere but the
  teachers' own test.
- **The teachers' test domains are easy**: `1s` reaches 0.921 on them, so
  they say little about another house style; the renamed test does.
- **BEAVER was read by the three frozen models** at 00:09 UTC on 8 November,
  as frozen (`frozen.json`), while the owner set the night's goal: a reading
  begun to be ready, interrupted, and found finished. It is reported as it
  came: `1s` 0.327, `cues` 0.394, `cues-restyled` 0.369, the candidates
  holding 230 of its 440 keys. BEAVER is seen from then on; the GitHub set,
  frozen by hash at 00:34 UTC before any of it was read, is the round's one
  clean set, kept for the model the night chooses.
- **The night of 8 November**, at the owner's goal of F1 0.80 on every
  benchmark with a model that carries, each change general and chosen on the
  development sets only:
  - *version 6* (`six.py`): `longtext` read as text (PostgreSQL's `text` as
    round 1b read it, 3.3 % of SchemaPile's test keys out of the candidates),
    `decimal` as an integer, a target through each column of a key of several
    columns and each unique column, across families where the names say so,
    and cues for tables that relate others; round 1r had tried the unique
    targets and decimal keys and lost precision, which this round found again;
  - *version 7* (`words7.py`): run-together words split by SchemaPile's
    training vocabulary, 999 shortenings expanded by Claude Haiku 5.5 agents;
  - *version 8*: 1 895 words mapped by Haiku agents to the entities a column
    so named refers to (author: user; pilot: driver);
  - *reliable negatives* (`six-restyled-pu`, `seven-restyled-pu`): a pair the
    name rules call a key that is not declared left out of training (a
    positive-unlabeled reading of SchemaPile's undeclared keys);
  - *the graph stage relative to its schema with shift-aware stacking*
    (`stack.py`): six features of a pair against its schema's scores, and the
    second stage fitted, for the renamed schemas, on a first stage that saw
    none of them;
  - *a has-a-key head* (`pi.py`), on frozen models;
  - *real schemas renamed by a language model* (`rename.py`): 400 SchemaPile
    training schemas renamed by Haiku agents in six semantic styles, the keys
    following the map, against the same 400 drawn into `restyle.py`'s styles
    once more as the control.
  Six design agents (Claude Opus 5.5 at extra-high effort) were asked for other
  architectures; their measured bounds on `cues-restyled`'s scores, a perfect
  per-schema threshold reaching CTU 0.764–0.777 and LiveSQLBench 0.631–0.654,
  say a threshold alone cannot reach 0.80 there. The rule above, unchanged,
  picks among the night's fits too: the best pooled development F1 within
  0.01 of `1s` on SchemaPile's test.
- **The GitHub set, once.** At 02:57 UTC on 8 November the rule's choice,
  `seven-restyled`, was frozen by hash with `1s`, `cues-restyled` and
  `seven-relative` (the night's best on LiveSQLBench) in `frozen-final.json`,
  the set's hash checked unchanged since 00:34 UTC, and the four read the set
  once (`fit.py heldout "github 2025" frozen-final.json`); all four are
  reported. `seven-restyled-pu` and `six-restyled-pu` were not fitted: the
  first ran out of memory beside other work, and the night's queue went to
  the fits above instead.
- **The morning of 8 November: a defect, and a second GitHub set.** Read
  key by key on CTU and LiveSQLBench (`errors.py`), `seven-restyled`'s
  second stage was found to hold 43 leaves of values over 50 (up to 15 832;
  `seven-relative`'s and `cues-restyled`'s stay under 11), which draw pairs
  the first stage scores under 0.001 at 1.0 (in AdventureWorks,
  `PasswordSalt` into `SalesPersonQuotaHistory`). On LiveSQLBench the model
  misses 161 of 299 keys and draws 33 false ones: keys named in one
  schema's own style (`acct_mon`, `acct_gate`, `acct_anchor` into
  `accounts`) each scored under the threshold. Two changes are tried on the
  development sets only, as before: a second stage whose leaves are bounded
  (`bounded second`), and features that read a pair against the other
  columns of its schema named alike (`style.py`). The rule above chooses
  among them and the night's fits, unchanged. **The second GitHub set**,
  written at 06:51 UTC before any of its schemas was read: the 177 schemas
  `github.py`'s rule kept and did not select (every one after the first 80
  by SHA-256 of `owner/name`), written by `github.py parse rest` into
  `heldout-github-rest/`, frozen by hash before any model is read on it, and
  read once by the rule's choice, `seven-restyled` and `1s`.
- **Before the second set is read** (frozen at 08:33 UTC): `seven-bounded`,
  `seven-relative-bounded`, `seven-style`, `seven-style2` (a name of one word
  with no lead word) and the blend `seven-bounded+seven-style` (`blend.py`)
  were fitted and scored on the development sets. The rule still chooses
  `seven-restyled` (pooled 0.748; `seven-relative-bounded` ties it at 0.748
  but is 0.017 under `1s` on SchemaPile's test). On CTU, the differences
  between fits are mostly three databases of a logic program's facts
  (Mooney_Family, SAT, Dunur): every key of one scores alike, so each is
  drawn whole or not at all, and `seven-restyled` drew Mooney_Family's 73
  through its runaway leaves (its keys' second-stage scores 0.79–0.85, at
  0.55–0.66 once the leaves are bounded). Read once on the second set, as
  frozen in `frozen-rest.json` before it is read: `seven-restyled` (the
  rule's), `1s`, `seven-bounded` (the defect mended) and `seven-style` (the
  best on LiveSQLBench, 0.671), all four reported, nothing chosen after.
- **A clean set in another house style**, written at 12:57 UTC on
  9 November 2026 before any of its schemas was read; the owner chose the
  source and the cut-off. Why: the round's gain where the style shifts stands
  on development data only (LiveSQLBench, SchemaPile renamed) and on BEAVER,
  read early by the first pass's models. The rule, applied by `github.py`:
  - *the pool*: the 28 045 repositories the first collector's code search
    found, created at any date, not forks, not archived, without the 400
    fetched for the two GitHub sets above and without every repository
    SchemaPile names (its `repo` field, compared case-insensitively);
  - *per repository* the same file, parser and thresholds as the GitHub set
    (steps 3 to 5 of `github.py`: the `.sql` file declaring the most keys at
    the default branch's head, at least five tables and three resolving
    keys, no copy of a sample database);
  - *dropped as well*: a schema with the table-name set of one of the 257
    the first rule kept, and a schema of which one SchemaPile schema holds
    80 % of the table names, three at least;
  - *the style filter, on names alone*: of the schema's declared one-column
    keys, at most 25 % have a column whose name, lower-cased with everything
    but letters and digits taken out, ends in `id`; no model's score is
    read by it;
  - *the set*: the first 100 that pass, by SHA-256 of `owner/name`. The pool
    is fetched in that order a prefix at a time, so the first 100 passing in
    a prefix are the first 100 of the pool; the collector reports counts
    only before the reading.
  The set is frozen by hash, then read once by `1s`, `seven-restyled` (the
  paper's model, the rule's), `seven-bounded` and `seven-style` as
  `frozen-rest.json` holds them, their hashes checked unchanged; no fit is
  made before the reading. Reported: precision, recall and F1 with 95 %
  intervals over databases, each minus `1s`, macro F1, F1 at 0.5, the keys
  the candidates hold, and the set's share of key columns ending in `id`
  beside LiveSQLBench's and Spider's.
  Done as written: the pool's first 1 500 by the hash fetched (300, then
  1 000, then 1 500), 108 passing, the first 100 taken; frozen with the
  models at 13:38 UTC (`frozen-style.json`) and read once
  (`fit.py heldout "github style" frozen-style.json`). Looked into after
  the reading, the set is development data from then on.
- **A fresh clean set and one mended fit**, written at 14:06 UTC on
  9 November 2026, before the fit below is made and before any of the set's
  repositories is fetched; the owner approved the plan as a whole.
  - *The set*: the style set's pool, filters and order exactly, with the
    parser's primary-key fallback on (`github.py` `fill_keys`, written after
    the style set's reading: T-SQL's `PRIMARY KEY CLUSTERED`, a key with its
    comma missing, MySQL's inline key, `ALTER TABLE … ADD PRIMARY KEY`), and
    without every repository fetched so far — the 400 of the two GitHub
    sets and the style pool's first 1 500 by SHA-256. The first 100 that
    pass, in the same SHA-256 order. Its repositories are fetched only after
    the new fit is frozen. *Added at 14:11 UTC, before the fit and the
    fetch*: the collector writes each column's type in the training's
    families too (`github.py` `training_family`, round 1b's `family`): the
    sets written before read `text` as text where SchemaPile reads it as
    longtext, and `decimal`, `numeric`, floats, booleans and times as other,
    so a `decimal` key, which version 6 reads as an integer, was no
    candidate (245 of the style set's 1 791 keys).
  - *The fit*: one, every fix together, not a search — the second stage's
    leaves bounded as `seven-bounded`'s; hard negatives from SchemaPile's
    training split (never the style set): ordinary columns that share a
    prefix or a shortening with a candidate target's name and are no key,
    all kept rather than sampled; the drawn styles gaining a key word first
    (`fk_`, `id_`, `fk_id_`, `cod_`, `codigo_`, `num_`); and a multilingual
    lexicon of roles that Claude Haiku 5.5 writes once, offline, as it wrote
    the earlier ones — words naming a reference to a row's own table
    (`jefe`, `chefe`, `boss`, `supervisor`, `parent`) and words naming a
    creator or an owner (`criador`, `create_by`, `owner`), read as cues
    against the pair's target. Haiku teaches it and judges nothing. A bug
    that breaks the fit is mended and said; nothing is tuned.
  - *What is decided now*: the new fit becomes the paper's model, because it
    mends known defects, whatever its development numbers; they are reported
    beside `seven-bounded`'s. `1s`, `seven-restyled`, `seven-bounded` and the
    new fit are frozen by hash, the set fetched, frozen and read once, and
    the reading reported whatever it is, beside every earlier reading, the
    style set's −0.030 included.
  Done as written: `nine-mended` fitted at 14:15–14:27 UTC, frozen with the
  three others at 14:34:57 (`frozen-fresh.json`); the pool fetched to 3 100
  by the hash, 113 passing after its first 1 500, the first 100 taken; the
  set frozen at 14:51:38 and read once (`fit.py heldout "github fresh"
  frozen-fresh.json`). Looked into after the reading, the set is
  development data from then on.
- **The style set judged, and read again with the fixed parser** (9
  November, development data): Claude Sonnet 5.5 at medium effort, which
  taught none of `1s`, `seven-restyled` and `seven-bounded`, judged blind
  every pair one of them drew that no declaration names, among declared
  keys and candidate pairs no model draws as controls (`judge.py`,
  `judge-prompt.md`); and the set rewritten with `fill_keys`
  (`github.py parse-style fixed`, the same 100 schemas and keys) was read by
  the same four frozen models (`frozen-style-fixed.json`), a corrected
  reading of a seen set, no model chosen on it.

## Run

```sh
P=round-1b/.venv/Scripts/python
$P -I round-1t/teach.py assign corpus/round-1t
# the agents write corpus/round-1t/teacher/<teacher>/*.json from the prompts
$P -I round-1t/teach.py collect corpus/round-1t
$P -I round-1t/fit.py fit cues restyled cues-restyled cues-restyled-taught
$P -I round-1t/fit.py score 1s cues restyled cues-restyled cues-restyled-taught
$P -I round-1t/fit.py summarize
```
