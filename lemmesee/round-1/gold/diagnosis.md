# Diagnosis of the gold questions

Written by `diagnose.py` from the last run of `see.sh` on each development
fixture. **in the five** — the question is among the configuration's five;
**rank** — generated and allowed, outranked; **abstained**, **removed** — by
the code given; **not generated** — a template could say it and the
generator did not; **inexpressible** — no template can, for the reason given.

## chinook, postgres

| # | Question | O | A | B1 | Run |
|---|---|---|---|---|---|
| 1 | Revenue month by month | rank 23 | in the five, #1 | in the five, #1 | 60 rows |
| 2 | Invoices month by month | rank 22 | rank 5 | rank 4 | — |
| 3 | Revenue by billing country | rank 40 | rank 4 | in the five, #2 | 24 rows |
| 4 | Revenue by genre | inexpressible: a measure that is an expression; two joins | | | |
| 5 | Revenue by genre, month by month | inexpressible: a measure that is an expression; three joins | | | |
| 6 | The twelve best-selling tracks | rank 81 | rank 19 | rank 14 | — |
| 7 | The twelve customers who spend the most | inexpressible: a label for a person, which the rules refuse | | | |
| 8 | Tracks by genre | rank 89 | rank 41 | rank 29 | — |
| 9 | Customers by country | rank 6 | rank 44 | rank 43 | — |
| 10 | Average invoice, month by month | rank 24 | rank 2 | rank 2 | — |
| 11 | Revenue by support representative | inexpressible: two joins; a label for a person, which the rules refuse | | | |

## airlines, postgres

| # | Question | O | A | B1 | Run |
|---|---|---|---|---|---|
| 1 | Bookings day by day | rank 7 | rank 13 | rank 10 | — |
| 2 | Booked revenue day by day | rank 8 | in the five, #3 | in the five, #3 | 91 rows |
| 3 | Flights day by day | rank 16 | in the five, #5 | in the five, #5 | 18 rows |
| 4 | Flights by status | rank 14 | in the five, #1 | in the five, #1 | 7 rows |
| 5 | Segment revenue by fare class | rank 43 | in the five, #2 | in the five, #2 | 3 rows |
| 6 | Passengers per flight | rank 38 | rank 19 | rank 20 | — |
| 7 | Flights by departure airport | inexpressible: a key the schema does not declare: routes has no primary key; two joins; a label in jsonb | | | |
| 8 | Average departure delay, day by day | inexpressible: a measure that is an expression of two times | | | |
| 9 | Tickets per booking | rank 46 | rank 41 | rank 34 | — |
| 10 | Booked revenue by fare class, day by day | inexpressible: a measure over time split by a category | | | |

## ergastf1, mysql

| # | Question | O | A | B1 | Run |
|---|---|---|---|---|---|
| 1 | Races season by season | rank 141 | in the five, #2 | in the five, #2 | 68 rows |
| 2 | Points by constructor, the twelve most | rank 231 | in the five, #3 | in the five, #3 | 12 rows |
| 3 | Points by driver, the twelve most | inexpressible: a label for a person, which the rules refuse | | | |
| 4 | Wins by constructor, the twelve most | inexpressible: a filter on the rows | | | |
| 5 | Races by circuit, the twelve most | rank 140 | in the five, #1 | in the five, #1 | 12 rows |
| 6 | Results by finishing status | rank 253 | rank 109 | removed: H3 | — |
| 7 | Pit stops season by season | rank 75 | rank 46 | rank 42 | — |
| 8 | Average pit stop, season by season | rank 78 | rank 47 | rank 43 | — |
| 9 | Drivers by nationality | rank 50 | rank 121 | rank 79 | — |
| 10 | Results season by season | rank 143 | rank 22 | rank 16 | — |

## Totals

| | Questions |
|---|---|
| best in A: in the five | 8 |
| best in A: outranked | 14 |
| best in B1: in the five | 9 |
| best in B1: outranked | 12 |
| best in B1: removed | 1 |
| inexpressible | 9 |
