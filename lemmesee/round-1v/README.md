# Round 1v: the key model against the size of a schema

Written at 21:25 UTC on 9 November 2026, before any fit or scoring of this
round, at the owner's word to take the key model as far as it goes.

Round 1t ended with the paper's model, `nine-mended`, 0.079 to 0.135 of F1
below four language models over the 398 external databases and 0.134 to
0.196 below them on the fresh GitHub set, the gap recall, and with each of
its mends gaining on one shape of a key's name and losing on another. Its
saved errors, read again that evening by the number of tables of a schema
(scripts not kept; this round measures it with its own code, and those
figures are not quoted from here):

- on SchemaPile's own test split, the style the model was trained on,
  `seven-restyled`'s F1 falls with every step in size, from schemas under
  five tables to schemas of 40 to 100;
- on the fresh GitHub set the eleven databases of 40 tables or more hold
  two fifths of the keys and two thirds of `nine-mended`'s misses;
- about three in ten of those misses are keys a name settles alone: a
  column named exactly as the one-column primary key of one other table, or
  a name that holds its target's;
- `nine-mended` scores a column named as its target's key under 0.3 where
  `1s` scored it over 0.88 (58 keys of one database), once version 6's wider
  candidates gave the name four tables to point at.

The model scores a column against a table, one pair at a time; its second
stage reads nineteen sums of those scores over the schema. A person, and a
language model given the whole schema, read a name across the tables that
carry it. The questions:

1. **Is the size of a schema what the model loses to**, on its training
   style and off it? F1 by the number of tables for `1s`, `seven-bounded`
   and `nine-mended` on every set this work has read, the language models
   beside them where their replies are saved.
2. **Does reading the schema whole repair it**, with no new knowledge of
   words: the keys names settle taken first; a column's name read once for
   all the tables that carry it; the decision made a column name at a time
   rather than a pair at a time; a stage fitted on the schema's own surest
   keys?
3. **Do the declarations a model learns from thin out as a schema grows?**
   A key nobody declared is taught as no key; if large schemas declare a
   smaller share of their keys, the model is taught to doubt a large schema.
4. **On a clean set**, chosen by a rule fixed before any of its schemas is
   read and read once by frozen models: the round's choice against `1s` and
   `nine-mended`.

Nothing here asks the owner of a database anything: `see` shows a database
from nothing, and a key is drawn from the schema alone.

## Development data

Everything this work has read is development data here: SchemaPile's
validation and test splits and their renamed copies, Spider, BIRD, CTU,
LiveSQLBench, BEAVER and the four GitHub sets. A stage that learns is fitted
on SchemaPile's training split, from first-stage scores made out of fold as
round 1b's second stage is, or on its validation split where that is said;
never on any other set. The owner's databases are measured on, never
learned from.

## The measures

Round 1t's, unchanged: precision, recall and F1 over every declared
one-column key, a key right when its column and its table are; 95 %
percentile intervals of 1 000 resamples of databases (seed 20261101), a
difference resampling both arms together; macro F1 beside every pooled F1.
Added: each of them by the number of tables of a schema, in the bins under
5, 5 to 9, 10 to 19, 20 to 39, 40 to 99 and 100 or more.

