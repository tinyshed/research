# Diagnosis of the gold questions

Written by `diagnose.py` from the last run of `see.sh` on each fixture.
**in the five** — the question is among the configuration's five;
**rank** — generated and allowed, outranked; **abstained**, **removed** — by
the code given; **not generated** — a template could say it and the
generator did not; **inexpressible** — no template can, for the reason given.

## northwind, postgres

| # | Question | O | A | B1 | Run |
|---|---|---|---|---|---|
| 1 | How much net revenue did we bill each month? | inexpressible: a measure that is an expression | | | |
| 2 | How many orders were placed each month? | rank 105 | rank 5 | in the five, #2 | 23 rows |
| 3 | Which 12 products earn the most net revenue? | inexpressible: a measure that is an expression | | | |
| 4 | Which 12 customers spend the most, by net revenue? | inexpressible: a measure that is an expression; two joins | | | |
| 5 | How many orders shipped after their required date, per month? | inexpressible: a filter on the rows | | | |
| 6 | How did net revenue by product category move each quarter? | inexpressible: a measure that is an expression; three joins; a measure over time split by a category | | | |
| 7 | Which 12 employees book the most net revenue? | inexpressible: a measure that is an expression; two joins; a label for a person, which the rules refuse | | | |
| 8 | What is the average discount given each month? | rank 33 | rank 23 | rank 12 | — |
| 9 | How much freight did each shipper charge each quarter? | inexpressible: a measure over time split by a category | | | |
| 10 | How many active products are at or below their reorder level, by category? | inexpressible: a filter on the rows | | | |

## ccs, mysql

| # | Question | O | A | B1 | Run |
|---|---|---|---|---|---|
| 1 | How much revenue comes in each month, by customer currency? | inexpressible: a measure over time split by a category | | | |
| 2 | How many transactions happen each month? | rank 30 | rank 10 | rank 10 | — |
| 3 | How much fuel do customers consume each month? | inexpressible: a month held as an integer, YYYYMM | | | |
| 4 | Which products bring in the most revenue? | not generated | | | |
| 5 | Which customers spend the most? | inexpressible: an identifier for a label | | | |
| 6 | Which gas stations take the most revenue? | inexpressible: an identifier for a label | | | |
| 7 | How many transactions happen in each country? | rank 45 | rank 39 | rank 22 | — |
| 8 | How many transactions come from each customer segment? | rank 39 | rank 44 | rank 12 | — |
| 9 | How much fuel does a customer use in a typical month, by segment? | not generated | | | |
| 10 | What is the average transaction value each month, by currency? | inexpressible: a measure over time split by a category | | | |

## lahman, mysql

| # | Question | O | A | B1 | Run |
|---|---|---|---|---|---|
| 1 | How many runs does an average team score each season? | inexpressible: a year held as an integer, which the rules take for no time | | | |
| 2 | How much does an average player earn each season? | inexpressible: a year held as an integer, which the rules take for no time | | | |
| 3 | How many home runs does an average team hit each season? | inexpressible: a year held as an integer, which the rules take for no time | | | |
| 4 | What is the average attendance per team each season? | inexpressible: a year held as an integer, which the rules take for no time | | | |
| 5 | How many teams play in each season? | inexpressible: a year held as an integer, which the rules take for no time | | | |
| 6 | Which franchises have won the most games in total? | not generated | | | |
| 7 | Which players have hit the most home runs in their careers? | inexpressible: a label for a person, which the rules refuse | | | |
| 8 | Which pitchers have struck out the most batters in their careers? | inexpressible: a label for a person, which the rules refuse | | | |
| 9 | Which awards are given most often to players? | not generated | | | |
| 10 | Which countries have produced the most players? | rank 194 | rank 34 | rank 23 | — |

## Totals

| | Questions |
|---|---|
| best in A: outranked | 6 |
| best in B1: in the five | 1 |
| best in B1: outranked | 5 |
| inexpressible | 20 |
| not generated | 4 |
