# What round 1 may read, and what reading costs

Every feature the rules use comes from a query below, carries a state, and is
counted on the database that answered it. **This is the contract the harness
is built to**; it was written from the documentation of PostgreSQL 18 and
MySQL 8.4, then checked against both servers with Chinook, and corrected
where they disagreed — [what the check found](#checked-against-the-servers)
is at the end. The queries themselves are in
`harness/snapshot/queries/`.

## Rules for every query

- **A read-only session**: on PostgreSQL `default_transaction_read_only`,
  `statement_timeout` and `lock_timeout` set for the session, so that a
  failed query does not abort the ones after it as it would inside one
  transaction; on MySQL `SET SESSION TRANSACTION READ ONLY` and the session's
  `max_execution_time`, which bounds every `SELECT` as the hint would.
  Nothing persistent is set on the server.
- **Nothing is refreshed**: no `ANALYZE`, no `ANALYZE TABLE`, no histogram
  made or updated. Creating statistics is the fixture's preparation, never
  discovery.
- **Named columns only**, never `SELECT *` from a statistics view, so that a
  value-bearing column is never fetched by accident.
- **What carries values stays on the server** unless the tier allows it: a
  count, a length or a frequency is computed in SQL, and only the number
  crosses the wire.
- **One query a feature group a database**, over all tables at once, rather
  than one a table, so the cost does not grow with the number of round trips.

## A feature's states

Each value is stored as `{value, state, source, as_of}`: `source` names the
query below, `as_of` is the statistics' own time where the engine has one.

| State | Means | How it is told apart |
|---|---|---|
| `present` | the engine has it and this role sees it | — |
| `absent` | the engine could have it and does not | PostgreSQL: no `pg_stats` row, and `last_analyze` and `last_autoanalyze` both null; MySQL: no `COLUMN_STATISTICS` row |
| `denied` | the engine has it and this role may not see it | PostgreSQL: `has_column_privilege(…, 'SELECT')` false, or `row_security_active(oid)` true; `pg_stat_statements` without `pg_read_all_stats`; MySQL: the object missing from `information_schema` while a privilege check names it, or the Performance Schema refused |
| `stale` | present, but the table changed much since | PostgreSQL: `n_mod_since_analyze` above 10 % of `reltuples` — a threshold of this round, not a finding |
| `unsupported` | the engine or the object has no such thing | MySQL: `correlation`, `avg_width`; PostgreSQL: `correlation` of a type without `<`; any column statistic of a view |

**Missing is not zero.** A rule that needs a feature in any state but
`present` treats it as unknown, and a candidate that depended on it says so.
The report counts, for each fixture and engine, how many features were in
each state.

## Tier S: the schema (configuration A)

What the schema's designer wrote: names, types, keys, constraints, defaults,
indexes, partitions, comments. Enum labels, `CHECK` expressions, defaults and
comments are text a designer wrote, not a user, and are in S; they are marked
`designer_text` so a later policy can drop them.

**PostgreSQL.** One query each from the catalogs, joined on `oid`, schemas
`pg_catalog`, `information_schema` and `pg_toast` excluded:

- tables, views and partitions: `pg_class` (`relkind`, `relispartition`,
  `relrowsecurity`, `relforcerowsecurity`), `pg_inherits`;
- columns: `pg_attribute` with `format_type(atttypid, atttypmod)`,
  `attnotnull`, `attidentity`, `attgenerated`, the default from
  `pg_get_expr(adbin, adrelid)`, `col_description`;
- constraints: `pg_constraint` (`contype` in `p`, `u`, `f`, `c`) with
  `conkey`, `confrelid`, `confkey` and `pg_get_constraintdef`;
- enumerated types: `pg_enum` labels in `enumsortorder`;
- indexes: `pg_index` (`indkey`, `indisunique`, `indisprimary`, and
  `indpred` through `pg_get_expr` for a partial index);
- comments: `obj_description` and `col_description`;
- privileges, for the states: `has_table_privilege`, `has_column_privilege`,
  `row_security_active`.

**MySQL.** From `information_schema`, which shows only the objects the role
holds some privilege on — a denied table is missing from the schema itself,
not only from the statistics:

- `TABLES`: `TABLE_TYPE`, `ENGINE`, `TABLE_COMMENT`, `CREATE_OPTIONS`;
- `COLUMNS`: `DATA_TYPE`, `COLUMN_TYPE` (which spells out an `ENUM`'s
  labels), `NUMERIC_PRECISION`, `NUMERIC_SCALE`, `IS_NULLABLE`,
  `COLUMN_DEFAULT`, `COLUMN_KEY`, `EXTRA` (`auto_increment`,
  `DEFAULT_GENERATED`, `on update CURRENT_TIMESTAMP`), `COLUMN_COMMENT`;
- `TABLE_CONSTRAINTS`, `KEY_COLUMN_USAGE` with `REFERENCED_*`,
  `REFERENTIAL_CONSTRAINTS`, `CHECK_CONSTRAINTS`;
- `STATISTICS` for index columns and order (`INDEX_NAME`, `SEQ_IN_INDEX`,
  `NON_UNIQUE`), its `CARDINALITY` belonging to tier N;
- `PARTITIONS` for partitioned tables.

## Tier N: numbers about columns and tables (configuration B1)

Numbers only. Where a statistics view holds values, the number is computed
from them on the server.

**PostgreSQL**, per column, from `pg_stats`:

```sql
SELECT s.schemaname, s.tablename, s.attname, s.inherited,
       s.null_frac, s.avg_width, s.n_distinct, s.correlation,
       array_length(s.most_common_freqs, 1)                AS mcv_count,
       s.most_common_freqs[1]                              AS mcv_top_freq,
       (SELECT sum(f) FROM unnest(s.most_common_freqs) f)  AS mcv_freq_sum,
       array_length(s.histogram_bounds::text::text[], 1)   AS hist_bounds
FROM pg_stats s
WHERE s.schemaname NOT IN ('pg_catalog', 'information_schema');
```

`most_common_freqs` holds frequencies, not values, and is read whole: the
share of the commonest value and how much of the column the common values
cover say whether a column is a category, skewed or near unique. `n_distinct`
keeps its sign: positive is a count, negative a fraction of the rows.
`correlation` is the agreement of the column's order with the rows' physical
order, never a relation between columns. A partitioned table's own row has
`inherited = true`.

Per table, from `pg_class` and `pg_stat_user_tables`: `reltuples` (`-1`
means never vacuumed nor analysed, which is unknown, not empty), `relpages`,
`n_live_tup`, `n_dead_tup`, `n_tup_ins`, `n_tup_upd`, `n_tup_hot_upd`,
`n_tup_del`, `n_mod_since_analyze`, `last_analyze`, `last_autoanalyze`,
`analyze_count`, `autoanalyze_count`. **Activity counters and `correlation`
describe how rows were written**: on a live database, by the application; on
a fixture, by its loading — Chinook's `invoice_date` and every identity
column correlate at 1 because the script inserts them in order. The
generated fixtures therefore replay a history of inserts, updates and
deletes where a table is meant to change, and the public ones are reported
as loaded.

**MySQL**, per table: `TABLES.TABLE_ROWS`, `AVG_ROW_LENGTH`, `DATA_LENGTH`,
`AUTO_INCREMENT`, `UPDATE_TIME`. These are cached for
`information_schema_stats_expiry` seconds, 86 400 by default, and refetched
from the storage engine when the cache has expired; the harness records the
setting, and the cost is counted with it as found. Per index prefix:
`STATISTICS.CARDINALITY`, an estimate from random dives, which is a column's
distinct count only for the first column of an index and is never given to
the other members of a composite key. Per table where the Performance Schema
is readable: `table_io_waits_summary_by_table`'s `COUNT_INSERT`,
`COUNT_UPDATE` and `COUNT_DELETE`, since the server started.

Per column, only where someone made a histogram, the scalars of
`COLUMN_STATISTICS`, the buckets never fetched:

```sql
SELECT cs.TABLE_NAME, cs.COLUMN_NAME,
       ANY_VALUE(cs.HISTOGRAM->>'$."histogram-type"')    AS kind,
       ANY_VALUE(cs.HISTOGRAM->>'$."null-values"')       AS null_frac,
       ANY_VALUE(cs.HISTOGRAM->>'$."sampling-rate"')     AS sampling_rate,
       ANY_VALUE(cs.HISTOGRAM->>'$."last-updated"')      AS last_updated,
       ANY_VALUE(JSON_LENGTH(cs.HISTOGRAM->'$.buckets')) AS buckets,
       SUM(jt.nd)                                        AS distinct_sum
FROM information_schema.COLUMN_STATISTICS cs
LEFT JOIN JSON_TABLE(CAST(cs.HISTOGRAM AS JSON), '$.buckets[*]'
                     COLUMNS (nd BIGINT PATH '$[3]')) jt ON TRUE
WHERE cs.SCHEMA_NAME = DATABASE()
GROUP BY cs.TABLE_NAME, cs.COLUMN_NAME;
```

A singleton histogram's bucket count is the column's distinct count in the
sample; an equi-height histogram's is the sum of its buckets' distinct
counts, summed on the server. Without a histogram, MySQL
has no null fraction and no distinct count for an unindexed column, and no
correlation at all: B1 on MySQL is close to A, and the report says how close.

## Tier V: values inside statistics (configuration B2)

PostgreSQL's `most_common_vals`, `histogram_bounds` and `most_common_elems`,
and MySQL's histogram buckets. They are rows in all but name — the bounds of
a salary column are salaries — so **V is read on public and generated
fixtures only**, as a separate configuration, to measure what the values buy:
a currency column's three codes, a status column's labels, a time column's
range.

## Tier Q: the application's own queries

PostgreSQL's `pg_stat_statements` (`query`, `calls`, `rows`), the text of
other roles' queries visible only with `pg_read_all_stats`; MySQL's
`performance_schema.events_statements_summary_by_digest` (`DIGEST_TEXT`,
`COUNT_STAR`). Both are normalised, their constants replaced. A freshly
loaded fixture has no workload, so round 1 records only whether Q is
available and to whom; what it says about joins and groupings is measured on
a live database, later.

## Counting the database's work

Three ledgers, kept apart and summed only in a fourth line, first open:

| Ledger | What | Counted by |
|---|---|---|
| discovery | every S, N, V and Q query | PostgreSQL: `pg_stat_statements` on the fixture server — `total_exec_time`, `shared_blks_hit`, `shared_blks_read`, `rows` — reset before the step and read after it; MySQL: `events_statements_history_long` — `TIMER_WAIT`, `ROWS_EXAMINED`, `ROWS_SENT`, `SELECT_SCAN`, `NO_INDEX_USED` — and the buffer pool's read counters before and after |
| execution | each judged candidate's query, as drawn | the same counters, and its plan from `EXPLAIN` before it runs: whether it scans a whole table, which is what chapter 14 forbids for looking |
| judging | nothing on the database | — |
| first open | the sum, for the six panels `see` would draw | — |

Each query carries a comment naming its step and candidate — inside the
statement, after its first keyword, because `pg_stat_statements` keeps a
statement from its first token and drops a comment before it — so the
counters can be matched to it. Wall time is taken on the client as well and reported
beside the server's. Discovery is expected to cost little and is measured
anyway: on MySQL an expired `information_schema` cache sends it to the
storage engine.

**Cancellation is measured, not assumed.** On the largest fixture, a
candidate whose plan scans a whole table runs under a 2 s limit, and the
harness checks in `pg_stat_activity` or the process list that the server
stopped working, not only that the client gave up.

## Checked against the servers

On 27 October 2026 the `snapshot` crate ran every query above, as the round's
read-only role, on Chinook 1.4.5 (commit `7f67772`) loaded into PostgreSQL
18.6 and MySQL 8.4.13 from `compose.yaml`, under Docker 29.6.2 on Windows 11,
and once on a scratch MySQL schema with three histograms made on purpose.
**This was a check of the queries, not a measurement**: one run on a desktop,
whose server counters show what each query touches, not what it costs.

| What the documentation suggested | What the servers did | Changed |
|---|---|---|
| a comment at a query's start names it in `pg_stat_statements` | the stored text starts at the first token; the comment is gone, and the ledger matched nothing | the marker goes after `SELECT` |
| the application's schema holds the application's tables | `pg_stat_statements`' two views sit in `public`, 54 columns that are no table's | objects an extension owns, by `pg_depend.deptype = 'e'`, are left out |
| `JSON_TABLE` over `COLUMN_STATISTICS.HISTOGRAM` in a scalar subquery | error 1210, also in a `LEFT JOIN`, until the column is cast to `JSON` | `LEFT JOIN JSON_TABLE(CAST(… AS JSON) …)`; on the scratch schema the sums gave 2 000 and 360 distinct values and the singleton 3, each the true count, and `null-values` 0.1 the true share |
| a read-only transaction | one failed query would abort every query after it | read-only and time limits set for the session |
| the Performance Schema readable where the role can read its database | error 1142 for `table_io_waits_summary_by_table` and for the statement digests | MySQL's activity counters and tier Q are `denied` to a role with `SELECT` on its database only, as found |
| `null_frac` unsupported on MySQL | it is absent until someone makes a histogram, and then present | states as in the table above |

What the two snapshots hold, Chinook's eleven tables and 64 columns:

| | PostgreSQL, `fresh` | MySQL, `none` |
|---|---|---|
| tier S | every table, column, key, index; no comments, no enums, no defaults but identity | the same, `auto_increment` for identity |
| rows estimate | 11 of 11, exact on these small tables | 11 of 11, exact on these small tables |
| null fraction, distinct count | 64 of 64 | none; an index's cardinality for 21 columns that lead an index |
| correlation, width, common values | 64 of 64 | unsupported |
| activity counters | 11 of 11, every table inserted once and never updated | denied |
| tier Q | the extension present, the statements denied without `pg_read_all_stats` | denied |

Of the eight PostgreSQL queries, `pg_stats` touched 11 166 shared buffers for
its 64 rows and the other seven at most 408 each. That the view reads the
whole of `pg_statistic`, the catalogs' own statistics included, and detoasts
its arrays before it filters by schema is a hypothesis until a run separates
it. On MySQL every `information_schema` query scanned, examining up to
794 dictionary rows. Whether either grows into a cost worth avoiding on a
database of thousands of tables is a measurement for the round's run, in the
Linux container.
