# What has been done before, a wider reading, 27 October 2026

Round 0, a second follow-up. [The abstract search](literature-2026-10-27.md)
and [its follow-up](research-start-2026-10-27.md) left a wider search open;
this is that search, and it reports only what they did not reach: older work
on finding measures and dimensions in a relational schema, learning from what
people keep, how Metabase types a column before its templates, bounded looks
in other products, where the column-type corpora's labels come from,
prediction under a test-time budget, and what PostgreSQL and MySQL hide or
carry beyond the follow-up's contract. **Like both, it is a reading of
abstracts, documentation and search summaries**, with three things read at
their source: Metabase's classifiers, the definition of PostgreSQL's
`pg_stats`, and SchemaPile's tables of counts.

| Question | Answer | What follows |
|---|---|---|
| Has anyone found measures, dimensions and joins in a relational schema? | **Yes, for over twenty years**: automatic multidimensional design from relational sources (Phipps and Davis 2002; Jensen, Holmgren and Pedersen 2004), and Auto-BI (VLDB 2023), which predicts a BI model's joins, above 0.9 F1 on over 100 thousand real models | beside SeeDB, two more lines to read before anything is said about several tables |
| Has anyone learned from people keeping and dropping suggested charts? | **Yes: VizDeck** (SIGMOD 2012) dealt charts ranked from a table's statistics as cards; what people kept and discarded fed the ranking | `see`'s keep and drop is a known instrument |
| How does Metabase type a column before its templates? | Ordered patterns over the column's name; a category at **at most 30** distinct values in the first 10 000 rows; a table's kind from a word at the start or end of its name | the rules baseline exists down to its classifiers; A's rules are written knowing them |
| Does any product bound a look per column? | **Datasette**: one query a column to suggest facets, 50 ms each, the column dropped past it | a bound per column ships; deciding whether to look does not |
| Are the column-type corpora's labels independent of a column's name? | **No.** Sherlock's labels are VizNet's headers matched to 78 types; GitTables' come from matching against its ontologies, by name as far as was read | a model that reads names can neither learn from nor be tested on them: it would learn the annotator |
| Can labels come from something other than a name? | **Declarations**: a foreign key, a default of `now()`, an `ENUM` say what a column is; SchemaPile has 700 thousand declared foreign keys | a model can learn to guess a declaration where a schema left it out |
| Is deciding whether to look a known problem? | **Yes: prediction under a test-time budget**, Greedy Miser (ICML 2012) and adaptive approximation (NeurIPS 2017) | D gets a literature older than ScaleViz and a method without reinforcement learning |
| What can a reader not see in `pg_stats`? | A column it may not select, and **every column of a table under active row-level security** | "denied" in the follow-up's contract has concrete causes, to be told apart from "never analysed" |
| Can statistics be studied without the rows? | **Yes, on PostgreSQL 18**: `pg_dump --statistics-only` writes them as calls that load them into an empty schema | real statistics without rows, once the values in them are stripped |
| What does an application's own query log say? | Over a quarter of the equality joins in BIRD's development queries are absent from its schema (2025); Precision Interfaces builds interfaces from a log (SIGMOD 2019) | `pg_stat_statements` is a source the plan does not list |
| Is SQaLe 2 the corpus lemmesee lacks? | **Not as it stands**: 9 259 SchemaPile schemas extended by a language model to a median of 113 tables, filled with 20 to 400 rows a table by generated code, and 176 761 distinct SQL queries written and checked by language models; SQLite, MIT | a source of joins and aggregates someone found askable, not of names, statistics or usefulness |
| How many judged panels show "one harmful in twenty"? | With none harmful, **59** kept guessed panels bound the rate below 5 % at 95 %; with one, 93; with two, 124 | the pilot cannot show chapter 14's line; round 2 is sized from it |

## Measures, dimensions and joins from a schema