A precision counted with a blind judge's answers stands beside the declared
one where a judge is asked, never in its place (round 1t's protocol).

## What counts as an answer

The round's choice helps if, on the clean set, its F1 over every declared
key is above `nine-mended`'s and above `1s`'s with both intervals over zero,
and its F1 in schemas of 40 tables or more is above theirs. It costs nothing
if its SchemaPile test F1 is within 0.01 of `1s`'s. Whatever the clean
reading says is reported, beside every development figure marked as such.

The model stays what plan.md allows: no language model and no inference
runtime when a schema is read, no rows read, and a file of about a megabyte
with its lexicons.

## Added while the round ran

- **The declarations thin out with size**, read at about 21:37 UTC on 9 November
  from SchemaPile's training labels and round 1's name rules alone, before
  any fit: of the columns the rules name a key (a column named after a table
  and `id`, the plainest keys there are), the schema declares 95 % where it
  has under five tables and 66 % where it has 40 to 100. A third of the
  plainest keys of a large training schema are taught as no key, and the
  first stage reads the number of tables. Two things follow, both fixed here
  before any fit:
  - *what is tried*: a training sample without the pairs most likely to be
    keys nobody declared — round 1t's reliable negatives (a pair the rules
    name, undeclared), and a second pass that leaves out the undeclared
    columns the first pass scores as keys out of fold;
  - *the guard*: a model taught so draws more keys no declaration names, in
    SchemaPile's large schemas above all, and its F1 over every declared key
    of SchemaPile's test falls for that alone. The guard "within 0.01 of
    `1s` on SchemaPile's test" is therefore read on the test schemas whose
    every rule-named column is declared (round 1b's `complete`), by size;
    the figure over the whole test split stands beside it.
- **No answers from the owner** (the owner, at about 21:38 UTC): a question to the
  owner was the first draft's third question; it was dropped before anything
  was run, since `see` is to show a database from nothing.
- **The first stage is fitted anew**, `ten` (first.py, from 21:39 UTC): `nine-mended`'s
  recipe without its hard negatives, which round 1t's last reading showed to
  cost the keys named after their target, and with version 10's cues
  (ten.py), written from the fresh GitHub set's misses, development data
  since its reading. Its scores out of fold are kept, so every second stage
  of the round is fitted on the same first stage; `ten` with round 1b's
  second stage is the control for what the second stage adds.
- **Two teachers of undeclared keys**, asked from 21:52 UTC on 9 November
  (teach.py), before any second stage of the round was scored on a
  development set: Gemini 3.5 Flash-Lite and Gemini 3 Flash through the
  owner's CLIProxyAPI, four requests in flight each, under round 1s's
  protocol and round 1p's whole-schema prompt unchanged, on SchemaPile's
  training and validation schemas only — those that declare keys and have
  ten tables or more (2 648 training, 323 validation), and the training
  schemas of five tables or more that declare none (2 094). What they are
  for: a column both name that no declaration names is most likely a key
  nobody declared; it is left out of the training sample's negatives, or
  taught as a key, and the round says which. Why these two: on the fresh set
  each alone reads precision 0.79 and both together 0.84 at recall 0.92,
  against Gemini 3.8 Flash's 0.88 and 0.94, which answers a schema of 40
  tables in one to four minutes where they take seconds. They are of one
  family; no teacher sees a test split or a held-out set, and a model they
  taught is never judged by them (teaching.md).
- **The teachers stopped early** (22:16 UTC): after about 4 900 requests
  the proxy answered 429 and then 503 for both models, the account's limit;
  both runs were stopped at once and not started again. Answered by both:
  every training schema of 20 tables or more that declares keys (1 038),
  every validation schema of 10 or more (323), and 1 011 of the 1 610
  training schemas of 10 to 19 tables; none of the schemas that declare no
  key. The fits below say which answers they were taught from.
- **A blind judge of what the teachers add** (judge.py, 22:18–22:20 UTC,
  before any taught fit was scored): Claude Sonnet 5.5 at medium effort as
  seven Claude Code agents, one packet each, round 1t's prompt unchanged;
  440 pairs drawn with seed 20261110 from the training schemas of 20 tables
  or more and the validation schemas of 10 or more. Of 220 columns both
  teachers point at the same table and no declaration names, it calls 202
  keys (9 no, 9 unsure); of 80 only one names, 34; of 70 declared keys, 69;
  of 70 pairs drawn at random, none. On those 1 359 schemas the two teachers
  agree on 23 013 keys no declaration names beside the 42 981 declared, so
  the declarations hold about two thirds of the keys there.
- **Taught fits** (first.py `ten-taught`), written at 22:17 UTC before the
  fit: a column both teachers point at the same table is added to its
  schema's declarations before the schema is renamed, so it is taught as a
  key and renamed as one; a column only one names, and a taught key no
  candidate reaches, is left out of the sample. The validation split is
  taught the same way, and its threshold is read twice: on the labels the
  model was fitted on, and on the declarations alone (a taught key drawn
  counts as false there). Every set the model is measured on keeps its
  declarations as they are; no teacher has seen any of them.
- **The clean sets and the choice**, written at 22:23 UTC on 9 November,
  before any schema of a clean set is parsed or read, before `ten-taught`
  is scored anywhere and before a choice is made.
  - *The choice*: among this round's fits — a second stage `graph` or
    `wide` on `ten`, `ten-taught` or `ten-taught-more`, a taught fit at
    either threshold — the one with the highest F1 over the four GitHub
    development sets pooled (452 schemas, 10 362 keys, as the fixed
    collector writes them), if its F1 on SchemaPile's complete test schemas
    is not more than 0.01 under `1s`'s; otherwise the next. It is frozen by
    hash with `1s`, `nine-mended` and `ten--graph` (this round's first stage
    under round 1b's second, taught by no teacher), and the four are read.
  - *github plain* (plain.py): round 1t fetched the style pool's first 3 100
    repositories by SHA-256 of `owner/name` and kept, for the style set and
    the fresh set, only schemas of which at most a quarter of the keys end in
    `id`; every schema that passed each other step of the rule and failed
    that one was counted and dropped, read by no model and by nobody. Those
    are this set's: the fresh set's rule with the style filter turned round
    (over a quarter of the one-column keys ending in `id`), both of the
    collector's fixes on, no table-name set of the four GitHub sets, one
    schema a table-name set. In SHA-256 order the schemas at even places are
    **plain A**, read once here; those at odd places are **plain B**, read by
    nothing in this round and kept clean for a later one.
  - *github style 3*: the fresh set's rule exactly, on the pool after its
    first 3 100 by SHA-256, fetched a prefix at a time until 100 pass; the
    first 100. Its repositories are fetched from 22:24 UTC and parsed only
    after the models are frozen.
  - Each set is frozen by hash before it is read; each is read once; after
    its reading it is development data. Reported for each: the measures
    above for the four models, each minus `nine-mended` and minus `1s`, by
    the number of tables, and a blind judge's answers on a sample of the
    keys the choice and `nine-mended` draw that no declaration names. The
    language models are not asked on these sets in this round: the proxy's
    limit for the two fast ones is spent, and the third serves the owner's
    production.
- **Out of memory, three times** (22:35, 23:03 and 23:40 UTC): two jobs
  building pairs at once, a fit beside a set's pairs with fourteen workers
  each, and the third time a reading of every frozen model's scores beside
  two fits, took the machine past its memory; the jobs lost were run again
  one at a time and nothing they had written was kept. The fetch for the third style set lost
  a worker to the first and was restarted at 22:43 UTC; it fetches only what
  is missing. A set's version 10 pairs are kept once since (first.py
  `pairs_of`), every first stage scoring the same rows.
- **What the taught fit read on the development sets** (23:00 UTC), before
  the fits below were made: against `ten--wide`, recall rises on every set
  and F1 over every declared key rises on CTU (0.662 to 0.757, its schemas
  of 40 tables or more from under a half found to nine in ten) and falls on
  three of the four GitHub sets, their pooled F1 0.774 against 0.792: the
  taught model draws keys no declaration names, `created_by` and
  `updated_by` above all, some into a table that only stands in for one the
  file does not hold. Under the rule above it is not the choice and it is
  under the guard. Three more ways of using the teachers were then written
  down, before any was scored:
  - `ten-unsure`: every column a teacher names and no declaration names
    left out of the sample, none taught as a key (the teachers used to
    doubt a label, not to teach one);
  - `ten--wide-taught`: the first stage as fitted on the declarations, the
    second stage alone taught (second.py `teach`), a renamed schema's taught
    columns left out since they were not renamed as keys;
  - a blend of `ten--wide` and `ten-taught--wide`, their second stages'
    scores averaged a pair (blend.py), at even weights and at 0.7 to 0.3.
  Each enters the rule of choice beside the others.
- **A name completed across its tables** (complete.py, 22:54 UTC): where
  half of the columns of one name whose best target is one table are drawn,
  the rest drawn too. On `ten--wide` it gains 0.011 on the fresh set, whose
  misses it was written from, and loses 0.001 to 0.004 on SchemaPile; not
  carried.
- **A judge of what the taught model draws** (judge.py `pack-drawn`, from
  23:06 UTC): on the fresh GitHub set, development data, every key
  `ten--wide` or `ten-taught--wide` draws that no declaration names (415),
  among 60 declared keys and 60 pairs drawn at random; Claude Sonnet 5.5 at
  medium effort, seven packets, round 1t's prompt.
- **What the judge said of the taught model's keys** (23:08 UTC, the fresh
  set, development data): of the 195 keys `ten--wide` draws that no
  declaration names it calls 111 keys; of `ten-taught--wide`'s 374, 226; of
  the 220 only the taught model draws, 124 — 91 of them one database's
  columns of keys of several columns, which no set here counts — and 89 no:
  a `creator` into `user_request` where the file holds no table of users,
  the columns of PostgreSQL's regression tests. Its controls: 60 of 60
  declared keys yes, 60 of 60 pairs drawn at random no.
