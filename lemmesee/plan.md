# lemmesee: the plan

**Status: written 27 October 2026; round 0 read, nothing run.** A round changes this file
when it settles something; a number lives in its report.

## The question

Given an unfamiliar PostgreSQL or MySQL, can a program suggest the views a
person would want — and refuse to suggest what it cannot know — while it

- adds at most a few hundred kilobytes to a binary and about a mebibyte to a
  process while it runs, with no language model, no Python and no inference
  runtime in production;
- costs the database next to nothing: its catalog and its own statistics,
  and a bounded look at rows only when that look is expected to change the
  answer;
- says when a guess is a guess, and keeps a meaning a person confirmed?

Three resources are spent at once — the binary and memory, the database's
work, and wrong suggestions — and the answer is a curve between them rather
than one score.

## What it is not

- **Not a business meaning.** It can tell that `created_at` is when a row was
  made and that `status` has four values; it cannot know which status is paid
  or that `amount` is in kopecks. A case whose honest answer is "cannot tell
  without the business" is part of every test set, and saying so is a right
  answer.
- **Not free SQL.** A generator writes only candidates a column's type allows
  — rows per day over a time, rows by a category with few values, a sum of a
  money-like number — and hard rules say what may be drawn; whatever learns
  only puts them in order.
- **Not trained on anyone's database.** Public corpora only; the databases of
  Dmitry's sites are measured on, never learned from, and are reported as
  counts without a table's or a column's name.

## Three questions

1. **From the schema and the database's statistics alone**, without reading a
   row, how good are the suggestions — for rules, a linear model and a small
   tree ensemble?
2. **When does a bounded look at rows improve them enough to be worth its
   cost**, and can a program decide that before it looks?
3. **How close to the best quality does a model of tens or hundreds of
   kilobytes come**, along the curve of quality against memory and against
   the database's work?

## The configurations compared

| | What it reads | What orders the candidates |
|---|---|---|
| A | the schema: names, types, keys, indexes | rules |
| B | and the database's statistics: estimated rows, nulls, distinct values | rules |
| C | the same as B | a small learned ranker: linear, gradient-boosted trees, a tiny network |
| D | and, when it decides to, a bounded sample of rows | the ranker, which also decides whether to look |

A and B are Dashbin's first `see`; C replaces its weights only if it earns it;
D comes last.

## The stores and corpora

| Kind | What | For |
|---|---|---|
| schemas | SchemaPile: 221 171 schemas, 1.7 million tables, 700 thousand foreign keys, data for more than 340 thousand tables; its permissively licensed part, SchemaPile-Perm | variety of names, types and relations, and some data |
| databases with data | Chinook, Pagila, AdventureWorks, a Stack Overflow dump, BIRD's databases | real runs, real statistics, the database's cost of each step |
| tables with charts | VizML's corpus, Table2Charts' | pre-training or comparison; a chart someone drew is not proof it was right |
| columns with types | GitTables, VizNet, SOTAB | what a column means, as a feature |
| a judged set of our own | 50 to 100 multi-table schemas unseen in training, 10 to 20 candidates each, judged by more than one person | the measure the rounds report |
| Dmitry's two production databases | schema and statistics only, read-only, nothing a user wrote | a check on the real case, reported as counts |

Every corpus's licence is read before it is used, and only what it allows is
trained on or published. A judged set is published only if its sources allow.

## How it is measured

- **Trained and tested on different projects**, by schema family, never on
  shuffled rows of the same tables.
- **Judged by people**: each candidate rated useful or not, and every kept
  guess checked for whether it means what its picture suggests; agreement
  between judges is reported.
- **Ranking**: NDCG@5 against the judgments, and how often a person would keep
  the top five.
- **Wrong meaning**: how often a kept suggestion means something false.
- **Abstention**: how often "cannot tell" was said where it was right, and
  where it was not.
- **Cost on the program**: inference time at p50 and p95, peak memory of the
  whole process, and what the binary grew by — weights alone are not the
  runtime.
- **Cost on the database**: time, rows read and the database's own work for
  each statistic and each sample, measured on PostgreSQL and MySQL, not
  inferred from the program's side.

## Lines set before the rounds

As engineering budgets, not results: the ranker's weights at most 256 KiB,
at most 1 MiB more memory while it runs, trained offline and carried as
weights read by a few hundred lines of Rust. Dashbin's own lines for shipping
it are in its chapter 14 and are the ones that decide.

## The rounds, in order

0. **What has been done before** — [read on 27 October](reports/literature-2026-10-27.md)
   from abstracts; ScaleViz (2024) already budgets which statistics to
   compute, so the second question is argued against it. The closest four are
   read whole next.
1. **Rules on the schema and statistics, A and B**, on the databases with data
   and on TinyStore's own stores, with a first judging pilot: how long judging
   takes a person, and whether two people agree.
2. **The judged set**: assembled, split by project, judged.
3. **The ranker, C**: linear, trees, a tiny network, against A and B on the
   judged set, with its cost.
4. **Looking closer, D**: the decision to sample, and what it costs the
   database for what it buys.

Each round is committed before it runs, in `lemmesee/`, and reported in
`reports/` with its environment, commands and raw output.

## What could be new

Not "recommending charts without a language model" — that is old. After
round 0, what may be: recommending from a schema and the database's own
statistics before reading a row, over several related tables, which was not
found; counting the cost on the live database and abstaining, beside
ScaleViz's budget of computed statistics; a ranker whose size and memory are
bounded, which was not found; and a judged set of multi-table schemas, if its
sources let it be published.
