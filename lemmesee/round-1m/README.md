# Round 1m: a bench of living databases, and the five as a weekly screen

Status: run on 30 October 2026 as a development probe; not reported, the
harness not yet committed. The weekly screen over round 1l's Luna and
DeepSeek ranker shows over time 16 of the bench's 29 planted stories against
round 1c's 10, and on the owner's busiest database fills four of the five
places the owner's bot asked for.

Round 1l found the yardstick wrong: the 93 public databases are frozen
datasets, where a timeline is 6 % of the judged panels and a weekly screen
means little, while the databases the screen is for are applications in use.
And the five was a ranking where a screen is made of roles.

## The bench (`stories.py`, `bench.py`)

Twelve SchemaPile-Perm schemas that look like applications (round 1c's
filter, 5 to 14 tables, money, a status and two times), from repositories
none of round 1c's 480 came from: loans, expenses, bike and car rental, a
hotel, a shop, tuition reimbursement, a test centre and others. For each,
a Haiku agent wrote a spec: what each table is (entity, event, lookup,
technical, dead), how many rows and how they grow, the columns that matter,
two or three stories planted from day 60 to 84 of ninety — a status's
values drawn anew (cancellations, denials, no-shows rising), money
multiplied, a rate multiplied — and decoys: a table imported once, a
technical table, a table mirroring another's activity. `stories.py build`
makes each as PostgreSQL's `story_<key>` on round 1's container, seeded, 8 000
to 50 000 rows, analysed, keys declared where they reference a primary key;
round 1's `see.sh` reads it like any fixture.

A story is found when a panel of the five reads what was planted (the
changed column; for a rate, the table over a time) and **shown** when that
panel is drawn over a time: a fortnight's change does not show in ninety
days' totals.

## What it found

**The right question exists, in the wrong shape and far down.** For 28 of
the 29 stories the generator makes a question that shows it over time.
Round 1c's ranker places it, by the kind of story:

| Planted | Questions | Median place |
|---|---|---|
| a rate | the table's rows over time | 1 |
| money | a sum over time | 8 |
| a status's values | the rows over time split by the status | 64 |

Teachers ask for rows by status, not rows over time split by status, so the
health of the screen is a static breakdown where a change cannot show.

**A screen of roles** (`roles.py`): each question has a role by what it
reads — pulse (rows over time), money (a sum or average of a money word),
health (a split by a status word, or rows flagged), speed (days between two
times), people (parents counted through a key) — and the screen takes, from
all of the schema's questions by the ranker's order, the best over a time of
each role there is, then the best of the rest under round 1c's limits with
one pulse. On the bench, shown over time of 29:

| Five | Found | Shown | Decoys a five |
|---|---|---|---|
| round 1c's ranker | 0.72 | 0.34 | 0.08 |
| round 1c's ranker, screen | 0.59 | 0.48 | 0.17 |
| Luna and DeepSeek's ranker | 0.69 | 0.38 | 0.08 |
| Luna and DeepSeek's ranker, screen | 0.62 | **0.55** | 0.00 |
| Luna, DeepSeek and Jev's ranker, screen | 0.62 | 0.52 | 0.00 |
| the look over round 1d's ten | 0.62 | 0.28 | 0.08 |
| the look over the ten, roles from the ten | 0.72 | 0.31 | 0.08 |

