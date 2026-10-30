# Round 1n: numbers first

Status: built on 30 October 2026 as a development probe; not reported, the
harness not yet committed, the numbers not yet judged.
[Reported](../reports/screen-2026-10-30.md) with rounds 1l to 1o.

The owner's own product dashboard for one of the owner's databases opens
with eight numbers — accounts, the thing made, who made it, who made it
twice, the shares between them, the new in a week and in a month — a funnel,
one breakdown and two daily lines. Of its eleven panels the generator could
say three ([round 1m](../round-1m/README.md)). lemmesee never made a number,
for three reasons of its own making:

- round 1's templates, T1 to T8, are charts, and every round inherited them;
- round 1c's teachers were told each question is "drawn as one chart", and
  every judge since that "a single bar … is not worth a chart";
- round 1's rules abstain from a sum over a table with a status, since which
  values count is the business's (`needs-population`), and the snapshot
  holds how many common values a column has, not what they are.

## The row (`kpi.py`)

From the schema, with no model:

- **who uses the product**: the table of people (round 1c's person, or a
  name such as user, account, member, customer, client, participant) that
  the most other tables point at; with no such table, any table of ten rows
  or more;
- **the one thing it is for**: the most pointed-at table with a time of its
  own that points at them and is not itself people; where it points at them
  twice, by the key that names its maker;
- **what counts as done**: each status of that table is read from the
  database first (`GROUP BY`, thirty values), and its values whose words say
  done — published, completed, active, paid, confirmed and the like — filter
  it, unless every value does;
- **what failed**: the busiest timed table whose status holds failed values
  — failed, cancelled, rejected, refunded, expired and the like.

Then one statement each, on round 1c's live rows (soft deletion, and a row
merged into another): people in all; the thing done; people who did it,
done; people who did it twice or more; the two shares; the funnel of the
three; new people in 7 and in 30 days; the busiest timed table's money over
30 days, done; the share of failures over 7 days. Round 1k's `live.py` draws
the row above the fives on its page, not judged with them.

## What it found

**On the database the owner keeps a dashboard for**, six of the owner's
eight numbers came out exactly: the thing done, who did it, who did it
twice, the share of those twice, none new in a week, and the funnel's two
steps. The first try counted as makers everyone with a draft; the owner's
dashboard counts the published, and a draft is not the thing done, which
became the rule. The count of people differs from the owner's by a few
percent, and so the share of makers by half a point, and the new in a month
is three times the owner's; neither is explained yet: the newest account is
three weeks older than the dashboard, and a merged account is left out.

**On the owner's busiest database** the first rule chose a one-row state
table, pointed at by every table through a guessed key; people first fixed
it. The row reads people, turns done, who started a turn and who came back,
new people, a month's cost and the week's share of failed effects: three of
the five things the owner's bot asked for.

**On the bench's twelve schemas** the row finds people and the thing in 8;
on one it takes the people of a film's cast for the service's members
(`person` in a name), and on four it finds no timed table pointing at the
people by a key.

## Not done

- The numbers judged, by the owner and by the bench: no story there is a
  number yet.
- Filters by value beyond the words of a status; money by its kind over the
  week; percentiles of a delay; the owner's definition of an account.
- A person among a schema's people told from a person in its content.

## Reproduction

From the research repository's lemmesee folder, in round 1b's environment:

    round-1b/.venv/Scripts/python -I round-1n/kpi.py row corpus/see storym<k>-postgres
    round-1b/.venv/Scripts/python -I round-1n/kpi.py row corpus/prod <fixture> <ssh target> <container>
    round-1b/.venv/Scripts/python -I round-1k/live.py page corpus/prod <fixture>...

Every number of the owner's databases stays in the ignored corpus.
