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

Two passes, the second after fixing what the first showed the rules got
wrong ([candidates.md](../candidates.md#revised-on-the-development-fixtures)):

| | First pass | After the fixes |
|---|---|---|
| inexpressible: no template can say it | 9 | 9 |
| not generated: a template could, the generator did not | 4 | 0 |
| removed by a hard rule | 3, all wrongly | 1, rightly: 134 finishing statuses |
| outranked in B1 | 12 | 18 |
| in B1's five | 3 | 3 |
| in A's five | 4 | 4 |

**The order loses the most: 18 of the 22 questions the generator can say
are outranked.** Revenue by country is 27th on Chinook, tracks by genre 20th,
points by constructor 49th on ErgastF1. Dozens of candidates score within a
point of each other: the penalties weigh how a panel is known, not whether
it matters, and nothing says which table is the database's subject, so the
lines of an invoice and the tracks of a catalog crowd out the invoice
itself. More penalties will not fix that.

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
