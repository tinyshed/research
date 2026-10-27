# lemmesee: the plan

**Status: written 27 October 2026; round 0 and its follow-up reviewed, nothing run.**
A round changes this file when it settles something; a number lives in its report.
The [follow-up](reports/research-start-2026-10-27.md) narrows the novelty
assessment and proposes an acquisition contract and a rules/judging pilot.

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
- **Not trained on anyone's database.** Public corpora and rows generated for
  public schemas only; the databases of Dmitry's sites are measured on, never
  learned from, and are reported as counts without a table's or a column's
  name.

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
D comes last. B reads numbers about columns (B1); the values that statistics
hold, a column's commonest values and its histogram's bounds, are a variant of
their own (B2), read only on public and generated databases.

## The stores and corpora

| Kind | What | For |
|---|---|---|
| schemas | SchemaPile: 221 171 schemas, 1.7 million tables, 700 thousand foreign keys, data for more than 340 thousand tables; its permissively licensed part, SchemaPile-Perm: 22 989 schemas, 199 thousand tables, data in 7.1 thousand schemas at 29 values a column | variety of names, types and relations, and in round 1b its declared keys and enumerations as labels; too little data to give statistics |
| generated rows | rows for public schemas — SchemaPile-Perm's and the pilot's — written offline by a small language model such as Claude Haiku as a generator for each table, run with a seed at the sizes a round sets, loaded into PostgreSQL and MySQL | statistics the engines compute themselves at real sizes, traps whose true meaning is known, fixtures for the harness |
| databases with data | Chinook, Pagila, AdventureWorks, a Stack Overflow dump, BIRD's databases | real runs, real statistics, the database's cost of each step |
| tables with charts | VizML's corpus, Table2Charts' | pre-training or comparison; a chart someone drew is not proof it was right |
| columns with types | GitTables, VizNet, SOTAB | what a column means, as a feature |
| a judged set of our own | 50 to 100 multi-table schemas unseen in training, 10 to 20 candidates each, judged by more than one person | the measure the rounds report |
| Dmitry's two production databases | schema and statistics only, read-only, nothing a user wrote | a check on the real case, reported as counts |

Every corpus's licence is read before it is used, and only what it allows is
trained on or published. A judged set is published only if its sources allow.

**Generated rows are allowed**, decided on 27 October 2026: a language model
is used offline, as it may be anywhere outside the product, under these rules.

- **The model writes a generator, not the rows**: a short program for each
  table, whose sizes, nulls, skew, time order and whether rows are ever
  updated are set by the round and written down. The generators are kept with
  the round, because the model will not write them the same way twice; the
  rows are rebuilt from a seed, pinned by a hash, and never committed.
- **Names stay the schema's own.** A generator fills tables; it adds or
  renames no table and no column, except in a trap, which is marked as built.
- **Traps are built on purpose**: orders never paid, refunds in a table of
  their own, several currencies, `order_items` that multiply an order's
  amount, a `created_at` that is an import's time. Their true meaning is known
  by construction, so a panel's harm is checked there without a domain owner.
- **The statistics are the engine's, the values are the model's.** PostgreSQL
  and MySQL compute statistics with their own `ANALYZE`, so their shape and
  their cost are real; what the rows hold is the model's guess. Whatever is
  learned or tuned on generated rows is tested on real databases with data —
  the public ones, and Dmitry's as counts — before it is believed, never on
  rows from the same generators.
- **Never a judgment.** People judge what is useful; no model judges what is
  kept. A report names the model, its version and the prompt behind each
  generator.

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
   from abstracts, then [checked further](reports/research-start-2026-10-27.md)
   in method sections, code and engine documentation. Metabase's templates,
   SeeDB's multi-table setting and ScaleViz's cheap features narrow the
   comparison. A [wider search](reports/wider-reading-2026-10-27.md) adds
   twenty years of finding measures and dimensions in a schema, VizDeck's
   learning from keep and drop, prediction under a test-time budget for D, and
   that the column-type corpora's labels come from names. Full-paper reading
   remains open.
1. **Rules on the schema and statistics, A and B**, on the databases with data,
   with a first judging pilot: how long judging takes a person, and whether
   two people agree. [Specified](round-1/README.md) on 27 October: B split
   into B1, numbers about columns, and B2, the values inside statistics, on
   public and generated databases only; Metabase's X-rays and the schema's own
   order beside them; seven public databases, three generated, two built as
   traps; TinyStore's stores left to Dashbin's own round, and Dmitry's
   databases to a step after it.
1b. **Guessing what a schema did not declare**: a small model of column
   roles — which column is a foreign key and to which table, which is a
   closed list of values — learned from what SchemaPile-Perm's schemas
   declare, with the declarations hidden, against the rules' own guesses on
   the same schemas. It needs no judges, and it asks the third question early
   for roles rather than panels. [Run](reports/declarations-2026-10-27.md) on
   27 October: boosted trees of 967 KiB guess foreign keys at F1 0.65 against
   the rules' 0.45 on unseen kinds of schema and beat them at the rules' own
   precision; they are to be carried in Rust and measured there. Closed lists
   pass the line on a label too weak to carry them yet. A
   [second pass](reports/keys-2026-10-28.md) on 28 October took keys to F1
   0.774 against 0.442, and 0.875 on hard cases agents wrote, with a stage
   over the schema's graph and the trees trained on a graphics card; names
   and types stop near 0.8, and the statistics of the data come next.
2. **The judged set**: assembled, split by project, judged.
3. **The ranker, C**: linear, trees, a tiny network, against A and B on the
   judged set, with its cost.
4. **Looking closer, D**: the decision to sample, and what it costs the
   database for what it buys.

Each round is committed before it runs, in `lemmesee/`, and reported in
`reports/` with its environment, commands and raw output.

## What could be new

Not "recommending charts without a language model", using several tables,
beginning with cheap features, or learning from what people keep: these have
predecessors. The working
hypothesis is the combination of existing planner statistics, a compact
offline-trained ranker, explicit semantic abstention and measured work on
the source database. Its novelty is not established by this search. A judged
set may be useful if its sources let it be published, without a claim that
no related set exists.

[Round 1's contract](round-1/contract.md) says which numbers B can read on
each engine, keeps missing apart from zero, and counts discovery and
execution on separate ledgers; the values inside statistics are a
configuration of their own, B2. None of it changes Dashbin's shipping gates.
