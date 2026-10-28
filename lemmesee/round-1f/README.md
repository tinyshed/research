# Round 1f: twice the databases

**Status: built and run on 28 October 2026 by the agent, at Dmitry's
request; results in [the report](../reports/scale-2026-10-28.md).** Round 1e's [look and choice](../round-1e/README.md)
unchanged; what changes is how many databases teach it, to see whether the
count of databases is what limits it.

## Why

Round 1e's gain on its ten held-out databases came from each of its three
changes, and taught on round 1d's 28 alone it kept 0.4 to 0.5 fewer of five
there; on the 28 it only matched round 1d. Fifty-one databases are few for a
model meant to meet any database, and a cheap teacher makes labels no
longer the limit.

## What changes

- **Databases**: 42 more, from BIRD's training and development sets
  (bird-bench.github.io, CC BY-SA 4.0), loaded from SQLite to PostgreSQL by
  `round-1/fixtures/sqlite.py` and `bird.sh`, so that a third of all are on
  PostgreSQL. Left out: what CTU already serves under another name (pubs,
  classicmodels, CraftBeer, SFScores, Hockey, nhl_draft, Mondial,
  MovieLens, Basketball, Yelp, restbase, Northwind, SalesDB, Shakespeare,
  Dallas, SAP, Student_loan, world, Walmart, Countries, ccs, ErgastF1,
  medical, Toxicology, CDESchools), the pilot fixtures' own (AdventureWorks,
  Sakila, `financial`, Stack Exchange's), sets made for learning (citeseer,
  trains, genes), and the four of more than 4 GB.
- **Twelve held out**, drawn with seed 20261031 before any was loaded:
  address, card_games, coinmarketcap, cookbook, human_resources,
  legislator, menu, movie, soccer_2016, superstore, synthea, video_games.
  The other thirty, and round 1e's ten now that they are counted, teach:
  81 databases.
- Teachers, counters, the look's numbers and the tuning are round 1e's:
  Jev, DeepSeek and GLM teach; Sonnet and Gemini count the twelve; Opus
  counts round 1d's 28 from its answers there.

## How it is measured

- **On the twelve**, once: round 1f's five against round 1e's as it was,
  taught on its 41, and against rounds 1, 1c and 1d.
- **A learning curve on the twelve**: the look taught on 10, 20, 40, 60 and
  all 81 databases, five draws of each smaller size, with the penalties and
  weight round 1e's tuning chose, counted by Sonnet and Gemini.
- **On round 1d's 28**, each left out: round 1f against round 1e and round
  1d, by Opus, Sonnet and Gemini.

## Files

| Path | What |
|---|---|
| `look.py` | round 1e's look over three folders of pools: the fives on the twelve and on the 28, and the learning curve |

The pools, answers and models live in `lemmesee/corpus/round-1f/`, which git
ignores; the answers and measures are copied beside the report.
