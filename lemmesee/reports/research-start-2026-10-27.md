# A starting point after the abstract search, 27 October 2026

Round 0 follow-up: literature, implementation and database-documentation
review, followed by a proposed pilot. **No prototype was run, no corpus was
downloaded, and there are no lemmesee measurements here.** Proposed counts
and budgets below are experimental choices, not findings.

This supersedes the preliminary novelty assessment in
[the abstract search](literature-2026-10-27.md). It does not replace that
report's bibliography or revise its historical figures.

| Question | Finding | What follows |
|---|---|---|
| What is the nearest practical baseline? | Metabase X-rays generates dashboards from field metadata and scored templates. | Compare the rules with that design, not just with learned chart recommenders. |
| Is several related tables a new setting? | SeeDB already defines its problem over a snowflake schema and select-project-join queries. | Multi-table input alone is not a novelty claim. |
| Is starting with cheap features new? | ScaleViz explicitly includes a zero-cost feature set. CompassQL describes schema-based ordering. | Specify exactly what existing database statistics buy; do not claim the cheap tier itself. |
| Is catalog-only discovery the same as seeing no values? | PostgreSQL statistics can contain actual values. | Make an explicit feature allowlist. |
| Can discovery's budget cover drawing the dashboard? | Cheap selection does not make aggregate execution cheap. | Account for recommendation, preview and final queries separately. |
| What should run first? | The current evidence does not establish that a learned ranker is needed. | A small rules pilot, an information contract and a judging pilot. |

## The closest work, and the limits of the comparison

### Metabase X-rays: the missing product baseline

