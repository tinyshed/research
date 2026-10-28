# Questions a schema can be asked, ordered by what teachers ask, 28 October 2026

Round 1c, as [its description](../round-1c/README.md) sets it, built and run
overnight by the agent: round 1's templates replaced by **the analytical
question** — a base, a measure that may be an expression, a time and a
breakdown reached through many-to-one joins — and round 1's penalties by
**a ranker of 55 KB learned from what two language models, Claude Haiku and
Gemini 3.8 Flash, ask first** about 480 public schemas. Measured at research
commit `a96225a` with this round's files uncommitted; raw output and the
teachers' answers in [data/questions-2026-10-28/](data/questions-2026-10-28/).

| Question | Answer | What follows |
|---|---|---|
| Can the generator say what people ask? | **Round 1 could say 39 to 44 % of the teachers' questions, round 1c 78 to 85 %**; of round 1's gold questions, 28 of 31 on the development fixtures against 22, and 29 of 30 on the check fixtures against 10 | the analytical question replaces the templates |
| Does a ranker learned from teachers choose what they would? | On schemas it never saw, **it chooses what a teacher asks 1.8 times as often as round 1's rules: precision 0.536 against 0.298, the difference's 95 % interval [+0.205, +0.273], and recall 0.354 against 0.181**, on 240 schemas held out | the ranker replaces the penalties |
| How close is that to the teachers themselves? | Two teachers agree on 0.71 to 0.73 of a five; the ranker reaches about three quarters of that | more teacher data moved it from 0.54 to 0.59 in NDCG between a quarter of the schemas and all; it is still rising |
| And on databases with data? | 17 gold questions in round 1c's fives over eight fixtures against round 1's 13: 10 against 9 on the development fixtures round 1 was tuned on, 3 against 1 on the check fixtures | a gain, smaller than against the teachers, since one gold list a fixture is ten questions |
| On two databases nobody looked at? | 4 against 3, of ten places; round 1c can say 18 of their 20 gold questions | what it loses there is the order between a count and a sum over one time |
| Does it fit? | 600 trees of seven leaves: **55 KB packed**, read back without LightGBM to within 3·10⁻⁸; round 1b's key model beside it is 0.9 MiB | a reader of a hundred lines in Rust |

## What was built

| Part | What it is | Where |
|---|---|---|
| the analytical question | base; measure — rows, a column's sum or average, a row's amount net of its discount, a line's quantity × its product's price, parents counted once, rows with a flag, days between two times, rows late; a time of the base or of its header, an integer year or `YYYYMM`; a breakdown of the base or of a table up to three many-to-one joins away, a parent's name or key; five shapes; compiled to SQL for both engines, rows deleted softly left out | `aq.py` |
| the teachers | 480 SchemaPile-Perm schemas that look like applications; Gemini 3.8 Flash through `agy` on all, Claude Haiku on 150; ten questions each, first five for the first screen, never seeing the generator | `teacher.py` |
| the ranker | about 90 features of tier S, the schema's other tables among them; LightGBM LambdaRank, 600 trees of seven leaves; with statistics, vetoes and a hand layer for size | `rank.py`, `learn.py` |
| its bytes | packed at ten bytes a node and four a leaf, read back by 60 lines of Python | `pack.py` |
| the page | round 1's five beside round 1c's, run on each fixture, gold questions marked, each panel's reasons the ranker's largest SHAP values | `see.py` |

## The generator's reach

A gold or teacher question is said when the generator makes a question of
the same identity: base, measure's aggregate and columns, time column,
breakdown column or the parent it names.

| Questions | Round 1 | Round 1c |
|---|---|---|
| round 1's gold, development fixtures (31) | 22 | **28** |
| round 1's gold, check fixtures, written blind (30) | 10 | **29** |
| Haiku's on 150 teacher schemas (1 500) | 39 % | **78 %** |
| Gemini's on the first 240 teacher schemas (2 400) | 44 % | **85 %**; 83 % of all 4 800 |
| the final fixtures' gold, written blind (20) | — | **18** |

