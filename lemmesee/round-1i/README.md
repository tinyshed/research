# Round 1i: what the look costs the database

**Status: run on 28 October 2026; [reported](../reports/cost-2026-10-28.md):
the best ten read, a large table sampled and half a second a statement
keep the five and cut the database's time over 93 databases from 536 s to
16 s.** Round 1g found the look's five, judged as a set, within
a fifth of a panel of the five its teachers would choose; what the five is
worth is no longer where the round's question is. The plan's second
question is: when does a bounded look at rows improve the five enough to be
worth its cost? Round 1f measured the price on 93 databases — thirty
statements each, 6.8 s per database in the median and two minutes on
SalesDB's 6.7 million rows — but never bounded it.

No language model is asked anything in this round. Every five is counted
with the answers the judges that never teach already gave, panel by panel,
on the 50 databases they answered.

## What is measured

1. **How many panels to look at.** The look runs round 1c's best k questions
   and chooses the five among them, k from 5 to 30; the kept of five by
   each judge, and the database's time, against k.
2. **Knowing the price before paying it.** Each statement's plan, asked of
   the database without running it — PostgreSQL's and MySQL's estimated
   cost — against the time it then takes. How well the estimate finds the
   slow ones decides whether a budget can be kept without running anything.
3. **A sample instead of the table.** Each statement run on a sample of its
   base table: on PostgreSQL `TABLESAMPLE SYSTEM`, whole pages at random; on
   MySQL, which has none, twenty runs of the primary key from random
   starting points, the same idea by key. At 1 % and 10 %, counts and sums
   scaled back. The look scores the sampled results with the same models,
   taught on full results, out of fold by family as in round 1g; the five it
   chooses, its kept of five, and the time saved, for samples taken only of
   tables above a size.
4. **A policy.** From 2 and 3, a rule a program can follow: run a statement
   whole when its estimate is under a budget, on a sample when its table is
   large, and leave it out when neither fits; its kept of five against its
   time.

The judges' answers were given on full results, so a sampled panel is
counted as the full one was. What a sample changes is what the look
chooses; that is what is counted.

## Files

| Path | What |
|---|---|
| `cost.py` | `run`: each pool statement explained, run whole and on samples, timed; `measure`: the curves, the estimates and the policy; `scale`: one database's statements at other sizes |
| `retails-x10.sql` | BIRD's retails with ten times its orders and their lines, for `scale` |

Outputs live in `lemmesee/corpus/round-1i/`, which git ignores. It needs
round 1's two servers, and its own environment, `round-1i/.venv`, for the
database drivers.