X-rays automatically generates charts for a table or model and can save the
result as a dashboard. The inspected implementation selects a dashboard
template from entity metadata, binds fields to dimensions, combines those
with metrics and filters, and selects scored cards. Its generic template
contains counts, sums, averages, time fields and category-cardinality gates.
This is close to the proposed generator and rules ranker.
[Documentation](https://www.metabase.com/docs/latest/exploration-and-organization/x-rays),
[implementation at 9f81a6d](https://github.com/metabase/metabase/blob/9f81a6dea2bc240f5e41000f14a25d298ff5d7ed/src/metabase/xrays/automagic_dashboards/core.clj),
[generic template at the same commit](https://github.com/metabase/metabase/blob/9f81a6dea2bc240f5e41000f14a25d298ff5d7ed/resources/automagic_dashboards/table/GenericTable.yaml).

Metabase's documented metadata pipeline also fingerprints the first 10,000
rows for distinct counts, null fractions and numeric summaries. That
upstream acquisition must be charged in any comparison; cached metadata
does not make its acquisition disappear.
[Sync, scanning and fingerprinting](https://www.metabase.com/docs/latest/databases/sync-scan).

**Proposed use:** inspect its template vocabulary and run a pinned release
on public fixtures later. Independently implement a small rules baseline
with declared inputs. This review did not run X-rays, establish its total
cost, or show that it can operate with lemmesee's restricted inputs.

### SeeDB: joins and database work are already part of the problem

SeeDB's section 2 assumes a snowflake schema and supports
select-project-join queries. It ranks aggregate views by deviation between a
selected subset and a reference. Sections 3–4 describe database middleware,
shared queries and progressive pruning; section 6 includes human judgments.
Thus neither SQL-backed recommendation, multi-table input nor judging views
is new by itself. Its task assumes dimensions, measures and an exploration
query; lemmesee's proposed task starts earlier, with an unfamiliar schema
and no such declared intent.
[Paper, §§2–4 and 6](https://www.vldb.org/pvldb/vol8/p2182-vartak.pdf).

### ScaleViz: compare objectives, not just budgets

ScaleViz profiles feature cost on growing samples, extrapolates cost, and
trains a budget-aware acquisition agent for the target dataset and
recommender. Section 4.1 explicitly names zero-cost features. Its evaluation
measures how closely reduced-feature predictions match a base model using
all features; that is different from asking a domain owner whether a panel
has a false meaning. Per-dataset profiling and adaptation are also costs to
account for, whereas lemmesee currently proposes offline training only.
[Paper, §§3–5](https://arxiv.org/html/2411.18657v1).

**Inference:** a plausible distinction is the combination of existing
planner statistics, an offline-trained compact policy, measured database
work and explicit semantic abstention. This is a working hypothesis about
scope, not an established first contribution. Do not translate the paper's
zero-cost assumptions about min/max or categories into SQL cost guarantees.

### Lux: the database backend also needs checking

Lux separates metadata computation from visualization processing, uses
lazy computation and caching, and can rank approximately before computing
selected views exactly. Its paper also describes a SQL execution backend;
it is not confined to a local dataframe. In the inspected `SQLExecutor`,
`compute_stats`, `get_cardinality` and `get_unique_values` issue queries for
the corresponding information. Calling these results metadata does not
establish a catalog-only implementation.
[Paper, §§7–8](https://www.vldb.org/pvldb/vol15/p727-lee.pdf),
[SQLExecutor at 972e5ec](https://github.com/lux-org/lux/blob/972e5ec24991483370dda67de6bb1e354bcf8ca6/lux/executor/SQLExecutor.py#L538).

### CompassQL, Foresight and relational visualization

CompassQL explicitly separates enumeration, constraints, ranking and
redundancy reduction. It describes schema-based query ordering, and Voyager
orders some suggestions by field order. It is a useful baseline for the
architecture and for testing whether a learned ranking improves on a very
simple deterministic order.
[Paper, §§3–5](https://www.domoritz.de/papers/2016-Compassql-HILDA.pdf).

Foresight recommends strong statistical patterns and uses approximate
sketching. It is an additional predecessor for scalable discovery, although
its abstract does not establish use of existing SQL planner statistics or a
compact recommender runtime.
[Authors' publication page](https://research.ibm.com/publications/foresight-recommending-visual-insights).

Wu's ICDT 2025 paper studies faithful visualization of database contents
and constraints, including foreign-key relationships and unambiguous join
paths. It is a conceptual source for preserving relational structure,
not evidence that a join's business interpretation or aggregate is correct.
[Paper, §§2–4](https://drops.dagstuhl.de/storage/00lipics/lipics-vol328-icdt2025/html/LIPIcs.ICDT.2025.35/LIPIcs.ICDT.2025.35.html).

One recent adjacent lead is QRec-NLI, published online in July 2026. Its
abstract describes context-aware recommendations over unfamiliar multi-table
databases in an LLM/NLI workflow. Read it before claiming that multi-table
exploration is uncovered; no compact catalog-only method was established
from this abstract.
[Primary publication](https://doi.org/10.1016/j.visinf.2026.100347).

## Define what the program is allowed to learn

The following is a proposed contract, derived from the documentation. It
has not been tested against running servers.

| Input | What the source actually offers | Proposed treatment |
|---|---|---|
| PostgreSQL schema | Names, SQL types and declared constraints | A; a name is evidence, not a confirmed business meaning. |
| PostgreSQL `pg_stats` scalars | `null_frac`, `avg_width`, `n_distinct`; negative `n_distinct` encodes a ratio | B; retain the raw representation, estimate provenance and availability. |
| PostgreSQL values in statistics | `most_common_vals`, `histogram_bounds`, array-element values | Exclude from the initial scalar-only B; any value-bearing variant is separately named. |
| PostgreSQL `correlation` | Physical row order versus a column's logical order | Never describe it as correlation between measures or a time trend. |
| PostgreSQL row estimate | `pg_class.reltuples` is approximate and may be stale | Do not display it as an exact count; preserve missing/uninitialized state. |
| MySQL index statistics | Estimated index cardinalities, with index and key-part context | Not a general per-column distinct-count service; do not assign a composite-key estimate to each member. |
| MySQL histograms | Existing `COLUMN_STATISTICS` histograms, when available; JSON includes buckets and scalar summaries | Optional capability; avoid fetching whole histograms for the scalar-only variant. |
| MySQL table statistics | Cached values; missing or expired cache can cause retrieval from the storage engine | Measure the actual metadata query; a catalog name is not a zero-work guarantee. |

Sources: PostgreSQL 18
[`pg_stats`](https://www.postgresql.org/docs/18/view-pg-stats.html) and
[planner statistics](https://www.postgresql.org/docs/18/planner-stats.html);
MySQL 8.4
[`STATISTICS`](https://dev.mysql.com/doc/refman/8.4/en/information-schema-statistics-table.html),
[optimizer histograms](https://dev.mysql.com/doc/refman/8.4/en/optimizer-statistics.html),
[`TABLES`](https://dev.mysql.com/doc/refman/8.4/en/information-schema-tables-table.html).

Do not refresh source statistics during discovery. MySQL histogram
management uses `ANALYZE TABLE`; creating statistics is a different operation
from reusing existing ones.
[MySQL 8.4 `ANALYZE TABLE`](https://dev.mysql.com/doc/refman/8.4/en/analyze-table.html).

Each feature should carry at least `value`, `source`, `availability` and
freshness information where the engine exposes it. Missing, denied and stale
are different states. Use capability flags per engine. Query explicit
columns rather than `SELECT *` from statistics views. The real-database
policy must also decide whether comments, enum labels and check expressions
are within scope; the label "schema" does not settle that choice.

## Discovery, preview and execution need separate accounts

Keep three ledgers: acquiring recommendation features, obtaining a bounded
preview, and executing the retained panel. Charge total first-open database
work as well as each component. PostgreSQL documents that an exact
`count(*)` over a whole table must scan the table or an index containing all
its rows. Choosing that count from the schema is cheap; filling it is not.
[Aggregate documentation](https://www.postgresql.org/docs/18/functions-aggregate.html).

Sampling is also engine- and method-specific. PostgreSQL's `BERNOULLI`
sampling scans the whole table; `SYSTEM` samples blocks and is sensitive to
clustering. Returned rows, examined tuples, touched pages and wall time are
different limits. A `LIMIT` on an aggregate's output does not bound its
input work.
[PostgreSQL `SELECT`, TABLESAMPLE and processing order](https://www.postgresql.org/docs/18/sql-select.html).

**Proposed decision before coding:** either a panel fits a verified
execution budget, shows a clearly identified estimate/preview, or remains
an unexecuted suggestion. The later harness must measure cancellation on
the server, not just the client's timeout. These are proposed research
requirements, not a change to Dashbin's product contract in this review.

## The pilot to specify next

Use the same candidate generator for A and B. Otherwise a larger candidate
pool could be mistaken for a better ranker. Reserve an independent checklist
of useful questions to expose views that the generator cannot express.

Start with a proposed **8–12 public schemas and roughly 10 candidates per
schema**, with two judges and a documented domain explanation. This is a
judging and failure-discovery pilot, not a sample-size justification for the
final study. Sample schemas with and without declared foreign keys, with
missing statistics, and from more than toy tutorial databases. No production
connection is needed to specify it.

For every candidate, record these separately:

- SQL/type validity and whether the declared row grain survives joins;
- whether the panel would be kept for the stated overview task;
- whether its title, units, population and aggregate have a supported meaning;
- whether more rows could resolve uncertainty, or domain confirmation is needed;
- whether it can be executed within the source-work budget.

Include deliberate traps: `orders` joined to several `order_items` duplicates
an order-level amount; money from several currencies cannot be summed as one
unit; balances are not additive across snapshots; `created_at` may describe
an import rather than the business event. A type-safe template does not
exclude those errors. A bigger sample cannot identify an undocumented unit
or establish which statuses count as paid.

Keep/drop alone is insufficient supervision. Randomize presentation and hide
which configuration proposed a panel; let judges mark uncertainty, report
agreement and adjudicate disagreements. A judge needs domain evidence to
check meaning. Report denominators for harm among all shown panels and among
kept panels, with uncertainty intervals. Do not treat abstaining everywhere
as success: show risk against coverage, alongside useful top-five output.
Selective prediction already formalizes a risk/coverage tradeoff; its
guarantees do not automatically transfer to this task.
[Geifman and El-Yaniv, 2017](https://arxiv.org/abs/1705.08500).

## Corpora and a later model

SchemaPile is useful for schema variety, not labeled dashboard usefulness.
The paper's permissive subset is 22,989 schemas and about 199,000 tables;
only a subset contains row data. The corpus comes from SQL files, not live
optimizer catalogs. Consequently it does not supply realistic PostgreSQL or
MySQL statistics merely by importing the DDL.
[Paper, tables 1 and 7](https://schemapile.github.io/assets/SchemaPile.pdf).

The authors publish a license allowlist and describe replacement of detected
PII values. Pin each selected source and its license. Their repository's
download instructions still point at an old Zenodo record which directs
readers to a newer record; verify the chosen artifact and hash when fetching.
This review did not validate the newer artifact, which returned HTTP 429.
[Subset description](https://github.com/amsterdata/schemapile/blob/main/schemapile-perm.md),
[license list](https://github.com/amsterdata/schemapile/blob/main/data/permissive_licenses.json),
[old record](https://zenodo.org/records/10931803),
[replacement record](https://zenodo.org/records/12682521).

Proposed later comparisons:

- Split by upstream project and schema family, deduplicating migrations,
  forks and tutorial copies. Include name-masked and missing-statistics tests.
- Treat column-type labels, authored charts and SQL questions as different
  auxiliary signals. An unchosen chart is not automatically a bad chart.
- Compare fixed rules, a linear scorer, then shallow trees. Include name
  tokens, neighbor roles, declared key structure, feature presence and
  source provenance. Train on the same acquisition regime used at inference.
- Before learning a sampler, compare never sample, always take the allowed
  sample, and a simple uncertainty rule at matched budgets. Retrospectively
  inspect how often extra information actually changes a judged decision.
  Buy feature groups per table/query when a single sample supplies many
  columns; do not charge each feature as an independent scan.
- Charge token dictionaries, feature extraction and evaluator code with
  weights in binary size and memory. The plan's 256 KiB weights and 1 MiB
  extra memory are research targets; Dashbin chapter 14 has its own shipping
  gates. No model here has met either.

**A sensible stopping result:** rules meet Dashbin's useful/harmful-panel
gates and no compact model improves on them at comparable coverage and
source cost. That is a successful research outcome.

## Review method and remaining reading

Reviewed on 27 October 2026 from the local Windows checkout at research
commit `e0791e49b814c4ba2982be1d16e18f4d9611a22f`, using repository reads,
web search, primary paper text, official engine documentation and read-only
GitHub source access. No benchmark environment, harness or performance
result is claimed. The database versions above identify documentation, not
servers measured. GitHub commits identify the implementations inspected,
not software run.

Reading depth: targeted method/evaluation sections of ScaleViz, SeeDB and
Lux; CompassQL's decomposition and ranking sections; the ICDT paper's main
text; Metabase's core design comment and generic template; Lux's SQL
metadata/execution functions. Foresight and QRec-NLI were screened from
primary abstracts/pages. This is not a claim to have read every paper whole
or completed an exhaustive 2024–2026 review.

Searches included `visualization recommendation metadata-only`,
`visualization recommendation catalog database`,
`visualization recommendation schema 2026`,
`Metabase automagic dashboards rules`, `ScaleViz budget statistics`, and
`SchemaPile 22,989`, followed by the linked sources and code. Empty search
results do not establish absence of prior work. A wider bibliography can
start from the authors' [2024 systematic review](https://doi.org/10.1561/1900000088)
and the [2026 review covering 2017–2025](https://doi.org/10.1177/14738716251409351);
their references still need checking against primary works.

Next deliverable: a reviewable acquisition contract, candidate specification,
fixture manifest and judging form for round 1. Prepare and review the harness
before running it; the repository requires it committed before measurement,
and commits remain a user action unless explicitly requested.