What it still cannot say: filters other than a flag or a soft deletion (a
win as `position = 1`, a product at its reorder level); measures that are
differences of two amounts (box office less budget); a parent's column
summed over its children; many-to-many breakdowns (films by category through
`film_category`); and a split by one of many values, genre by month.

## The ranker against the teachers

Five folds by repository over the first 234 teacher schemas; each schema's
five chosen by the same rules for every system.

| | Haiku: precision | recall | Gemini: precision | recall | NDCG@5 |
|---|---|---|---|---|---|
| round 1's rules, configuration A | 0.295 | 0.188 | 0.330 | 0.209 | — |
| round 1c's hand prior | 0.336 | 0.221 | 0.327 | 0.208 | 0.30 |
| **round 1c's ranker** | **0.527** | **0.356** | **0.555** | **0.386** | **0.59** |
| one teacher's five in the other's ten | 0.707 | | 0.726 | | |

Precision is the share of the five a teacher asked anywhere in its ten,
recall the share of the teacher's first five in the five. The ceiling the
generator and the rules of choice allow, the teachers' own labels as scores,
is 0.72 and 0.86.

Tried and not kept, each on the same folds: rates per word of what teachers
ask (no gain at 138 schemas), a second stage over the first's scores in the
manner of round 1b's graph (−0.01), a classifier or `rank_xendcg` in place of
LambdaRank (within 0.01), larger or smaller trees (within 0.01), a cap of two
panels of one kind (−0.06: the teachers show several counts over time
themselves). More teachers' schemas helped every time: NDCG@5 0.54, 0.57,
0.57, 0.59 at a quarter, half, three quarters and all of the training folds.

## Once, on schemas never looked at

The second wave of 240 teacher schemas was chosen after the first, by the
same rule from the same shuffle, and answered by Gemini alone. The ranker
was fitted on the first wave only and measured on the second once, beside
round 1's rules and round 1c's hand prior on the same schemas; each
schema's difference bootstrapped, 10 000 times.

| | Precision | Recall |
|---|---|---|
| round 1's rules, configuration A | 0.298 | 0.181 |
| round 1c's hand prior | 0.319 | 0.201 |
| **round 1c's ranker** | **0.536** | **0.354** |
| ranker − round 1 | **+0.238**, 95 % [+0.205, +0.273] | **+0.173**, [+0.147, +0.200] |

The ranker chooses what a teacher asks 1.8 times as often as round 1's
rules, and finds twice as many of its first five. Fitted on all 480
schemas, five folds by repository give 0.507 and 0.329 against Haiku and
0.545 and 0.365 against Gemini; the two teachers agree on 0.71 and 0.73.

## On the six fixtures

Gold questions in the five, of fifteen places a group of three fixtures.
Round 1's five is its B1 after the subjects of 28 October, tuned on the
development fixtures by looking at them; round 1c was developed looking at
all six, but never fitted to them.

| Fixture | Round 1, B1 | Round 1c, A | Round 1c, B1 |
|---|---|---|---|
| Chinook | 2 | 1 | 2 |
| airlines | 4 | 3 | 4 |
| ErgastF1 | 3 | 4 | 4 |
| **development** | **9** | **8** | **10** |
| Northwind | 1 | 2 | 2 |
| ccs | 0 | 1 | 0 |
| Lahman | 0 | 1 | 1 |
| **check** | **1** | **4** | **3** |

Round 1c matches round 1 where round 1 was tuned, and does better where it
was not. Every one of round 1c's eighty panels, on all eight fixtures, ran
on its database within the five seconds it was given; the slowest, over
the airlines' 3.9 million segments, took 3.8 s through Docker's client.
What the five still misses there:
Lahman's averages of team statistics by season, whose columns are `R`, `HR`,
`SO`, named only for someone who knows baseball; ccs's two money-like
columns, `Amount` and `Price`, of which it sums the one the teacher did not;
counts and sums over one time, which the rule of one time column keeps from
standing side by side.