- **Version 11, the people of a schema** (ten.py `add_people`), written at
  23:13 UTC before any fit of it, from the misses of `ten--wide` on the
  second GitHub set and the fresh set, development data: the largest group
  of keys the model misses there is a column named for someone
  (`created_by`, `author_id`, `owner`, `user_id`) into the schema's table of
  people (`users`, `profiles`), and the taught model's false keys are the
  same columns drawn into a table that only carries a person's word
  (`user_request`). A table is one of people by the last word of its name,
  by round 1t's lexicon; a column is someone's by the lexicon's actor words,
  a person word among its own, or a word for "by" at its end, of nine such
  words in as many languages that the round's agent (Claude Fable 5.2)
  wrote into ten.py. Two fits: `eleven`, on the declarations, and
  `eleven-taught`, taught from every answered schema, a column SchemaPile
  declares as one of a key of several columns not taught (1 752 of the
  27 591 taught keys are such). Both enter the rule of choice.
- **What the other uses of the teachers read, and three things fixed for
  the choice** (23:25 UTC, before any fit of version 11 was scored). On the
  four GitHub development sets pooled: `ten--graph` 0.793, `ten--wide`
  0.792, `ten-unsure--wide` 0.792 (0.794 at the threshold its declarations
  choose), the blend at 0.7 to 0.3 0.796, at even weights 0.793. The fits
  stand within 0.004 of each other, a tenth of any one's interval; the
  first stage's cues carried the gain over `nine-mended` (0.765), and no use
  of the teachers moves F1 over every declared key there. Fixed now:
  - *a blend is not the choice*: it is two first stages and two second
    stages, twice the megabyte plan.md allows a model; it is reported beside
    the choice. The rule chooses among single models;
  - *version 11 enters as* `eleven--graph`, `eleven--wide`,
    `eleven-taught--wide` at either threshold;
  - *a second seed is read beside the choice*: the choice's recipe fitted
    again with seed 20261028 for the sample of negatives and both stages'
    trees (first.py `ten-seed2`, `eleven-seed2`), frozen with it and read in
    the same one reading of each clean set, so that a difference between
    models can be held against what a seed alone moves. The owner asked at
    23:20 UTC that the round be finished and written up overnight.
