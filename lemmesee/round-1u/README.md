# Round 1u: see's questions through a model's keys

Written on 9 November 2026 at 13:51 UTC, before anything here was run. The
key model's paper is framed by `see`: round 1c's analytical question
(`round-1c/aq.py`) reaches a breakdown, a header's time and a product's
price through foreign keys, many to one. This round puts one number on
that: what the questions `see` asks become when the keys come from round
1t's model rather than the schema's declarations. It chooses no model; the
sets are development data.

## The three runs

On every database of Spider, BIRD, CTU and LiveSQLBench, as round 1t reads
them (`fit.sets()`), each schema is written as the snapshot `aq.load` reads
(tables, columns and their types, primary and unique keys; no statistics,
`reads_n` off) three times:

- **declared**: the schema's declared one-column keys, and nothing guessed
  (`aq._guess_keys` turned off);
- **rules**: no declaration, `aq._guess_keys` as `see` runs it (a column
  named `<table>id`, `<table>_id`, `<table>key`, `<table>code` or
  `<table>no`, or named as a table's one-column key, of a compatible type —
  round 1's rule in `round-1/candidates.md`, widened as round 1c wrote it);
- **model**: no declaration, the keys round 1t's `seven-restyled` draws at
  its threshold, as frozen in `frozen-final.json` (hashes checked), and
  nothing guessed.

`aq.generate` makes each run's questions, `rank.veto` removes what it
removes, as `see.py` does. A question is its identity (`aq.identity`) and
its shape; it **joins** when its time or its breakdown is reached through a
key, or its measure needs one (parents counted, rows per parent, a line's
quantity by its product's price); the keys it goes through are each step
`(table, column)` into the table the key names.

## The measures

Per set, over the questions of the declared run that join:

- **still asked**: the share the run asks too (the same identity and shape);
- **lost**: the rest;
- and, of the run's own questions, how many go through a key no declaration
  names. Whether such a key is wrong or a real undeclared one is not told
  apart: no set here marks its undeclared keys.

Each share is pooled over the set's databases, with a 95 % interval from
1 000 resamples of databases (seed 20261101); the share still asked also as
the mean of each database's own (added before the full run, after Spider's
first run showed a few databases of thousands of questions carrying the
pooled share), over the databases with a joining question. Beside it, **the five**: round
1c's learned ranker (`corpus/round-1c/learn/ranker.txt`, configuration A, no
statistics, `rank.choose`) picks five of each run's questions, and the
report gives how many of the declared run's five each other run keeps.

## Run

```sh
cd <repo>/lemmesee
round-1b/.venv/Scripts/python -I round-1u/questions.py
```
