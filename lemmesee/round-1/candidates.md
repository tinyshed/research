# From a snapshot to five panels

How round 1 turns what [contract.md](contract.md) read into candidates, which
of them may be drawn, and in what order. **Every threshold and penalty here
is set before the run and is the round's choice, not a finding**; the pilot
judges them as written, and round 2 tunes them on a set held apart.

## The steps and their documents

```
snapshot.json ─▶ roles ─▶ candidates.json ─▶ scores-<config>.json ─▶ panels-<config>.json ─▶ decisions.jsonl
   contract        │            one generator          hard rules,          top five             judges
                   └─ evidence for each role           penalties
```

Each document is JSON with a `version` and the hash of the snapshot it came
from. A candidate's `id` is a hash of its template, tables, columns,
aggregate and join, so the same candidate proposed by two configurations, or
on two engines, is one candidate and is judged once.

- `snapshot.json`: the engine and version, the role, the settings recorded
  (`default_statistics_target`, `information_schema_stats_expiry`), and every
  table, column, constraint, index and privilege with its features as
  `{value, state, source, as_of}`.
- `candidates.json`: each column's roles with their evidence, and each
  candidate: template, tables, columns, join, aggregate, its SQL for
  PostgreSQL and for MySQL, chart, title, reasons, flags.
- `scores-<config>.json`: for each candidate, the hard rule that removed it
  or its penalties item by item, their total and its rank.
- `panels-<config>.json`: the five chosen, in order.
- `decisions.jsonl`: one line per judge and candidate, from
  [judging.md](judging.md).

## Roles

A role is what a column or a table probably is, with the **evidence** that
says so, each item naming its tier: `declared` (a key, an enum, a `CHECK`, a
default, `ON UPDATE`), `named` (tokens of the name, matched case-folded on
`_`, case changes and digits), `typed`, or `measured` (tier N or V). A role
held only by `named` evidence is weak. Names are matched in English; a schema
in another language falls back on types, declarations and statistics, and
the report counts how often.

### Columns

| Role | Evidence that gives it |
|---|---|
| `created` | declared: a default of `now()`, `CURRENT_TIMESTAMP` or `clock_timestamp()`; named: `created`, `inserted`, `registered`, `signup`, `joined` on a time type; measured: on PostgreSQL, `correlation` ≥ 0.9 |
| `updated` | declared: MySQL's `ON UPDATE CURRENT_TIMESTAMP`; named: `updated`, `modified`, `changed` |
| `event` | named: `at`, `date`, `time`, `timestamp`, `occurred`, `ordered`, `paid`, `shipped`, `sent`, `started` on a time type, when not `created` nor `updated` |
| `period` | named pairs: `from`/`to`, `start`/`end`, `valid_from`/`valid_to` in one table |
| `calendar` | named: `birth`, `dob`, `birthday`, `year`; a date that describes a thing, not when a row happened |
| `deleted` | named: `deleted`, `removed`, `archived` on a time or boolean type |
| `pk`, `fk` | declared keys; `fk` also guessed when `<table>_id` or `<table>id` names a table whose primary key is `id` of a compatible type, kept as `fk-guessed` |
| `identifier` | named: `id`, `uuid`, `guid`, `key`, `hash`, `token`, `code`, `sku`; typed: `uuid`; measured: `n_distinct` = −1 on a non-time column |
| `category` | declared: an enum type, a MySQL `ENUM`, a `CHECK … IN (…)`, a boolean; measured: a positive `n_distinct` of 30 or fewer, or common values covering 95 % of rows in 30 or fewer; named, weak: `status`, `state`, `type`, `kind`, `category`, `level`, `role`, `tier`, `plan`, `country`, `source`, `channel` |
| `currency` | named: `currency`, `currency_code`, `ccy`; typed: `char(3)` beside a `money` column; measured with V: three-letter values |
| `money` | typed: `numeric` or `decimal` with scale 2 or 4, or `money`; named: `amount`, `price`, `total`, `subtotal`, `cost`, `fee`, `charge`, `tax`, `discount`, `paid`, `salary`, `wage`, `revenue`, and integers named `*_cents`, `*_kopecks` |
| `quantity` | an integer named `qty`, `quantity`, `count`, `num_*`, `*_count`, `units`, `items`, `seats` |
| `level` | named: `balance`, `stock`, `inventory`, `remaining`, `level`, `on_hand`; or any measure in a `snapshot` table |
| `ratio`, `score` | named: `percent`, `pct`, `rate`, `ratio`, `share`; `rating`, `score`, `stars`, `rank`, `priority` |
| `place`, `code` | named: `lat`, `lng`, `lon`, `latitude`, `longitude`; `zip`, `postal`, `phone` |
| `text` | typed: text or long `varchar`; measured: `avg_width` above 40, or `n_distinct` near −1; named: `name`, `title`, `description`, `email`, `url`, `comment`, `note`, `body` |
| `number` | any other number |

