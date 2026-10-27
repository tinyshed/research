# Diagnosis of the gold questions

Written by `diagnose.py` from the last run of `see.sh` on each development
fixture. **in the five** — the question is among the configuration's five;
**rank** — generated and allowed, outranked; **abstained**, **removed** — by
the code given; **not generated** — a template could say it and the
generator did not; **inexpressible** — no template can, for the reason given.

## chinook, postgres

| # | Question | O | A | B1 | Run |
|---|---|---|---|---|---|
| 1 | Revenue month by month | rank 23 | in the five, #2 | in the five, #3 | 60 rows |
| 2 | Invoices month by month | rank 22 | rank 2 | rank 5 | — |
| 3 | Revenue by billing country | rank 40 | rank 22 | rank 27 | — |
| 4 | Revenue by genre | inexpressible: a measure that is an expression; two joins | | | |
| 5 | Revenue by genre, month by month | inexpressible: a measure that is an expression; three joins | | | |
| 6 | The twelve best-selling tracks | rank 81 | rank 32 | rank 35 | — |
| 7 | The twelve customers who spend the most | inexpressible: a label for a person, which the rules refuse | | | |
| 8 | Tracks by genre | rank 95 | rank 13 | rank 20 | — |
| 9 | Customers by country | rank 6 | rank 72 | rank 57 | — |
| 10 | Average invoice, month by month | rank 24 | rank 18 | rank 23 | — |
| 11 | Revenue by support representative | inexpressible: two joins; a label for a person, which the rules refuse | | | |

## airlines, postgres

| # | Question | O | A | B1 | Run |
|---|---|---|---|---|---|
| 1 | Bookings day by day | rank 7 | rank 4 | rank 4 | — |
| 2 | Booked revenue day by day | rank 8 | in the five, #3 | in the five, #3 | 91 rows |
| 3 | Flights day by day | rank 16 | rank 13 | rank 14 | — |
| 4 | Flights by status | rank 14 | in the five, #5 | in the five, #4 | 7 rows |
| 5 | Segment revenue by fare class | rank 43 | rank 2 | rank 2 | 3 rows |
| 6 | Passengers per flight | rank 38 | rank 24 | rank 24 | — |
| 7 | Flights by departure airport | inexpressible: a key the schema does not declare: routes has no primary key; two joins; a label in jsonb | | | |
| 8 | Average departure delay, day by day | inexpressible: a measure that is an expression of two times | | | |
| 9 | Tickets per booking | rank 46 | rank 44 | rank 34 | — |
| 10 | Booked revenue by fare class, day by day | inexpressible: a measure over time split by a category | | | |

## ergastf1, mysql

| # | Question | O | A | B1 | Run |
|---|---|---|---|---|---|
| 1 | Races season by season | rank 115 | in the five, #1 | rank 13 | 68 rows |
| 2 | Points by constructor, the twelve most | rank 181 | rank 42 | rank 49 | — |
| 3 | Points by driver, the twelve most | inexpressible: a label for a person, which the rules refuse | | | |
| 4 | Wins by constructor, the twelve most | inexpressible: a filter on the rows | | | |
| 5 | Races by circuit, the twelve most | rank 114 | rank 9 | rank 39 | — |
| 6 | Results by finishing status | rank 227 | rank 52 | removed: H3 | — |
| 7 | Pit stops season by season | rank 67 | rank 6 | rank 7 | — |
| 8 | Average pit stop, season by season | rank 69 | rank 68 | rank 68 | — |
| 9 | Drivers by nationality | rank 44 | rank 179 | rank 85 | — |
| 10 | Results season by season | rank 117 | rank 10 | rank 14 | — |

## Totals

| | Questions |
|---|---|
| best in A: in the five | 4 |
| best in A: outranked | 18 |
| best in B1: in the five | 3 |
| best in B1: outranked | 18 |
| best in B1: removed | 1 |
| inexpressible | 9 |
