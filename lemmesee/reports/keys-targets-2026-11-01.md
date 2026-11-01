# Keys into any unique column, and decimal keys — 1 November 2026

**The harness of this round, [round-1r](../round-1r/README.md), is at research
commit `e297347`.** Its specification was written before any variant was
fitted; nothing below changed it. Round 1p's widened model `both`
[as reported](keys-external-2026-11-01.md) is the baseline; round 1q
[found](keys-closing-2026-11-01.md) that keys into another unique column than
the primary key are what the pair task misses and what an owner's database
holds.

| Question | Answer | What follows |
|---|---|---|
| How many of SchemaPile-Perm's keys refer to another column than their target's one-column primary key? | 597 of 90,804 one-column keys whose target is in the schema, 0.66 %; 35 in validation, 77 in test. 3,673 point at a table with no primary key, 898 at one whose key spans several columns | the labels that name the referred column change little; the targets do the work |
| Which variant did the rule choose? | `both, unique, decimal`: F1 over every declared validation key 0.705 against `both`'s 0.691, recall at one 0.861 against 0.852. Each switch alone moved F1 by 0.001–0.002 | one fit each; round 1p's three seeds spread 0.016 in F1 over candidates, so a seed study comes before it is carried |
| Once on SchemaPile-Perm's test? | F1 over every declared key 0.736 against `both`'s 0.734 and round 1b's 0.719; the paper's task, column and referred column, 0.911 against 0.906 and 0.839; the hard cases 0.880 against 0.871 | the widening's own value, on data it was not designed on: +0.015 F1 and +0.067 in the pair task over round 1b |
| On Spider, BIRD and CTU, the paper's task? | 0.919, 0.906, **0.934** against `both`'s 0.913, 0.898, 0.886: CTU's 48 pairs with no candidate of the key's kind, its `decimal(11,0)` keys, are answered, and CTU now passes the constrained T5's 0.929 | — |
| And over every declared key? | **Worse**: F1 0.727 against 0.734, precision 0.824 against 0.845, recall 0.650 against 0.649 | the new targets cost precision in the open task; keep `both` for it |

## The labels

`targets.py referred` read SchemaPile-Perm's own record of each key's
referred columns, present on every one of its 93,784 foreign keys
([referred-summary.json](data/keys-targets-2026-11-01/referred-summary.json)).
Of the one-column keys whose target table is in the schema, 85,636 refer to
their target's one-column primary key, 597 to another column, 3,673 to a
table declaring no primary key and 898 to a table whose key spans several
columns. A candidate is now a key when its column is declared to point at its
table and at its key column; with one target a table, only the 597 change.

## Validation, the rule