A column can hold several roles; the most declared wins, then the most
measured, then the first in this table.

### Tables

| Role | Evidence |
|---|---|
| `fact` | two or more foreign keys out, declared or guessed, and a `money`, `quantity` or `event` column |
| `event` | a `created` or `event` time and no `updated` column; measured: no updates or deletes while rows were inserted; named: `log`, `event`, `history`, `audit`, `activity` |
| `entity` | referenced by two or more foreign keys, with a `created` time |
| `snapshot` | a unique key over a foreign key and a date |
| `link` | two foreign keys and nothing else but keys and times |
| `lookup` | referenced, at most three columns besides its key; measured: under 100 estimated rows |

## Templates

The generator writes every candidate a template allows from the roles, the
same for every configuration. Aggregates are `count(*)`, `sum` and `avg`,
nothing else.

| | Template | Shape | Drawn as |
|---|---|---|---|
| T1 | rows over time | `count(*)` by bucket of a `created` or `event` time | line, or bar under 20 buckets |
| T2 | rows by category | `count(*)` by a `category`, or by a lookup's label through one many-to-one join; 12 largest and the rest | bar |
| T3 | measure over time | `sum` of `money` or `quantity`, or `avg` of `money`, `quantity`, `score`, `ratio`, by bucket of a `created` or `event` time | line |
| T4 | measure by category | the same aggregates by a `category` | bar |
| T5 | distribution | a `money`, `quantity` or `score` column in bins; or child rows per parent through a foreign key | bar |
| T6 | rows over time by category | T1 split by a `category` of at most 6 values | stacked bar |
| T7 | latest rows | the last 10 rows of an `event` table by its time, at most 5 columns, no `text`, no `identifier` | table, flagged `shows-rows` |
| T8 | largest tables | the estimated rows of the 10 largest tables, from tier N | bar, titled as an estimate |

**Joins** go only from the table whose rows are counted or summed to the one
side of a many-to-one foreign key, at most one join, to bring a label to group
by or a time to bucket by: items counted by their order's time are each
counted once. A guessed foreign key may be joined, flagged `join-guessed`.

**A time bucket** is chosen when the candidate is drawn, from the range its
own query returns — day, week or month for about 60 buckets — on the ledger
of execution, never at discovery.

## Hard rules

A hard rule removes a candidate; its code is kept in the scores. A rule that
needs a feature which is not `present` cannot fire, and the candidate stays
with the penalty for not knowing.

| | Never |
|---|---|
| H1 | a `sum` or `avg` of a `pk`, `fk`, `identifier`, `code`, `place`, or a `calendar` year |
| H2 | a `sum` of a `level`, `ratio` or `score`; a `sum` over time of anything in a `snapshot` table |
| H3 | a group by anything but a `category`, and never by a column measured above 30 distinct values |
| H4 | an aggregate of a column after a join that can repeat its row |
| H5 | a `sum` or `avg` of `money` in a table with a `currency` column, unless grouped by it |
| H6 | a time axis but a `created` or `event` time |
| H7 | a panel of its own from a `lookup` or `link` table, but T5's children per parent and T8 |
| H8 | anything from a table estimated at no rows; an estimate of `-1` is unknown, not empty |
| H9 | a title that names a meaning: no "revenue", "sales", "users", "active" |

## Abstaining and flags

**An abstention removes a candidate from the top five and says why**, and the
candidate is still judged in the pool, so the report can say how often
abstaining was right. A flag keeps it, with a penalty and a note shown beside
the panel.

