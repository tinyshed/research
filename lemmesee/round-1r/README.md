# Round 1r: targets through any unique column

**Status: specified on 1 November 2026, before any variant was fitted.** A
round of [the plan](../plan.md) that needs no judges: round 1p's widened
model `both`, [as reported](../reports/keys-external-2026-11-01.md), given
more of the columns a key may point at.

## The question

In the paper's task — two tables known to hold one key, which columns —
`both` answers 0.913, 0.900 and 0.886 on Spider, BIRD and CTU against the
constrained T5's 0.957, 0.934 and 0.929, and what it misses it mostly
misses for want of a candidate. Of its 768, 381 and 1 267 pairs, round 1p's
`coverage.json` counts:

| | Spider | BIRD | CTU |
|---|---|---|---|
| the referred column is not the target's key | 17 | 6 | 3 |
| the target's key spans several columns | 3 | 11 | 26 |
| the target's key is not of a key kind | 6 | 0 | 48 |
| the kinds of column and key differ | 34 | 0 | 0 |
| the target declares no primary key | 1 | 10 | 8 |

Round 1q found the first row again on the owner's database A: of 36 keys
the models drew that A does not declare, 21 are links into another unique
column of the target than its primary key. The model draws such a key as a
column and a table, and then names the primary key as what it refers to.

1. Does a target for every unique column, beside the table's key, let the
   model name the column a key refers to, and close part of the gap to T5?
2. CTU's 48 are keys declared `decimal(11,0)`: does `decimal` as a key kind
   of its own reach them?
3. What does either cost in round 1b's task, every column of a database?

## The widenings

Each a switch on top of `both`'s two:

- **unique**: every one-column unique column of a key kind is a target of
  its own, beside the table's primary key or pseudo key; a table yields as
  many targets as it has such columns. Three features say what the target
  is: whether its key is not the table's primary key, whether the key's name
  (three letters or more, not `id`) is inside the column's name, whether the
  column's name ends with it.
- **decimal**: `decimal`, `numeric` and `money` are a key kind of their own;
  a decimal column is paired only with targets whose key is decimal.

**Labels name the referred column.** Round 1b's labels name only the target
table, which is the same thing while a table is one target. Here a candidate
is a key when its column is declared to point at its table and at its key:
the referred column SchemaPile records, or the table's primary key where it
records none. `targets.py referred` reads them from SchemaPile-Perm's file
into `referred.jsonl` beside round 1b's `hidden.jsonl`, and counts how many
one-column keys refer to a column other than their table's primary key.

**Four variants**, each fitted as `widen.py`'s `fit` fits `both`, on round
1b's training split, with the same seed and parameters:

| | new targets | labels |
|---|---|---|
| `both, labels` | none | the referred column |
| `both, unique` | every unique column | the referred column |
| `both, decimal` | decimal keys | the referred column |
| `both, unique, decimal` | both | the referred column |

`both, labels` separates what the labels do from what the targets do.

## The rule

On SchemaPile-Perm's validation split only, against round 1p's saved `both`
scored here:

- **F1 over every one-column key the validation schemas declare** (7 636),
  each column's best target at the variant's threshold, a key right when its
  column and its table are;
- **recall at one in the paper's task built on validation schemas**: every
  pair of tables with exactly one declared key from the first to the
  second, the column asked; right when the best-scored candidate between
  them has the declared column and the referred column.

**A variant is chosen** if its F1 over every declared key is no more than
0.005 below `both`'s and its recall at one is higher; among several, the
highest recall at one. If none is, `both` stays and the round reports a
negative result. The chosen one is then measured once on the test split, the
hard cases, and round 1p's 428 databases (Spider 206, BIRD 80, CTU 142). This
rule was written before any variant was fitted.

## The measures, once

- Round 1b's task on the test split, the hard cases and the external
  databases: precision, recall and F1, one target a column, at the chosen
  threshold; F1 over every declared key on the test split and the external
  databases.
- Beside it on the same test split, round 1b's model and round 1p's `both`
  as saved: F1 over candidates and over every declared key, and the paper's
  task built on test schemas — no report gives either for the test split.
- The paper's task on the external databases: recall at one, the column and
  the referred column both right, with 95 % intervals by resampling
  databases, and the first reason a pair has no candidate.

`external.py`'s paper task took the key of a target's first row; with one
target a table that is the key of every row, and with several it is not.
It now takes the key of the row chosen, and calls a pair unanswerable only
when no row's key is its referred column. For every variant of round 1p
the two are the same; `both` is scored again here to show it.

**What would be false:** if a unique column outranks the primary key where
the primary key is right, the paper's task falls on pairs `both` answered;
the report counts the pairs each answers that the other does not.

## Run

```sh
cd <repo>/lemmesee
round-1b/.venv/Scripts/python -I round-1r/targets.py referred corpus corpus/round-1b
round-1b/.venv/Scripts/python -I round-1r/targets.py dev corpus/round-1b corpus/round-1p/variants corpus/round-1r
round-1b/.venv/Scripts/python -I round-1r/targets.py final corpus/round-1b corpus/round-1r <variant> corpus/round-1p/variants corpus/round-1p/models-1524905
round-1b/.venv/Scripts/python -I round-1r/targets.py external corpus/round-1r <variant> corpus/round-1p
round-1b/.venv/Scripts/python -I round-1r/targets.py external corpus/round-1p/variants both corpus/round-1p
```