Luna and DeepSeek's screen against round 1c's five: +6 stories shown, 95 %
over the databases [+3, +9], no database worse; the screen alone on round
1c's ranker +4 [0, +8]. Roles chosen only among a ranker's ten or thirty
(the look's pool) do not reach the splits by status, and gain nothing.

**On the 93 public databases**, roles among the look's panels keep no more
by the product judges: −0.05 [−0.18, +0.08] of five against the look, while
round 1k's seat for money keeps +0.09 [0.00, +0.18]. Most of their panels
have no role (68 %); the screen's splits over time are mostly panels the
judges never saw, so it was not counted there.

**On the owner's databases** (places only; what was asked stays on the
owner's machine): on the busiest, the screen over Luna and DeepSeek's
ranker holds one pulse, the money over time, the health split by status
over time and the speed of a turn by day — four of the five panels the
owner's bot described — where round 1c's five held three timelines of one
activity.

## The screen on the owner's databases (`screen_live.py`)

The screen over Luna and DeepSeek's ranker, run on each database's own host
through round 1k's read-only session, a panel over a time kept to its last
28 days of data by day and a speed drawn in seconds, minutes or hours as its
values need. Round 1k's `live.py` now draws a split over a time as a line a
value with a key, and puts a later round's five, marked `five: <name>` in the
pool, on the blind page beside its three; the look's candidates stay round
1k's. The page waits for the owner's judgment.

## The owner's verdict (30 October)

On the busiest database the owner chose the screen as the closest of the four
fives, its money, health, speed and pulse kept, its people panel (distinct
states of a coordinator) the one not wanted. On the other database the owner
kept none of the four and showed the product dashboard kept for it by hand:
eleven panels, of which the generator can say three.

| The owner's panel | Kind | The generator |
|---|---|---|
| accounts in all | a total, as a number | no number is ever asked |
| things made, published | a total where a status holds a value | no filter by value, no number |
| makers of things | distinct parents of an event table, as a number | only over a time or a breakdown |
| makers of two or more | parents with at least two children | only the distribution of children a parent |
| share of makers | makers over accounts | no ratio of two counts |
| rate of reuse | makers of two or more over makers | no ratio |
| new accounts in 7 days, in 30 days | a total in a window | no window |
| account → maker → repeat maker | a funnel of the three | no funnel |
| accounts by platform | a breakdown | yes |
| new accounts a day, 90 days | a pulse | yes |
| things made a day, 90 days | a pulse | yes |

A product's screen leads with numbers and the ratios between them, its
charts second; lemmesee makes charts alone. This, more than any ranker,
is what round 1n should take on: a total, distinct actors, repeat actors and
new ones in a window, each as a number and with a filter by value; the
ratios between two of them that share a parent; and the funnel they make.

## Not done

- Filters by value (`status <> 'Cancelled'`, `published = 1` beside a sum)
  and filters across two tables, which lose most of the teachers' questions
  the generator cannot say; shares (the failed share of a day's rows).
- The last days and the speed's unit for the bench and for the other fives,
  which still draw every day of their data in days.
- The look over a pool that holds the screen's candidates.

## Reproduction

From the research repository's lemmesee folder, round 1's containers up:

    python -I round-1m/stories.py pick corpus/schemapile-perm/12682521/schemapile-perm.json.gz corpus/round-1c/teacher corpus/round-1m/bench 12
    python -I round-1m/stories.py prompts corpus/round-1m/bench 3
    # four Haiku agents, each told to read prompts/agent-<k>.txt and write the specs it names
    python -I round-1m/stories.py check corpus/round-1m/bench
    python -I round-1m/stories.py build corpus/round-1m/bench m00 m01 m02 m03 m04 m05 m06 m07 m08 m09 m10 m11
    round-1b/.venv/Scripts/python -I round-1m/bench.py see corpus/round-1m/bench
    round-1b/.venv/Scripts/python -I round-1m/bench.py pool corpus/round-1m/bench

In the Linux container of round 1j:

    docker run --rm --cpus=4 -v <repo>:/src -w /src/lemmesee lemmesee-joint python round-1m/bench.py count corpus/round-1m/bench corpus/round-1l/learn
    docker run --rm --cpus=4 -v <repo>:/src -w /src/lemmesee lemmesee-joint python round-1m/bench.py reach corpus/round-1m/bench
    docker run --rm --cpus=4 -v <repo>:/src -w /src/lemmesee lemmesee-joint python round-1m/roles.py count corpus/round-1m/roles

On the owner's databases, from the host, in round 1b's environment (the
hosts and containers are in the local handoff):

    round-1b/.venv/Scripts/python -I round-1m/screen_live.py pool corpus/prod <fixture> <ssh target> <container> corpus/round-1l/learn
    round-1b/.venv/Scripts/python -I round-1k/live.py page corpus/prod <fixture>...

The specs, the databases' snapshots, the pools, the counts and every page
of the owner's data stay in the ignored corpus.