| Code | Kind | When |
|---|---|---|
| `needs-population` | abstain | T3, or a `sum` in T4 by another column, over a table that has a `category` named `status`, `state`, `type` or `kind`: which values count is the business's. T4 by that category is offered instead |
| `currencies` | abstain | `money` whose table, or the one side of its join, has a `currency` column |
| `level-over-time` | abstain | a `level` in T3 outside a `snapshot` table, where H2 could not tell |
| `stats-missing`, `stats-denied`, `stats-stale` | flag | a feature the candidate needed was in that state |
| `join-guessed` | flag | its join follows a guessed foreign key |
| `soft-deleted` | flag | its table has a `deleted` column; it counts every row, deleted or not, and says so rather than filtering |
| `time-ambiguous` | flag | its table has more than one `created` or `event` time |
| `time-clumped` | flag | its time axis has a measured positive `n_distinct` under 1 % of the table's estimated rows: many rows share few moments, as an import's do; +3 rather than +1 |
| `shows-rows` | flag | T7 |

No unit is ever drawn on a guessed panel: `amount` is a number, not roubles.

## The order: penalties

Lower is better; each candidate starts at 0. The penalties are integers and,
as with Draco's, only their ratios mean anything.

| What | Penalty |
|---|---|
| a role it uses held by `named` evidence only | +2 each |
| a role it uses `declared` or `measured` | 0 |
| a name that contradicts its type, such as `amount` on an integer with no `_cents` | +1 |
| its time measured in physical order, `correlation` ≥ 0.9 | −1 |
| its table is a `fact` / an `event` / an `entity` for T1 / any other | −2 / −1 / 0 / +1 |
| estimated rows under 100 / under 1 000 / unknown | +3 / +1 / +1 |
| a category of 3–12 values / of 2 / of 13–30 / of unknown size, not declared | 0 / +1 / +1 / +1 |
| a column with `null_frac` above 0.5 | +2 |
| a category whose commonest value covers over 95 % | +3 |
| a measure that is `money` / `quantity` / `score` or `ratio` / `number` | 0 / +1 / +1 / +3 |
| a declared join / a guessed join | +1 / +2 |
| template T1, T2 / T3, T4, T8 / T5, T6, T7 | 0 / +1 / +2 |
| each flag but the `stats-*` ones | +1 |

**The top five** are filled greedily from the lowest total, adding +3 for
each panel already chosen from the same table and +5 for one of the same
template on the same column, and never more than two from one table. Ties go
to O's order, so every run gives the same five.

In A, every feature of tier N is unknown and takes the penalty for not
knowing; in B1 it is read; in B2 the values of tier V may confirm a
`currency` or a `category` and fire H5 or `currencies` where B1 could not.

## Titles and reasons

A title says what is computed and over what, never what it means:
`sum of amount by created_at, per day`, `rows of orders by status`. Every
guessed panel is marked `guessed = true`, as chapter 11 has it. **A reason is
one plain line from the evidence**, as chapter 9 asks of a suggestion:
"created_at defaults to now() and follows the rows' order; orders has two
foreign keys out and an amount". The judge sees it; it is part of what is
judged.

## The other two orders

**O** takes the candidates that survive the hard rules needing only tier S,
in the schema's order: tables as the catalog lists them, columns in their
declared position, templates T1 to T8. It is the floor a ranking must beat.

**M** is Metabase's X-ray at a pinned release, connected to the same fixture
with the same read-only role: the tables it ranks for a database's X-ray,
where its release has that ranking, each table's cards in its dashboard's
order, at most two a table, until five. Each card's query, compiled to SQL by
Metabase, becomes a candidate of the closest template, or of `M-other`, and
is judged in the same pool. Its reading — the first 10 000 rows of each table
— is counted on the discovery ledger like ours. How M's top five are taken
is the round's choice, said in the report.

## Confirmed meanings, reserved

Round 1 does not confirm meanings, but its documents leave room for one:
`{instance, schema, table, column, type, evidence_hash, meaning, scope,
confirmed_by, at}`, where `evidence_hash` covers the column's name, type and
constraints, so that a renamed, retyped or reconstrained column loses its
meaning and is asked again.