- **Automatic multidimensional design** has derived facts, measures and
  dimensions from relational schemas since the early 2000s: Phipps and Davis
  (DMDW 2002), Jensen, Holmgren and Pedersen's *Discovering multidimensional
  structure in relational data* (DaWaK 2004), and others reviewed by Romero
  and Abelló (DKE 2010), who note that most of them find what may be a fact or
  a dimension with patterns or heuristics, and need well-documented sources.
  **Only citations, Romero and Abelló's review and others' summaries were
  read.**
  [Phipps and Davis](https://ceur-ws.org/Vol-58/),
  [Jensen et al.](https://doi.org/10.1007/978-3-540-30076-2_14),
  [Romero and Abelló](https://www.cs.upc.edu/~aabello/publications/10.DKE1.pdf)
- **Auto-BI** (Lin, He and Chaudhuri, PVLDB 16, 2023) predicts the joins of a
  BI model — a star or a snowflake — from its tables, combining local join
  predictions with the shape of the whole schema graph as a minimum-cost
  arborescence; sub-second on close to 100 tables, F1 above 0.9 on over
  100 thousand real models and four TPC benchmarks. Whether its local
  predictions need the rows is to be read.
  [paper](https://www.vldb.org/pvldb/vol16/p2578-he.pdf)
- **Keys nobody declared.** SchemaPile's paper fine-tunes a 16-billion
  parameter code model to find foreign keys from the schema alone, tested on
  555 table pairs from Spider, 386 from BIRD and 877 from the CTU Prague
  repository. The model is what lemmesee refuses to carry; the pairs are a
  ready test for guessing undeclared keys by name. HoPF (Jiang and Naumann,
  2020) finds 88 % of primary and 91 % of foreign keys, but from the values.
  [SchemaPile, §4.1](https://ir.cwi.nl/pub/34763/34763.pdf),
  [HoPF](https://hpi.de/oldsite/fileadmin/user_upload/fachgebiete/naumann/publications/PDFs/2020_jiang_holistic.pdf)

## Learning from what people keep

- **VizDeck** (Key, Howe, Perry and Aragon, SIGMOD 2012 demo) ranks charts of
  an unfamiliar table from its statistics and deals them as cards; people
  keep the useful ones and discard the rest, and the choices teach the ranking
  for later users. A 2019 survey doubts it was deployed beyond testing.
  [paper](https://homes.cs.washington.edu/~billhowe/publications/pdfs/key2012vizdeck.pdf),
  [survey](https://arxiv.org/pdf/1911.03871)
- **An evaluation-focused framework** (Zeng et al., VIS 2021) describes a
  recommender as a design space, a way of walking it and an oracle that ranks
  what it finds; four algorithms built from it performed similarly for the
  people who tried them. A judged set must be large enough to separate rankers
  that differ little — the pilot's 8 to 12 schemas will not be.
  [paper](https://homes.cs.washington.edu/~leibatt/static/papers/zeng2021evaluation.pdf)
- **No harm seen is not safety shown.** Chapter 14 allows at most one harmful
  guessed panel in twenty kept. The exact one-sided binomial bound at 95 %
  falls below 5 % only at **59** kept guessed panels with none harmful, **93**
  with one and **124** with two; with none harmful among 20 it is 13.9 %, among
  40 it is 7.2 %. The pilot's 80 to 120 candidates, of which only the kept
  guesses count, can find harms but cannot show the line; round 2's judged set
  is sized by these numbers, per engine if the line is to hold per engine.
  They are arithmetic, computed with Python's `math.comb` by bisection, not a
  measurement.

## How products look before they draw

- **Metabase's classifiers**, read on `master` beside the templates the
  follow-up read. A column's type comes from an ordered list of patterns over
  its lowercased name, the first that fits its base type winning: `create` on
  a date or time is a creation time, `^status$` and `_type$` on text are
  categories, `count$` and `^num_` on an integer are quantities, `discount`
  and `income` on a number are money of their kind. A column with no type yet,
  not all null, with at most 30 distinct values in the fingerprint of the
  first 10 000 rows, becomes a category. A table is a transaction table when
  its name begins or ends with `order`, `transaction` or `sale`; a user table
  with `user`, `account`, `person` or `employee`; an event table with `event`,
  `checkin` or `log`; otherwise generic — the four kinds the templates are
  written for. The code outside `enterprise/` is AGPL: read and run, never
  copied into Dashbin.
  [name classifier](https://github.com/metabase/metabase/blob/master/src/metabase/analyze/classifiers/name.clj),
  [category classifier](https://github.com/metabase/metabase/blob/master/src/metabase/analyze/classifiers/category.clj),
  [templates](https://github.com/metabase/metabase/tree/master/resources/automagic_dashboards/table),
  [licence](https://github.com/metabase/metabase/blob/master/LICENSE.txt)
- **Rill** went from a source or model straight to a dashboard by heuristics
  in 0.7 — text columns as dimensions, `COUNT(*)` as the first measure, a
  timestamp for the line charts — and in 0.41 let a language model write it,
  then checked each measure against the engine because some were not valid
  SQL.
  [0.7](https://docs.rilldata.com/notes/0.7),
  [0.41](https://docs.rilldata.com/notes/0.41),
  [the post](https://rilldata.com/blog/one-click-dashboards-with-generative-ai-and-bi-as-code)
- **Datasette** suggests facets by running one SQL query for every column of
  a table, each allowed 50 ms by default; a column whose query runs longer is
  not suggested.
  [settings](https://docs.datasette.io/en/latest/settings.html)

Chapter 14's bound on looking, 10 000 rows a table, is Metabase's number.
Metabase's are the first rows, and the first rows of a table that is only
appended to are its oldest.

## What the catalogs hide or carry

- **What `pg_stats` hides.** PostgreSQL keeps `pg_statistic` from the public
  because "minimum and maximum values of a salary column might be quite
  interesting". The definition of `pg_stats` on `master` shows a row only
  where `has_column_privilege(c.oid, a.attnum, 'select')` holds, and only where
  row security is off or not active for the reader: a role that cannot read a
  column sees no statistics for it, and a table under row-level security, as
  in a multi-tenant application, shows none.
  [pg_statistic](https://www.postgresql.org/docs/current/catalog-pg-statistic.html),
  [system_views.sql](https://github.com/postgres/postgres/blob/master/src/backend/catalog/system_views.sql)
- **PostgreSQL 18 dumps and restores statistics**: `pg_dump --with-statistics`,
  `--no-statistics` and `--statistics-only` write them as calls to
  `pg_restore_relation_stats()` and `pg_restore_attribute_stats()`, made for
  `pg_upgrade`; extended statistics are not carried. A fix to restoring
  selected objects with `--statistics-only` was backpatched to 18 in June 2026.
  [Postgres Pro](https://postgrespro.com/blog/pgsql/5972351),
  [the fix](https://www.postgresql.org/message-id/E1wZGep-000Tet-2N%40gemulon.postgresql.org)
- **An exact bounded sample**: `tsm_system_rows` returns exactly *n* rows by
  sampling blocks, so it clusters as `SYSTEM` does, and cannot be repeated.
  [tsm_system_rows](https://www.postgresql.org/docs/current/tsm-system-rows.html)
- **MySQL 8.4** adds `AUTO UPDATE` to a histogram, refreshed with each
  `ANALYZE TABLE` and with InnoDB's background recalculation; `MANUAL UPDATE`
  stays the default, so a histogram nobody asked for still does not exist.
  [ANALYZE TABLE](https://dev.mysql.com/doc/refman/8.4/en/analyze-table.html)
- **MariaDB** keeps engine-independent statistics in `mysql.table_stats`,
  `column_stats` and `index_stats`, but a plain `ANALYZE TABLE` does not
  collect them by default: a third engine, with a third contract.
  [engine-independent statistics](https://mariadb.com/docs/server/ha-and-performance/optimization-and-tuning/query-optimizations/statistics-for-optimizing-queries/engine-independent-table-statistics)

## Where the labels come from

- **SchemaPile-Perm**, from the paper's Table 6, beyond the follow-up's
  counts: 7.1 thousand of its schemas and 34.9 thousand tables carry data,
  5.9 million values, **28.7 a column on average**, from `INSERT` statements
  in files of at most 400 kB. The subset keeps repositories under the 193
  licences The Stack counts as permissive — MIT 64 %, Apache 2.0 28 %,
  BSD-3-Clause 4 %, CC0 1 %. The newer Zenodo record, which the follow-up could
  not load, is version 0.2 of 7 July 2024 under CC BY 4.0, with
  `schemapile-perm.json.gz` at 28.3 MB.
  [paper](https://ir.cwi.nl/pub/34763/34763.pdf),
  [record](https://zenodo.org/records/12682521)
- **Sherlock's** 78 types are matched exactly to VizNet's column headers, and
  the headers are its ground truth. **GitTables** annotates with DBpedia and
  Schema.org types by a syntactic and a semantic method; its authors' later
  SigmaTyper matches the column's *name* both ways, and GitTables is taken to
  do the same until its paper is read whole. The follow-up's name-masked tests
  catch a model leaning on names; they do not make these labels usable for
  one that reads them.
  [Sherlock](https://arxiv.org/pdf/1905.10688),
  [GitTables](https://arxiv.org/pdf/2106.07258),
  [SigmaTyper](https://arxiv.org/pdf/2109.05173)
- **Pythagoras** (EDBT 2024) types numeric columns with a graph network over
  the values and the table's name; numeric columns are where `see`'s money and
  counts are.
  [LWDA version](https://ceur-ws.org/Vol-3630/LWDA2023-paper13.pdf)
- **SQaLe 2** (Wolff, Gomm and Hulsebos, Amsterdam and CWI, a 2026
  preprint) takes 9 259 schemas from SchemaPile, has a language model add
  tables to each — to a median of 113 tables and 538 columns, where SchemaPile's
  median is four tables — and fills them with rows from pandas code the model
  writes, Qwen3.6-35B: 20 to 100 rows a table, 200 to 400 for a table with
  foreign keys into two or more others, a median of 69, and 4.8 % of tables
  empty; 1 103 669 tables and 108 708 694 rows in 4.03 GB of Parquet. Its
  177 377 questions, 176 761 distinct SQL queries, are generated over
  subgraphs of up to 20 tables, answered by an agent and checked by a model
  judge; each names the tables it was asked over. The DDL is SQLite's, the
  cards say MIT, and the schemas split 8 836 for training and 423 for testing.
  For lemmesee its names are partly a model's, its row counts are budgets and
  its statistics would be of rows a model wrote; what it may give is which
  joins and aggregates a question needed, as a weak signal of what is askable,
  which is not what is worth drawing. An earlier version is on arXiv.
  [project](https://trl-lab.github.io/sqale/),
  [schemas](https://huggingface.co/datasets/trl-lab/SQaLe-2-text-to-SQL-Schemas),
  [queries](https://huggingface.co/datasets/trl-lab/SQaLe-2-text-to-SQL-Queries),
  [first version](https://arxiv.org/abs/2602.22223)
- **SchemaPile annotated** (the same lab) is SchemaPile-Perm's 22 989 schemas
  with a domain label and a two-to-four-sentence description each, written by
  a language-model agent that cloned and explored each source repository; the
  licence is each record's own. A domain is useful to split by and to choose
  the pilot's schemas — every shop in one split — and is a model's guess, not
  a judgment.
  [dataset](https://huggingface.co/datasets/trl-lab/schemapile_annotated)
- **Labels from declarations.** In schemas that declare a foreign key, a
  default of `now()` or `CURRENT_TIMESTAMP`, MySQL's `ON UPDATE
  CURRENT_TIMESTAMP`, an `ENUM` or a `CHECK (status IN (…))`, the declaration
  says what the column is, and was written by whoever designed the schema,
  not by a user. A model can learn to guess it from name, type and neighbours
  in the schemas that leave it out — which most do. This is a proposal, not a
  finding.

## Deciding whether to look

- **Greedy Miser** (Xu, Weinberger and Chapelle, ICML 2012) trains boosted
  shallow trees whose splits pay for the features they use, under a ceiling
  on test-time cost; the **cost-sensitive tree of classifiers** (2013) sends
  each input down a path that buys its own features; **adaptive
  approximation** (Nan and Saligrama, NeurIPS 2017) trains a cheap gate that
  sends an input to an expensive model only where the cheap one would be
  wrong. With the catalog as a free feature group, statistics as a cheap one
  and a sample of rows as a dear one, configuration D is this problem, and
  the follow-up's three baselines — never, always, a simple uncertainty rule —
  are where it starts.
  [Greedy Miser](https://arxiv.org/pdf/1206.6451),
  [tree of classifiers](https://arxiv.org/pdf/1210.2771),
  [adaptive approximation](https://ar5iv.labs.arxiv.org/html/1705.10194)
- **Beside the risk–coverage curve** the follow-up cites, a NeurIPS 2024
  paper argues that its area, AURC, misleads, and proposes AUGRC, the average
  risk of failures not caught.
  [AUGRC](https://arxiv.org/html/2407.01032v2)

## What an application's queries say

- **Precision Interfaces** (Zhang et al., SIGMOD 2019) builds an interface
  from how the queries of a log differ, up to 10 000 queries in 10 s.
  [paper](https://arxiv.org/pdf/1904.02344)
- **Metadata for text-to-SQL** (2025) profiles a database and parses its query
  log, and finds over a quarter of the equality joins in BIRD's development
  queries absent from the schema.
  [paper](https://arxiv.org/abs/2505.19988v2)
- PostgreSQL's `pg_stat_statements` holds the application's own queries
  without their constants: what it joins, filters and groups by. It needs the
  extension and `pg_read_all_stats`, and sits between B and D in cost and in
  what it reveals — not yet in the follow-up's contract.

## Dashboards, learned and generated

- **Learned**: MultiVision (VIS 2021) recommends a dashboard's charts for a
  table with deep learning; DashBot (2022) does it by reinforcement learning;
  MEDLEY (2022) recommends collections of views by intent; DMiner (2022) mines
  design rules from a corpus of dashboards. Each works on one table.
  [MultiVision](https://arxiv.org/pdf/2107.07823),
  [DashBot](https://arxiv.org/pdf/2208.01232),
  [MEDLEY](https://arxiv.org/pdf/2208.03175),
  [DMiner](https://arxiv.org/html/2209.01599v2)
- **Generated by language models**: Data-to-Dashboard (2025) with several
  agents, and DashArena (2026), a benchmark of open-ended dashboard
  generation judged by people. DashArena's paper did not load; what is said of
  it is a search's summary.
  [Data-to-Dashboard](https://arxiv.org/abs/2505.23695),
  [DashArena](https://arxiv.org/pdf/2608.10567)

## Carrying a model

**m2cgen** turns a trained scikit-learn, XGBoost or LightGBM model into code
without dependencies, Rust among its targets; it computes in `f64` only, and
large ensembles have hit Python's recursion limit while generating. Whether
the Rust it writes is smaller than weights read by our own code is a
measurement for round 3.
[m2cgen](https://pypi.org/project/m2cgen/)

## Signals not yet in the contract

None of the work read used these. They are **hypotheses for round 1**, each
measured before it is believed:

- A timestamp whose `correlation` is near 1 has values in the physical order
  of the rows: written in that time's order, as a creation time is in a table
  that is only appended to. It says nothing about any measure.
- `pg_stat_user_tables`' inserts against updates and deletes: a table never
  updated is a log of events.
- Defaults, `ON UPDATE CURRENT_TIMESTAMP`, enumerated types and `CHECK`
  constraints, as above; the follow-up leaves open whether they are in scope,
  and they are the designer's words, not a user's.
- `pg_stat_statements`, as above.

## What follows

- **The claim of novelty narrows again.** Besides SeeDB, finding measures,
  dimensions and joins over several tables is twenty years of data warehouse
  design and Auto-BI; learning from keep and drop is VizDeck's; deciding what
  to compute under a budget is a family older than ScaleViz. The follow-up's
  working hypothesis — existing statistics, a compact offline ranker, measured
  work on the database, semantic abstention — stands as its combination.
- **Proposed to the plan, not yet in it**:
  - no corpus whose labels were matched from names used to train or test
    anything that reads names; labels from declarations, or from judges;
  - "denied" split by its cause — a column's privilege, row-level security —
    and MariaDB as a third engine in the contract if Dmitry's sites use it;
  - `pg_stat_statements` and the signals above in round 1's features;
  - PostgreSQL 18's statistics dumps, stripped of their values, as a corpus
    of real statistics without rows;
  - round 2's judged set sized so that chapter 14's one-in-twenty can be
    shown: at least 59 kept guessed panels for each line that is to hold;
  - SQaLe 2's questions read for joins and aggregates, never as labels of
    usefulness, and annotated SchemaPile's domains used to split and to pick.
- **To read whole next**: Metabase's classifiers beside its templates,
  VizDeck, Auto-BI, a survey of multidimensional design, Greedy Miser and
  adaptive approximation.

## How this round was made

Web searches on 27 October 2026, their results read as the search returned
them, with each work's own page linked. Read at the source that day:
Metabase's `name.clj` and `category.clj` under
`src/metabase/analyze/classifiers/`, the listing of
`resources/automagic_dashboards/table/` and `LICENSE.txt` on its `master`;
the definition of `pg_stats` in `src/backend/catalog/system_views.sql` on
PostgreSQL's `master`; the documentation of PostgreSQL 18.6, MySQL 8.4 and
MariaDB; SchemaPile's paper for its Tables 1 and 6 and Sections 2.3 and 4.1;
the first pages of Pythagoras and of Romero and Abelló's 2010 paper; the
Hugging Face cards of SQaLe 2's two datasets and of annotated SchemaPile, and
SQaLe's project page, after another agent pointed to them. The arXiv pages of
DashArena, GitTables and SQaLe did not load; what is said of them comes from
search summaries and the project's own pages. Nothing was run, nothing was
downloaded, and the only computation is the binomial bound above.
