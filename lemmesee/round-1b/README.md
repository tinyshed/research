# Round 1b: guessing what a schema did not declare

**Status: specified, run and [reported](../reports/declarations-2026-10-27.md)
on 27 October 2026**, at commit `a7a2f40`, after a dry run uncommitted and the
corrections said below. A round of
[the plan](../plan.md) that needs no judges: whether a small model, learned
from what public schemas declare, guesses two column roles better than round
1's rules do when the declarations are hidden. The report goes to
`../reports/declarations-<date>.md`; the harness is committed before the run
it reports.

## The question

Given only what tier S shows of a table when nothing is declared — names,
types, nullability, uniqueness, primary keys, indexes — which column is

1. **a foreign key, and to which table**; and
2. **a closed list of values**, which its designer declared as an `ENUM` or
   checked against a list?

Both matter where `see` is weakest: configuration A, MySQL without
histograms, and the many applications that declare no foreign keys at all.
A model that wins would enter `roles.rs` as evidence of its own kind,
`learned`; one that does not leaves the rules as they are.

## The data

SchemaPile-Perm, Zenodo record 12682521, version 0.2 of 7 July 2024 (CC BY
4.0; each schema under its own permissive licence), fetched by `fetch.sh` and
pinned in `hashes.txt`. As read on 27 October 2026:

| | |
|---|---|
| schemas, tables, columns | 22 989, 198 756, 1 330 024 |
| repositories they come from | 14 597 |
| schemas declaring a foreign key | 11 809 |
| foreign keys declared | 103 575; 99 808 of one column, every one to a table of its own schema |
| `Enum` columns; columns checked against a list | 6 139; 2 049 |

**Types are SchemaPile's parser's**, normalised and without length or scale:
`Varchar`, not `varchar(40)`; `Decimal`, not `numeric(10,2)`. Round 1's
typed evidence for money does not exist here. The rows SchemaPile keeps in
`VALUES` are not read.

**Creation times cannot be learned from it.** Its JSON keeps a function
default in 68 columns of 1 330 024; of 31 445 time columns named `create…`,
506 have any default at all. The default of `now()` that would label them is
lost in extraction, so this round leaves `created` out.

## Labels, and what they cannot say

- **Foreign keys, as pairs.** Only schemas that declare at least one foreign
  key are used: a schema declaring none says nothing about its keys. A pair
  is a column and a table of the same schema that has a one-column primary
  key of the same kind of type — integer, text or uuid — the column's own
  table included, except the column that is that key. A pair is positive
  when the schema declares the column a foreign key to that table. Keys over
  several columns, 3 767, are left out.
- **Closed lists, as columns.** Text and integer columns, keys and booleans
  excepted, are positive when declared `Enum` or checked against an `IN` or
  `ANY` list. A `status varchar` nobody closed is a negative here, so
  precision against this label is a lower bound of the truth.

## What the guesser may see

The schema with its declarations hidden: no foreign keys, `Enum` shown as
`Varchar`, no checks, no defaults, no comments. Names, types, nullability,
uniqueness, primary keys and indexes stay, as a real database shows them.
**MySQL makes an index for every foreign key**, so an index may be the
declaration's echo: every model is also run without the index, and both are
reported.

## The split

By repository, the `owner/repo` of each schema's URL: 70 % to train, 10 % to
choose thresholds on, 20 % to test, from a recorded seed. A test schema whose
signature — its sorted table and column names — also occurs in training is
dropped from the test, so a fork or a copied migration is not tested on
itself.

A test schema is **novel** when no training schema shares half its tables'
names (Jaccard under 0.5): the template schemas copied from repository to
repository — a framework's own tables, a tutorial's — are then measured
apart, and every measure is also given on the novel schemas alone. This was
added after the dry run and before the run reported.

## The contestants

| | What it is |
|---|---|
| **rules** | round 1's `roles.rs`, as committed, through a `guess` binary on the same hidden schemas: a foreign key where it guesses one, a closed list where a name says category. The dry run found two bugs in it, fixed before the run reported so that the models meet the rules at their best: a column of a primary key over several columns, as a link table's are, was never guessed a foreign key, and a plural in `-ies` was not read |
| **logistic** | logistic regression over hashed tokens of the column's, its table's, its neighbours' and the target table's names, with the type, nullability, uniqueness, key, index and position; 2¹⁵ weights |
| **trees** | LightGBM over the same features, at most 200 trees of 15 leaves |

The features of a pair add how the column's name meets the target table's:
the name minus `id` equal to the table's name or its plural, the shared
tokens, the column named as the target's key, the kinds of type.

## The measures, and the lines set before the run

- Precision, recall and F1 on the test at the threshold chosen on the
  validation split, and the area under the precision–recall curve; the
  model's precision at the rules' recall, and its recall at their precision.
- **A model replaces the rule for a role** only if its F1 beats the rules'
  with a 95 % interval, from 1 000 resamples of test repositories, that
  excludes zero — on all the test schemas and on the novel ones — and its
  weights fit what Dashbin's chapter 14 allows a model in the binary, 2 MiB.
  The plan's own 256 KiB is a research budget, which the plan says chapter 14
  overrules for shipping; each model is reported against both. The spec
  first named 256 KiB as the deciding line; it was set to chapter 14's after
  the dry run, as the plan has it, and both are reported.
- Weights in KiB as they would be carried: the logistic model's non-zero
  `f32` weights, the trees' text file, which a format of our own would
  shrink. The memory a model takes once read, against the plan's 1 MiB, and
  whether reading it stays under chapter 14's millisecond a column are
  measured when one is carried in Rust, not here.

## Not in this round

Creation and update times, which the data cannot label; anything from
statistics, which SchemaPile's few rows cannot give; reading a model in Rust.

## Files

| Path | What |
|---|---|
| `fetch.sh`, `hashes.txt` | the corpus, pinned |
| `requirements.txt` | the Python packages, pinned, for an environment of its own in `.venv/`, which git ignores |
| `prepare.py` | the columns, pairs and labels, the split, and the hidden schemas the rules are run on |
| `train.py` | the models, the rules' guesses read back, and the measures |
| `../round-1/harness/rules/src/bin/guess.rs` | the rules on hidden schemas |

Training runs offline on Python, as the plan allows; accuracy is not a
performance measurement, and any time quoted is an anecdote.
