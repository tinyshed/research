# What has been done before, 27 October 2026

Round 0 of [the plan](../plan.md): what is already known about suggesting
views of data automatically, about guessing what a column means, and about
spending a budget to do either — and how much of lemmesee's three questions
is new. **This round read abstracts, project pages and search summaries, not
the papers whole**; the closest four are to be read whole next.

| Question | Answer | What follows |
|---|---|---|
| Is suggesting views without a language model new? | No: SeeDB (2015), DeepEye (2018), VizML (2019), Draco (2019), Table2Charts (2021), Lux (2022) | lemmesee never claims it |
| Has anyone suggested views from a schema and the database's statistics, before reading a row? | Not found. The systems found compute their features from the values (VizML, Table2Charts, Sherlock) or run the candidate queries (SeeDB, QuickInsights, Lux) | the first question, A and B against C, is the least covered |
| Has anyone decided, under a budget, what to compute about the data? | **Yes: ScaleViz (Adobe, PAKDD 2024)** profiles each statistic's cost on samples, extrapolates it, and picks with reinforcement learning which to compute within a user's time budget | the second question narrows: what differs is the database's own cost on a live PostgreSQL or MySQL, the free tier of the catalog's statistics, and abstaining |
| Has anyone bounded the recommender's own size and memory? | Not found | the third question stands, as an engineering contribution more than a scientific one |
| Is there a judged set of multi-table schemas with useful views? | Not found. nvBench and VisEval pair natural language with charts; an ICDT 2025 paper treats foreign keys in visualization as theory | a judged set would be new, if its sources let it be published |
| Do column-type detectors transfer to new corpora? | Badly: a Sherlock model trained on VizNet fell from 0.77 to 0.66 macro F1 on GitTables | training and testing on different projects is not optional |

## Searching a database for interesting views

