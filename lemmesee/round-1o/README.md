# Round 1o: the numbers checked, and the charts a product asks for

Status: run on 30 October 2026 as a development probe; not reported, not
committed, the owner's judgment of the new page not yet given.
[Reported](../reports/screen-2026-10-30.md) with rounds 1l to 1o.

Round 1n's row of numbers matched six of the eight on the owner's own
dashboard. This round checks it on the bench, fixes what the check showed,
and draws the charts the owner and the owner's bot asked for that no
question of the generator makes.

## The gold numbers (`gold.py`, `match.py`)

For each of round 1m's twelve bench schemas, a Haiku agent wrote the six to
eight numbers the person running that application would open the weekly
screen with — what, over which table and column, filtered how, over how
many days, divided by what — from the schema and round 1m's sentence of
what the application is, never seeing round 1n's row or the stories. A gold
number is found when the row holds one of the same kind over the same table,
with its counted column where both name one and its window where the gold
gives one; a share by its top.

| Row | Gold numbers found | Our numbers in the gold |
|---|---|---|
| round 1n as committed | 21 of 87 (24 %) | 24 of 73 (33 %) |
| + the thing done in 7 and 30 days, a month's active people | 37 of 87 (43 %) | 47 of 110 (43 %) |
| + money named run together, amounts before taxes; keys named for a table | 44 of 87 (51 %) | 57 of 119 (48 %) |
| + people the activity points at first; a payments table's money beside the thing's | 46 of 87 (53 %) | 59 of 124 (48 %) |

The changes, all to round 1n's `kpi.py`: the thing done and its active
people over 7 and 30 days; a money column found by its name even when its
words run together (`TOTALPRICE`), amounts and totals before fees, never a
tax, a discount or a rate, the thing's own money first and a table of
payments or orders beside it; keys round 1c's reader left unguessed, by a
column named for a table (`idUser`, `CustomerId`); people ranked by how many
tables with a time of their own point at them, so a table of logins' users
gives way to the customers the rentals point at; a form holding its maker's
name no longer counts as people; a table of links leading from people to the
thing; and failed words for money owed (overdue, defaulted, unpaid, a
no-show). On the owner's dashboard database the six numbers still come out
as before.

What is still missed: a member table the schema does not have; averages
(an award, a price a day); a sum of a second money column; a share with a
filter of its own; numbers over a lookup's rows.

## The product's charts (`charts.py`)

- **Health as a share**: the share of a day's rows whose status is a failed
  value, read from the database first, over the last 28 days of the table's
  own time — on the table and status round 1m's screen split by, so the
  ranker's choice holds; where no failed value is found the split stays.
- **Speed as a percentile**: the 95th percentile a day of the delay round
  1m's screen averaged, in seconds, minutes or hours as its values need
  (PostgreSQL; MySQL keeps the average).
- **The week's money by its kind**: a money column of the busiest timed
  table with money, by a column named as a kind, type, channel, method,
  plan, model or the like, over the last seven days.
- **The pulse split by where it happened**: the pulse drawn split by a
  column named as a kind of its own table's or, first, of the table it hangs
  on — a chat's kind, a channel, a membership — after the owner asked for
  the messages a day and how many of them were private.

On the bench round 1o's screen shows 17 of the 29 stories over time,
against 16 for round 1m's: health as a share reads the same status a split
read, and the gain is in what the chart says, not in what it reads. Its new
charts took a place on 4 of 12 screens (the failed share), 1 (money by kind)
and 0 (the percentile, no speed being on the bench's screens).

On the owner's busiest database round 1o's screen holds the messages a day
split by private and group chats, a day's cost, the failed share of effects
a day, the week's cost by model and the 95th percentile of a turn a day; on
the other, its data being sparse, mostly single points.

## Reproduction

From the research repository's lemmesee folder, round 1's containers up, in
round 1b's environment:

    python -I round-1o/gold.py prompts corpus/round-1m/bench 3
    # four Haiku agents, each told to read gold-prompts/agent-<k>.txt and write the numbers it names
    python -I round-1o/gold.py check corpus/round-1m/bench
    round-1b/.venv/Scripts/python -I round-1o/match.py corpus/round-1m/bench
    round-1b/.venv/Scripts/python -I round-1o/charts.py bench corpus/round-1m/bench corpus/round-1l/learn
    round-1b/.venv/Scripts/python -I round-1o/charts.py live corpus/prod <fixture> <ssh target> <container> corpus/round-1l/learn
    round-1b/.venv/Scripts/python -I round-1k/live.py page corpus/prod <fixture>...

The gold rows, the counts and every number of the owner's databases stay
in the ignored corpus.
