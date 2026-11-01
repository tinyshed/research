# Round 1q: closing on the language models

**Status: specified on 1 November 2026, before any of its fits, and [reported](../reports/keys-closing-2026-11-01.md).**
A round of [the plan](../plan.md) that needs no judges.
Round 1p ([report](../reports/keys-external-2026-11-01.md)) left the widened
key model at F1 0.736 over every one-column key 398 public databases declare,
where Gemini 3.8 Flash given the same schema reaches 0.891 — at nearly the
same precision, 0.848 against 0.893, with three quarters of its recall.

## The questions

1. **Is the threshold the gap?** Round 1b chose it on SchemaPile-Perm's
   validation split, whose labels miss every key a schema did not declare.
2. **How far are the fast language models?** Gemini 3.5 Flash-Lite and Gemini 3
   Flash, through the same endpoint, on round 1p's prompts.
3. **Do features for keys named by their role help?** Round 1p found the
   largest losses on CTU's relational-learning databases, whose keys are named
   for the role a row plays — `brother.name1` → `person.name` — and on keys
   that carry the target's key name inside a longer one (`home_team_id` →
   `team.team_id`).
4. **How close does a cascade come?** The model draws the keys it is sure of,
   and asks a fast language model only about the candidates it is unsure of.

## 1. The threshold

`threshold.py`: the widened variant's F1 over every declared key at every
threshold from 0.05 to 0.95, on validation, validation's complete schemas and
round 1p's databases.

## 2. The fast models

`round-1p/gemini.py` with the model's replies in `corpus/round-1q/llm/<model>/`:
both of round 1p's prompts, unchanged, at temperature 0.

## 3. Keys named by role

A switch `roles` in `round-1p/widen.py`, off for every round 1p variant, adds
to each pair of a column and a target with key *k* (names normalised):

- the column's name less any trailing digits equals *k*, the name not being
  *k* itself (`name1` → `name`);
- *k* is inside the column's name less its digits, *k* not `id`;
- the column's name ends with *k*, with something before it;
- one over the number of targets whose key is named *k*;
- how many columns of the schema carry *k* in their name, out of twenty.

**The variant `both, roles`** is the chosen variant `both` with the switch on,
fitted as round 1p fitted it and measured on validation. **It is taken** if
its F1 over every one-column key the validation schemas declare (7 636) is
higher than `both`'s 0.690 by at least 0.003, and its recall at one in the
paper's task on validation is not lower than `both`'s 0.853 by more than
0.005. Then it is measured once on the test split, the hard cases and round
1p's databases. The features were written after round 1p's failure analysis
on those databases; this is said wherever its external numbers are.

## 4. The cascade

The model's one target a column, scored as round 1p scores it:

- a key scored at or above the model's threshold is drawn;
- a key scored in [0.2, threshold) is put to the language model as a question
  — does this column refer to that table? — all of a database's questions in
  one prompt with its schema written as round 1p wrote it;
- below 0.2 nothing is drawn or asked.

The band is fixed here, not tuned. Measured: F1 over every declared key, the
questions asked, and the prompt characters against round 1p's whole-schema
prompts, with each fast model and with Gemini 3.8 Flash.

## 5. Whose values: keys from the planner's own sample

Added on 1 November 2026, after the fast models were asked and before any of
this was run. A key's values are a subset of a unique column's values, and a
database already keeps a sample of every column for its planner: PostgreSQL's
`pg_stats` holds up to a hundred most common values with their frequencies,
up to 101 histogram bounds of the rest, the distinct count (−1 for a column
whose every value is distinct) and the null fraction. That is some two
hundred real values a column, read in milliseconds from the catalog, with no
row read. So inclusion can be tested on the planner's sample:

- `stats.py` computes, for Spider's and BIRD's SQLite files, what PostgreSQL's
  `ANALYZE` would keep at `default_statistics_target` 100: a sample of up to
  30 000 rows a table, the null fraction, the distinct count estimated as
  PostgreSQL estimates it, the most common values (those more frequent than
  1.25 times the mean, at most 100), 101 equi-depth bounds of the rest, and
  each column's least and greatest value. CTU follows when its rows can be
  sampled through the tunnel.
- `whose.py` reads only those statistics and, for every column and every
  column that is unique by them (a declared key or a distinct count of −1),
  measures how much of the column's sample falls inside the unique column's
  range and among its sampled values, and whether its distinct count fits
  the unique column's rows. No model is trained on it.
- **The rule, fixed here:** a candidate of the widened variant with roles
  scored in [0.2, threshold) is drawn when every sampled value lies within
  the target key's range and its distinct count is at most the target's rows;
  a key drawn above the threshold is dropped when under half its sampled
  values lie within the target key's range. Measured as round 1p measures,
  over every declared key, on the databases the statistics exist for, beside
  the model alone and the cascade.

**What the rule came to on Spider and BIRD** (251 databases with statistics,
1 236 declared keys): it drew 218 of the band's candidates and dropped 16 sure
keys; F1 0.837 against the model alone's 0.864, precision 0.816 against 0.916,
recall 0.860 against 0.816. The band held 88 true keys in 462 candidates;
inclusion in the key's range raised their share to 28 %, 49 % for text keys,
22 % for integer ones, whose range 1 … N holds any small integer. The rule is
reported as it was fixed, and fails.

## 6. Whose values, as a signal of its own

Added after section 5's result, before CTU's statistics exist. Inclusion in a
range says little; exact values say much where a key's sample covers its
domain. For a column *c* and a column *u* of another table that is unique by
its statistics or declared key, with *q* the share of *u*'s distinct values its
sample holds (sampled values over distinct count, at most 1) and *h* the share
of *c*'s sampled values found exactly among *u*'s:

- **a value key** is drawn from *c* to *u*'s table when *u* is text, *q* ≥ 0.5,
  *h* ≥ 0.9, *c*'s distinct count is at most *u*'s rows, and *c* is not itself
  unique — whatever the model scored it; each column keeps its best *h*, ties
  to the model's score;
- the thresholds are chosen on Spider and BIRD, written here before CTU's
  statistics are computed, and CTU is measured once with them, as the model
  alone, the model with value keys, and the cascade.

CTU's rows are sampled through the tunnel as `stats.py` samples SQLite's:
about 30 000 a table in one scan, by `RAND()`, the same fields kept.

**Chosen on Spider and BIRD**, before CTU's statistics were read: over a grid
of q in {0.3, 0.5, 0.8} and h in {0.7, 0.8, 0.9, 1.0}, value keys added 16 to
20 keys and moved F1 from the model alone's 0.864 to 0.866–0.867, the
thresholds barely mattering; q ≥ 0.5 and h ≥ 0.9 as written above, 0.867,
stand.

## 7. What CTU is measured with

Written on 1 November 2026, after the owner's database A and before any of CTU's statistics
were read by a measure. CTU's 142 databases are the held-out set for every
use of values; Spider, BIRD and database A are development sets from here on.

- **Value keys** (section 6), q ≥ 0.5 and h ≥ 0.9, added to the model's keys.
- **The catalog's drop**, section 5's second half alone, which database A
  suggested: a key the model draws at or above its threshold is dropped when
  under half of its column's planner-sample values lie within its target
  key's range; a key whose column or target has no sample is kept.
- **Both together**, and each beside the cascade with Haiku and the cascade
  with Haiku plus both.

Each is measured once on CTU, as round 1p measures, over every declared
one-column key of the databases whose statistics were sampled; databases
whose sampling failed are named by count and left out of every arm alike.