## Once, on two databases nobody looked at

classicmodels and Credit were loaded after the ranker's last change, Haiku
wrote ten questions for each from its schema with rough row counts, and
round 1's chain and round 1c's ranker, fitted on all 480 teacher schemas,
ran on them once. Nothing changed after.

| | Gold questions round 1c can say | Round 1, B1 | Round 1c, A | Round 1c, B1 |
|---|---|---|---|---|
| classicmodels | 8 of 10 | 1 | 2 | 2 |
| Credit | 10 of 10 | 2 | 2 | 2 |
| **in the five, of 10 places** | | **3** | **4** | **4** |

The two it cannot say are a margin, `(priceEach − buyPrice) × quantity`
over two tables, and revenue by product line month by month, a split by a
parent's name. What it lost was mostly the order between a count and a sum
over one time: Credit's first question, the money charged each month, is
eighth, behind the number of charges each month, which takes its time
column. Over all eight fixtures round 1c's B1 puts 17 gold questions in
its fives and round 1's 13.

## What it means, and what does not follow

- **The teachers are language models, not people.** The ranker learned
  their taste, and the check fixtures' and the final fixtures' gold was
  written by one of them. Round 1's gold on the development fixtures is the
  agent's, another model again. That two teachers agree on 0.71 of a five is
  the nearest thing to a ceiling here; people may agree less, or on other
  things. Round 2's judged set is what says whether any of this is what a
  person keeps.
- **Tier S is what the ranker learned**; SchemaPile has no rows. With
  statistics it adds vetoes and a small hand layer. A ranker that learns
  from statistics needs schemas with data: round 1's generated rows, or
  BIRD's databases.
- **The generator now says enough that the order matters again**, and the
  order's gains come from teachers' data, not from rules: the learning curve
  is still rising.

## What follows

1. Carry the analytical question and the packed ranker into Rust, beside
   round 1's harness, so `see` runs it.
2. More teacher schemas and a third teacher, and labels from teachers that
   see the data's statistics, through generated rows.
3. Round 2's judged set, with round 1c's five beside round 1's, so the
   ranker is measured against people.
4. The generator's remaining reach: filters, differences, many-to-many.

## Reproducing

```bash
python -I round-1c/teacher.py select corpus/schemapile-perm/12682521/schemapile-perm.json.gz corpus/round-1c/teacher 480
python -I round-1c/teacher.py batches corpus/round-1c/teacher 6
round-1b/.venv/Scripts/python -I round-1c/learn.py build corpus/round-1c/teacher corpus/round-1c/learn
round-1b/.venv/Scripts/python -I round-1c/learn.py cv corpus/round-1c/learn
round-1b/.venv/Scripts/python -I round-1c/evaluate.py corpus/round-1c/learn corpus/round-1c/teacher round-1/harness/target/release/rules.exe s240
round-1b/.venv/Scripts/python -I round-1c/learn.py fit corpus/round-1c/learn
round-1b/.venv/Scripts/python -I round-1c/pack.py corpus/round-1c/learn
round-1b/.venv/Scripts/python -I round-1c/see.py corpus/round-1c/learn corpus/see corpus/round-1c/see round-1/gold/questions.json round-1/gold/check.json round-1c/final.json
```

Between the second and third commands the teachers answer each prompt in
`corpus/round-1c/teacher/batches/`: Gemini through `agy -p` with
`--model gemini-3.8-flash-high`, Haiku as an agent that reads one prompt
and writes its answer. Their answers are not reproducible by asking again;
they are kept, with the schemas' keys, in
[the data](data/questions-2026-10-28/) as `teachers.jsonl.gz` and
`schemas.jsonl.gz`. Measured on Windows 11 with Python
3.13, LightGBM 4.7.0, PostgreSQL 18.6 and MySQL 8.4.13 in Docker; any time
quoted is an anecdote.
