# Gold questions

What a person would want answered first about each development fixture —
Chinook, the airlines demo, ErgastF1 — and where round 1's pipeline loses
each one. **The questions are a draft the agent wrote on 28 October 2026 for
Dmitry to edit**; they diagnose the pipeline and are never trained on, and
the development fixtures they belong to are never pilot fixtures.

| File | What |
|---|---|
| `questions.json` | 31 questions: base, grain, measure, time, breakdown, the joins to reach them, what the generator lacks, the candidate that would answer |
| `diagnose.py` | each question against the last `see.sh` run: inexpressible, not generated, removed, abstained, outranked, or in a configuration's five |
| `diagnosis.md` | its output, as last run |

## Where the questions are lost, 28 October

Three passes: the second after fixing what the first showed the rules got
wrong ([candidates.md](../candidates.md#revised-on-the-development-fixtures)),
the third after giving the order the database's subjects
([candidates.md](../candidates.md#the-databases-subjects)):

| | First pass | After the fixes | After the subjects |
|---|---|---|---|
| inexpressible: no template can say it | 9 | 9 | 9 |
| not generated: a template could, the generator did not | 4 | 0 | 0 |
| removed by a hard rule | 3, all wrongly | 1, rightly: 134 finishing statuses | 1 |
| outranked in B1 | 12 | 18 | 12 |
| in B1's five, of 15 places | 3 | 3 | **9** |
| in A's five, of 15 places | 4 | 4 | **8** |

**The order lost the most: after the fixes, 18 of the 22 questions the
generator can say were outranked.** Revenue by country was 27th on Chinook,
points by constructor 49th on ErgastF1: the penalties weighed how a panel is
known, not whether it matters, and nothing said which table the database is
about, so the lines of an invoice and the tracks of a catalog crowded out the
invoice itself.

**The subjects** — a table with a time of its own that facts hang on — put
B1's five at revenue by month and by country on Chinook; flights by status
and by day, segment revenue by fare class and booked revenue on the airlines;
races by season and by circuit and points by constructor on ErgastF1. What
is still outranked is the questions about the things rather than what
happened to them — customers by country, drivers by nationality, tracks by
genre — and the second fact of a subject, ErgastF1's pit stops. Chinook's
best-selling tracks are in the five as revenue by track, not as the
quantity the gold names: Chinook's lines all sell one.

These rules were chosen on the same three databases: the gain says they fit
what was looked at, not that they generalise.

**The generator cannot say 9 of 31**, and the reasons repeat; a question
may lack more than one:

| What it lacks | Questions |
|---|---|
| more than one join — genre through track, an employee through a customer, an airport through a route | 4 |
| a person named, which the rules refuse — a customer, a support representative, a driver | 3 |
| a measure that is an expression — a line's `unit_price × quantity`, a delay as two times' difference | 3 |
| a filter on the rows — wins as `position = 1` | 1 |
| a measure over time split by a category | 1 |
| a key nobody declared, to a table with no primary key | 1 |
| a label inside `jsonb` — an airport's name by language | 1 |

**Drawing**, fixed: a time axis placed by each bucket's date, so a missing
bucket leaves a gap; the rest of a bar chart added up only where its values
add, never for averages; a group by a parent's name grouped by the parent's
key as well, so two artists of one name stay two.

## What follows

The penalties have done what they can. What the diagnosis asks for is the
**analytical question** as an object between the schema and the chart —
base, grain, a measure that may be an expression, time and breakdown reached
through chains of many-to-one joins that keep the grain, a population, a
unit, and evidence for each — compiled to SQL, and **an order that knows the
database's subjects** and chooses a set rather than five singles. The gold
questions are the test of both.