- **SeeDB** (Vartak et al., PVLDB 8(13), 2015) ranks candidate views by how
  much a subset's distribution deviates from the whole, as middleware on any
  DBMS, with pruning and shared computation that make it interactive. It runs
  queries; its cost is spent on the data.
  [paper](http://www.vldb.org/pvldb/vol8/p2182-vartak.pdf)
- **Top-K insights** (Tang et al., SIGMOD 2017) and **QuickInsights** (Ding et
  al., SIGMOD 2019, in Power BI) mine multi-dimensional data for insights by
  scoring aggregations, with pruning and a specialized cube.
  [Top-K](https://www.microsoft.com/en-us/research/publication/extracting-top-k-insights-multi-dimensional-data/),
  [QuickInsights](https://microsoft.com/en-us/research/?p=588988)
- **Lux** (Lee et al., PVLDB 15(3), 2022) recommends views of a dataframe
  whenever it is printed, using intent, history, structure and metadata, kept
  fast by approximate query processing, laziness and caching: no more than two
  seconds over pandas on over 98 % of UCI's datasets.
  [paper](https://vldb.org/pvldb/vol15/p727-lee.pdf)
- **Visualization-aware sampling** (Park et al., ICDE 2016) turns a time bound
  into a number of tuples and samples for what will be drawn rather than
  uniformly.
  [paper](https://web.eecs.umich.edu/~michjc/papers/ypark_icde16.pdf)

## Learning which chart to draw

- **VizML** (Hu et al., CHI 2019) learns five design choices from Plotly's
  community feed: about a million raw pairs, some 120 thousand datasets and
  287 thousand charts after cleaning; neural networks predict them close to
  human consensus.
  [paper](https://arxiv.org/pdf/1808.04819)
- **DeepEye** (Luo et al., ICDE 2018) classifies candidate views as good or
  bad and ranks the good ones with learning to rank; its journal version adds
  rules that filter bad views before ranking.
  [summary](https://sciopen.com/article/10.26599/BDMA.2018.9020007)
- **Table2Charts** (Zhou et al., KDD 2021) learns a shared representation of
  fields from 165 thousand spreadsheet tables and 266 thousand charts, with
  deep Q-learning and heuristic search: R@3 0.61 and R@1 0.43 on the
  multi-type task.
  [paper](https://arxiv.org/pdf/2008.11015)
- **Draco** (Moritz et al., 2019) and **Draco 2** (2023) hold design knowledge
  as hard constraints, which say what is valid, and weighted soft constraints,
  which rank the rest; the weights are set by hand or learned from ranked
  pairs. Dashbin's suggestion weights are Draco's (chapter 9).
  [Draco 2](https://arxiv.org/pdf/2308.14247)

## Guessing what a column means

- **Sherlock** (Hulsebos et al., KDD 2019): 686 765 columns from VizNet, 78
  DBpedia types, 1 588 features of the values, support-weighted F1 0.89.
  [page](https://vis.mit.edu/pubs/sherlock)
- **Sato** (2020) adds the table around a column, because a column alone is
  ambiguous — a city's name may be a location, a city or a birthplace.
  [paper](https://arxiv.org/pdf/1911.06311)
- **Doduo** (Suhara et al., SIGMOD 2022) reads the whole table with a
  pre-trained language model and predicts types and relations between
  columns, with 8 tokens a column.
  [paper](https://arxiv.org/pdf/2104.01785)
- **GitTables** (Hulsebos et al., 2023): a million tables from GitHub; its
  retraining of Sherlock showed the transfer problem in the table above.
  [paper](https://arxiv.org/pdf/2106.07258)

## Spending a budget to decide

- **ScaleViz** (Adobe Research, PAKDD 2024; arXiv November 2024) is the
  closest work to lemmesee's second question. Given an existing recommender
  and a user's time budget, it profiles each input statistic's cost on samples
  of growing size, extrapolates to the full data by regression, and chooses
  with reinforcement learning which statistics to compute; about ten times
  faster than baselines at similar error. A related patent covers the same.
  [paper](https://arxiv.org/pdf/2411.18657),
  [patent](https://patentsgazette.uspto.gov/week41/OG/html/1539-2/US12443621-20251014.html)

What lemmesee would add to it, if anything, is to be argued after reading it
whole: the cost counted on the database's side, a live PostgreSQL or MySQL
rather than a local dataframe; a free tier — the catalog and the planner's
own statistics — before any computed statistic; and a right to say "cannot
tell".

## Schemas and relations

- **SchemaPile** (Döhmen et al., SIGMOD 2024): 221 171 schemas from 373 153
  SQL files on GitHub, 1.7 million tables, 10 million columns, 700 thousand
  foreign keys, and data for more than 340 thousand tables — more data than
  the plan assumed. A permissively licensed subset, SchemaPile-Perm, is
  published; its size and licences are to be read from its record.
  [paper](https://ir.cwi.nl/pub/34763/34763.pdf)
- **Database visualization** (ICDT 2025) maps a database's contents and
  constraints to visual form, extending single-table grammars to foreign keys,
  and argues common designs follow from data modelling decisions. Theory, not
  a recommender — and the nearest thing found to suggesting views over several
  tables.
  [paper](https://drops.dagstuhl.de/storage/00lipics/lipics-vol328-icdt2025/html/LIPIcs.ICDT.2025.35/LIPIcs.ICDT.2025.35.html)

## Language models, for contrast

**LIDA** (Dibia, 2023) summarizes data, proposes goals and writes the
plotting code with a language model; **nvBench** (2021, 780 tables) and
**nvBench 2.0** (NeurIPS 2025, 7 878 questions, 24 076 charts, 153 domains)
and **VisEval** (146 databases) measure natural language to chart. They are
the other end of lemmesee's budget: they show what a large model can do, and
what lemmesee refuses to carry.
[LIDA](https://arxiv.org/pdf/2303.02927),
[nvBench 2.0](https://arxiv.org/html/2503.12880v2)

## What follows

- **The plan's claim of novelty shrinks to three things**: recommending from
  a schema and the database's own statistics before reading a row, over
  several related tables; counting the cost on the live database and
  abstaining, beside ScaleViz's budgeted statistics; and a ranker whose size
  and memory are bounded. The judged set of multi-table schemas would be new
  if it can be published.
- **To read whole next**: ScaleViz, SeeDB, Lux and the ICDT 2025 paper, and a
  search for 2024–2026 work on recommending from schemas and catalogs that
  this round's searches did not reach.
- **The plan's numbers** are corrected where this round read them; SchemaPile-
  Perm's count and licences are still to be read from its record.

## How this round was made

Web searches on 27 October 2026, their results read as the search returned
them, with each work's own page or paper linked; no paper was read whole, and
a figure quoted here is the abstract's or the project page's. Nothing was
run.
