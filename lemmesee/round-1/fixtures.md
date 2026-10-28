# The databases of round 1

Public databases with data, schemas from SchemaPile filled with generated
rows, two schemas built to mislead, and two with their names masked. Licences
were read on 27 October 2026 from each source's own page; **sizes and
versions are pinned when the harness first fetches them**, with a SHA-256 of
each archive in `fixtures/hashes.txt`. Nothing here is committed but the
fetch scripts, the hashes and the generators.

## Development fixtures

Three databases the rules are tuned by looking at, and so never pilot
fixtures: **Chinook** (P1 below), **Postgres Professional's airlines demo**
of 1 September 2025, three months (MIT; PostgreSQL; `fixtures/airlines.sh`;
bookings 1.3 million, segments 3.9 million, flights 21 758) and **ErgastF1**
as CTU's relational repository serves it (from ergast.com; MySQL, with a
histogram on every column; `fixtures/ergastf1.sh`; 14 tables, lap times
419 thousand). Their gold questions are in [gold/](gold/README.md). Chinook
leaves the pilot's ten: another public database takes its place there.

## Check fixtures

Three databases the rules are measured on once, before anything is changed
by looking at them, with gold questions an agent wrote blind
([gold/](gold/README.md#on-databases-the-rules-never-saw-28-october)); after
that they are development fixtures, and never pilot fixtures:
**Northwind** as pthom/northwind_psql ports it to PostgreSQL, at a pinned
commit (`fixtures/northwind.sh`; 830 orders, 2 155 lines), and two databases
from CTU's relational repository on MySQL with histograms (`fixtures/ctu.sh`):
**ccs**, a fuel-card company's customers, stations, a thousand transactions
and 383 thousand monthly consumptions, and **lahman_2014**, Sean Lahman's
baseball database to 2014, 25 tables keyed by season. Round 1c, built
looking at all six, made them development fixtures, and took two more from
CTU's repository as its final check, run once with nothing changed after:
**classicmodels**, a seller of scale models (326 orders, 2 996 lines, 273
payments), and **Credit**, a credit-card company's members, statements,
payments and 1.6 million charges. Round 1d looked at results on those
eight and on eleven more from the same repository, loaded by `ctu.sh` and
judged only by models: pubs, world, Dallas, restbase, voc, medical,
PremierLeague, Basketball_men, NCAA, Hockey, imdb_MovieLens, Student_loan,
CraftBeer, NBA, Basketball_women, Mondial, SFScores, Shakespeare,
Atherosclerosis and ftp.

## Public

| | Database | Licence | Engines | What it holds | Why it is here |
|---|---|---|---|---|---|
| P1 | [Chinook](https://github.com/lerocha/chinook-database) | MIT | PostgreSQL, MySQL | an online music store: artists, albums, tracks, customers, invoices and their lines; its sales randomly generated over four years | an invoice's total beside its lines, so fanout through a join occurs on its own |
| P2 | [Sakila](https://dev.mysql.com/doc/sakila/en/) | New BSD, for its schema and data files only | MySQL | DVD rentals: films, inventory, rentals, payments, staff, stores | the schema of P3 on the other engine |
| P3 | [Pagila](https://github.com/devrimgunduz/pagila) 4.0 | PostgreSQL License | PostgreSQL 18 and later | Sakila ported and grown to about 51.8 thousand rentals and 51 thousand payments, January 2022 to July 2026; `payment` partitioned by month | a partitioned table's statistics; not the same rows as P2, so the two are compared by contract, not by numbers |
| P4 | [Employees](https://github.com/datacharmer/test_db) | CC BY-SA 3.0 | MySQL, and a PostgreSQL port in the same repository | about 300 thousand employees and 2.8 million salary rows, 167 MB; salaries and titles valid from a date to a date | the largest fixture; periods whose sum over time means nothing; cancellation measured on it |
| P5 | [AdventureWorks for Postgres](https://github.com/lorint/AdventureWorks-for-Postgres) | the loader MIT; the data Microsoft's Adventure Works 2014 OLTP script from `sql-server-samples`, its licence read on fetch | PostgreSQL | 68 tables in 5 schemas | order headers and details, currency rates, statuses kept as small integers: three traps that occur on their own |
| P6 | [BIRD Mini-Dev](https://github.com/bird-bench/mini_dev) `financial` | CC BY-SA 4.0 | MySQL and PostgreSQL dumps, transpiled from SQLite | a Czech bank's accounts, clients, loans and transactions with a running balance | a balance that is a level, in types poorer than a designed schema's |
| P7 | BIRD Mini-Dev `codebase_community` | CC BY-SA 4.0 | both | a Stack Exchange site's posts, users, votes, comments and badges | event tables with real timestamps and many foreign keys |

The CC BY-SA sources bind whatever is published from them: a judged set that
carries their schemas is shared alike, or not published.

## Generated

**G1 to G3** are three schemas from SchemaPile-Perm, filled under the plan's
rule for generated rows.

- **Chosen** by annotated SchemaPile's domain label — a shop, a subscription
  or billing application, a community or forum — among schemas of 5 to 30
  tables with at least one `money` and one time column, after a seeded
  shuffle: the first that fits each domain, and every schema passed over is
  recorded with why. At least one of the three declares no foreign keys, and
  at least one declares enums or defaults.
- **Written for both engines** from SchemaPile's JSON — tables, columns,
  types, keys, defaults, checks — by a fixed type map, not from its SQL files,
  whose dialects are mixed.
- **Filled by generators a language model wrote**, one a table, a function of
  a row count, a seed and its parents' keys, run offline in the container.
  The round sets everything the model does not choose, from the declared
  schema rather than from the roles the rules infer:

  | Table, by its declared keys | Rows |
  |---|---|
  | referenced, with three columns or fewer besides its key | 10 to 50 |
  | referenced by others | 1 thousand to 10 thousand |
  | two or more foreign keys out | 100 thousand to 1 million |
  | any other | 1 thousand |

  Nullable columns get 0 to 30 % nulls, drawn per column from the seed;
  categories a Zipf-like skew; money a log-normal; times rise with the order
  of insertion over two to four years, with a weekly rhythm and growth.
- **A history is replayed**, because activity counters describe one: a table
  with a status, an `updated` or a `deleted` column has a share of its rows
  updated or deleted after they were inserted, as statements, so that
  `n_tup_upd` and `n_tup_del` are what a living application would leave.
- **Checked** before use: every foreign key resolves, every value fits its
  type, no table that should have rows is empty. The generators, the model
  and its version, and the prompt are kept in `generators/`.

## Built

**X1 and X2** are the two schemas of [traps.md](traps.md), written by hand
for both engines and filled the same way as G1 to G3, except where a trap
fixes a column's values.

## Masked

**K1 and K2** are P1 and P3 with every table and column renamed — `t1`,
`t1_c1` — and nothing else changed: types, keys, constraints, rows and
statistics stay. A candidate maps back to its unmasked twin through the
renaming, so it is judged once; what masking takes from A and B is the
difference in what they choose.

## States of statistics and privileges

| Engine | Statistics | How it is prepared |
|---|---|---|
| PostgreSQL | `fresh` | `ANALYZE` after the load and the history |
| PostgreSQL | `never` | autovacuum off for the fixture's tables before the load, and no `ANALYZE` |
| PostgreSQL | `stale` | `fresh`, then half as many rows again with autovacuum off |
| MySQL | `none` | the default: InnoDB's persistent statistics as computed, no histogram |
| MySQL | `histograms` | `ANALYZE TABLE … UPDATE HISTOGRAM` on every column of every table, 100 buckets |

| Privileges | Means |
|---|---|
| `full` | `SELECT` on every table |
| `partial` | one table without `SELECT`; on PostgreSQL also one column of another table |
| `rls` | PostgreSQL: one table with row-level security enabled and forced, for a role that does not own it |

Every state is prepared for P1 and P4 on both engines; the rest run
`fresh` or `none`, and `full`, except X2, whose protected table runs `rls`.
The report gives each fixture's states as run.

## What is judged

Ten fixtures are judged, each on PostgreSQL: P1, P3, P4, P5, P6, P7, G1, G2,
X1 and X2. A candidate is the same candidate on MySQL, in another state or in
another configuration, so it is judged once and its judgment reused; one that
appears only on MySQL, as B2 reading a histogram may make, is judged as it
appears. P2, G3, K1 and K2 are run and compared, not judged anew.

## Dmitry's databases, a step after this round

Not in round 1. When the harness has run on the public fixtures, a step of
its own, agreed before it runs:

- **A database role, not a shell**: a role with `CONNECT`, `USAGE` on the
  schemas and `SELECT` on the tables — which `pg_stats` needs to show a
  column — and nothing else, reached through an SSH tunnel to the database's
  port, its session read-only, at a quiet hour.
- **Tiers S and N only.** No values from statistics, no query text from tier
  Q beyond whether it is available. Every statement sent is logged on the
  client and matched with the server's own count, so that the log shows no
  row was read to recommend.
- **Drawn for Dmitry alone**: candidates are drawn on his machine for him to
  judge, T7 never; nothing drawn is kept.
- **Reported as counts**: features by state, roles, candidates by template,
  hard rules and abstentions that fired, kept and harmful by his judgment.
  No table's or column's name, no value. Snapshots and decisions stay in an
  ignored directory on his machine, and addresses, hosts and credentials are
  never written into this repository.