F1 over every one-column key the validation schemas declare (7,633 distinct;
round 1p's 7,636 counted three declarations twice), and recall at one in the
paper's task on validation schemas, column and referred column right
([dev.json](data/keys-targets-2026-11-01/dev.json)):

| | threshold | F1 over every declared key | precision / recall | recall at one | covered |
|---|---|---|---|---|---|
| `both`, round 1p's, scored here | 0.809 | 0.691 | 0.710 / 0.673 | 0.852 | 0.878 |
| `both, labels` | 0.835 | 0.688 | 0.734 / 0.648 | 0.853 | 0.878 |
| `both, unique` | 0.862 | 0.693 | 0.736 / 0.654 | 0.857 | 0.880 |
| `both, decimal` | 0.856 | 0.692 | 0.748 / 0.644 | 0.855 | 0.882 |
| **`both, unique, decimal`** | 0.812 | **0.705** | 0.720 / 0.691 | **0.861** | 0.883 |

All four new variants pass the F1 condition; the rule takes the highest
recall at one. The pair of switches gains more than the sum of each alone,
+0.014 against +0.002 and +0.001. With one fit each and round 1p's seeds
spreading 0.016, that is not separated from a fit's luck; the round reports
it, and carries nothing on it.

## Test, once

On SchemaPile-Perm's test split, beside round 1b's model and round 1p's
`both` as saved, on the same split
([final-both-unique-decimal.json](data/keys-targets-2026-11-01/final-both-unique-decimal.json)):

| | F1 over candidates | F1 over every declared key (13,455) | the paper's task (12,013 pairs) |
|---|---|---|---|
| round 1b's model | 0.774 | 0.719 | 0.839 |
| `both` | 0.762 | 0.734 | 0.906 |
| `both, unique, decimal` | 0.762 | 0.736 | 0.911 |

No report gave these for the test split before: round 1p measured `both` on
it over candidates only, and its pair task only on validation. The widening
round 1p designed on the external databases' failures is worth +0.015 F1
over every declared key and +0.067 in the pair task on data it was not
designed on. The hard cases: F1 0.880 against round 1p's `both` 0.871.

## Spider, BIRD and CTU, once

The paper's task, recall at one with 95 % intervals over databases, and its
pairs left with no candidate
([score-1r-both-unique-decimal.json](data/keys-targets-2026-11-01/score-1r-both-unique-decimal.json),
[score-1r-both.json](data/keys-targets-2026-11-01/score-1r-both.json)):

| | Spider (768) | BIRD (381) | CTU (1,267) |
|---|---|---|---|
| `both` | 0.913 [0.871, 0.949] | 0.898 [0.814, 0.957] | 0.886 [0.814, 0.954] |
| `both, unique, decimal` | 0.919 [0.879, 0.957] | 0.906 [0.827, 0.960] | 0.934 [0.873, 0.970] |
| `both, unique, decimal`, two tables only | 0.918 | 0.898 | 0.945 |
| T5, constrained, round 1p | 0.957 | 0.934 | 0.929 |
| no candidate of the key's kind, `both` / new | 14 / 13 | 0 / 0 | 48 / 0 |
| the referred column no target's key, `both` / new | 21 / 18 | 27 / 23 | 37 / 37 |

Over every declared one-column key of the 398 databases that declare one
([every-key-1r-both-unique-decimal.json](data/keys-targets-2026-11-01/every-key-1r-both-unique-decimal.json)):

| | Spider | BIRD | CTU | all |
|---|---|---|---|---|
| `both` | 0.874 | 0.836 | 0.617 | 0.734 (P 0.845, R 0.649) |
| `both, unique, decimal` | 0.871 | 0.826 | 0.609 | 0.727 (P 0.824, R 0.650) |

The new targets answer the pair task and lose precision in the open one:
more candidates a column, the same number of keys found.

**`both` scored again** with `external.py`'s new choice of key — the key of
the row chosen, not of the target's first row — answers 2,632 of round 1p's
2,636 pairs with the same column. The 4 others take another column among
near-equal scores: one of BIRD's, right in round 1p and wrong now (0.900 then,
0.898 now), and three of CTU's, one now right, one now wrong and one wrong
both times. Round 1p's four scorings of one model already varied so
(0.8910–0.8914); its choice of key cannot move a pair while a target has one
key. The pair task's figures for `both` here are this run's.

## Limits

- One fit a variant; the rule's margin is inside round 1p's seed spread.
- The referred columns are SchemaPile's record of the DDL; a key declared
  without them names its target's primary key by SQL's default, and the
  record does not say which.
- `unique` makes a target of every declared one-column unique column, so a
  table with many has many targets, and the graph stage counts each as an
  arrow into the same table.
- `decimal` pairs a decimal column only with a decimal key; a key declared
  `decimal(11,0)` beside an integer column is still missed.

## Environment and reproduction

| | |
|---|---|
| research commit | `e297347` |
| models | round 1b's at `1524905`; round 1p's `both` in `corpus/round-1p/variants/both`; this round's in `corpus/round-1r/` |
| machine | AMD Ryzen 7 7700, RTX 5070 12 GiB, driver 610.88, Windows 11 Pro 26300 |
| Python | 3.13.15; XGBoost 3.4.1 (first stage on the GPU), LightGBM 4.7.0, scikit-learn 1.9.1, numpy 2.5.4, scipy 1.18.1 |
| run | `referred` before 12:40 UTC; `dev` 12:40–13:17; `final` and `external` 13:19–13:25 |

```sh
cd <repo>/lemmesee
round-1b/.venv/Scripts/python -I round-1r/targets.py referred corpus corpus/round-1b
round-1b/.venv/Scripts/python -I round-1r/targets.py dev corpus/round-1b corpus/round-1p/variants corpus/round-1r
round-1b/.venv/Scripts/python -I round-1r/targets.py final corpus/round-1b corpus/round-1r "both, unique, decimal" corpus/round-1p/variants corpus/round-1p/models-1524905
round-1b/.venv/Scripts/python -I round-1r/targets.py external corpus/round-1r "both, unique, decimal" corpus/round-1p
round-1b/.venv/Scripts/python -I round-1r/targets.py external corpus/round-1p/variants both corpus/round-1p
```

Raw output in [data/keys-targets-2026-11-01](data/keys-targets-2026-11-01/).