- **The choice, and what is frozen** (00:16 UTC on 10 November, every
  development fit scored, no clean set parsed). The rule, over the single
  models: `eleven--wide`, 0.7965 over the four GitHub development sets
  pooled (precision 0.823, recall 0.772; `nine-mended` 0.7652, `1s` 0.7106),
  0.798 on SchemaPile's complete test schemas against `1s`'s 0.774. Behind it
  `eleven-taught--wide` 0.7964 at either threshold and under the guard
  (0.748), `eleven--graph` 0.7946, `ten--graph` 0.7930, `ten--wide` 0.7924,
  `ten-unsure--wide` 0.7918, `ten--wide-taught` 0.7887, `ten-taught--wide`
  0.7743; the blend of `eleven--wide` and `eleven-taught--wide` at 0.7 to
  0.3 reads 0.7991 and is two models. Every fit of the round but the first
  taught one stands within 0.008 of the choice, where one model's interval
  is 0.03 wide: the rule picks one of several that development data cannot
  tell apart, and the report says so. Frozen by hash before any clean set is
  parsed, and read together, once, on each:
  `eleven--wide` (the choice); `eleven-seed2--wide` (its recipe, another
  seed); `ten--wide` and `ten--graph` (the same without version 11, and
  under round 1b's second stage: what each part adds); `eleven-taught--wide`
  (the teachers' keys taught) and the blend at 0.7 to 0.3 (beside, as
  written above); `nine-mended` and `1s`. The choice is compared with
  `nine-mended` and with `1s`; the others are read beside it and compared
  with nothing after the fact. `ten-seed2` was not fitted: the choice is of
  version 11.
- **What a seed alone moves** (00:21 UTC, before the models were frozen):
  `eleven-seed2--wide` reads 0.7829 over the four GitHub development sets
  pooled against the choice's 0.7965, and `eleven-seed2--graph` 0.7869
  against `eleven--graph`'s 0.7946: a seed moves a fit by 0.008 to 0.014
  there, more than any two of this round's fits differ. Both seeds stand
  above `nine-mended` (+0.031 [+0.018, +0.046] and +0.018 [+0.003, +0.033]).
  Nothing is chosen again on it: the frozen list stands as written, and the
  clean sets are read by both seeds.
- **The clean sets, as the collector counts them** (00:22–00:24 UTC, the
  models frozen at 00:21:24; counts only, no schema looked at).
  From the pool's first 3 100 repositories 1 436 plain schemas pass:
  **plain A** 718 (14 756 tables, 16 399 one-column keys, 726 keys of
  several columns left out, 90.9 % of the key columns ending in `id`, 262
  created before July 2025; 248 schemas of 5 to 9 tables, 246 of 10 to 19,
  156 of 20 to 39, 56 of 40 to 99, 12 of 100 or more) and **plain B** 718
  (14 441 tables, 16 976 keys), which stays unread. From the next 1 600
  repositories 112 pass the fresh set's rule: **style 3** is the first 100
  (1 634 tables, 1 762 one-column keys, 15 of several columns left out,
  5.2 % ending in `id`, 63 created before July 2025; 38, 42, 12, 7 and 1
  schemas by the same sizes). The three hashes were frozen at
  00:23:43 to 00:23:45 UTC; plain A and style 3 are read once each
  (clean.py `read`), plain B by nothing.
- **Plain A's reading was started twice** (00:24 and 00:38 UTC). The first
  run stopped at the sixth model, the blend, on an import: round 1t holds a
  `blend.py` earlier on the path than this round's. It had drawn five
  models' keys in memory and had computed, printed and written no figure of
  any; clean.py now loads this round's `blend.py` by its path and lets this
  round's pairs go before the earlier rounds' models build theirs. The
  second run is the same frozen models on the same frozen set, and its
  figures are the reading's.
- **The readings** (plain A 00:38–00:57 UTC, style 3 00:57–00:58, once
  each, the eight frozen models; the report has every figure). Plain A: the
  choice 0.757 against `nine-mended`'s 0.702, +0.055 [+0.038, +0.074], and
  `1s`'s 0.658; the other seed 0.752; in schemas of 40 tables or more 0.651
  against 0.564. Style 3: 0.734 against 0.722, +0.011 [−0.003, +0.025]; the
  other seed 0.724. Both sets are development data from their reading on.
- **After the readings** (00:58–01:01 UTC): a blind judge of a sample of
  the keys the eight draw on each clean set that no declaration names
  (judge.py `pack-clean`: 400 of plain A's, 300 of style 3's, drawn with
  seed 20261110, among 60 declared keys and 60 random pairs; Claude Sonnet
  5.5 at medium effort, eight and six packets, round 1t's prompt); and
  `see`'s questions through the keys, round 1u's measure and code, a model's
  keys read from its kept scores (questions.py), `nine-mended` read the same
  way to check them against round 1t's report.

## Run

```sh
P=round-1b/.venv/Scripts/python
G=corpus/round-1t/.venv-parse/Scripts/python
$P -I round-1v/dump.py sets                       # the development sets, once
$P -I round-1v/dump.py score nine-mended          # and 1s, seven-bounded
$P -I round-1v/sizes.py reports/data/keys-size-2026-11-10/sizes.json
$P -I round-1v/thin.py reports/data/keys-size-2026-11-10/thinning.json
$P -I round-1v/first.py fit eleven                # a first stage: FITS in first.py
$P -I round-1v/first.py score eleven              # its scores on every development set
$P -I round-1v/second.py fit eleven wide          # a second stage: KINDS in wide.py, TAGS in second.py
$P -I round-1v/second.py score eleven wide
$P -I round-1v/table.py eleven--wide ten--wide    # scorings side by side
$P -I round-1v/choose.py eleven--wide ten--wide   # the rule of choice
$P -I round-1v/clean.py read plain-a              # refuses a set read before
```

Building pairs takes the machine's memory: one first stage and one second
stage at a time fit in 31 GB, two jobs building pairs at once do not. The
report has the whole order of commands.
