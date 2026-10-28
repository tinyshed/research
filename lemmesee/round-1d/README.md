# Round 1d: the five learned, and a look at what it would show

**Status: built and run on 28 October 2026 by the agent, at Dmitry's
request; results in [the report](../reports/look-2026-10-28.md).** Round
1c's [analytical question and ranker](../round-1c/README.md) unchanged; what
changes is how five are chosen from what it ranks, and whether anything is
read of their results before they are shown — the plan's configuration D, a
bounded look at rows, before its order.

## Why

Round 1c chose its five by two rules of its own: never two panels over one
time column, never two over one breakdown. On Credit and on aimee, a
database of Dmitry's, they kept the number of charges each month and dropped
the money charged, though the teachers ask both. Two blind judges, the
morning before, dropped as well what any look at the result would have
caught: a bulk import shown as one bar, a state blank for every provider, a
timeline of nine employees.

## The five, learned

`choose.py` replaces the rules by penalties for what two panels share — the
base table; the measure; one time column with the same measure or another;
one breakdown with the same measure or another; one shape — each weighed by
coordinate ascent on how often the five agrees with the teachers' own fives,
over round 1c's 480 teacher schemas, each scored by a ranker that never saw
it, in the folds by repository round 1c uses. What two panels may share is
learned; the greedy choice over the sixty best is not.

## The look

`look.py` runs each database's thirty best questions by round 1c's ranker,
with round 1c's five and round 1d's, and reads each result for what a chart
of it would show: rows, how many buckets a time covers of those its span
needs, the share the largest value holds, distinct values, blanks among the
labels, zeros, an error or nothing. Six judges answered every one of those
panels blind, beside round 1's five — Claude Sonnet and Opus, Gemini,
DeepSeek, GLM and Jev, as [teaching.md](../teaching.md) says; **the look is a
small model of what one of them keeps** — the result's numbers, the
ranker's score and rank, and the question's features — learned over the
other databases each time one is left out, and **the five that did not
teach it count the fives**. Its probability, as log-odds times a weight
Sonnet's answers choose, is added to the ranker's score before the five is
chosen. Packed as round 1c's ranker is, it is 8.7 KB.

## The databases

Twenty-eight: round 1c's eight — Chinook, the airlines demo, ErgastF1,
Northwind, ccs, Lahman, classicmodels, Credit — and twenty more from CTU's
repository, loaded by `round-1/fixtures/ctu.sh` with a histogram on every
column: pubs, world, Dallas, restbase, voc, medical, PremierLeague,
Basketball_men, NCAA, Hockey, imdb_MovieLens, Student_loan, CraftBeer, NBA,
Basketball_women, Mondial, SFScores, Shakespeare, Atherosclerosis and ftp.
None is a pilot fixture.

## Files

| Path | What |
|---|---|
| `choose.py` | the teacher schemas scored out of fold; the penalties tuned and measured against round 1c's rules |
| `look.py` | each database's pool run and read; the blind packets; the look learned from one judge and every five counted by the others; the look fitted on all and packed |
| `gemini.py` | the packets cut by database and asked of a judge through an OpenAI-style endpoint — Gemini, DeepSeek, GLM — and the replies gathered |
| `jev.py` | each panel asked of Jev, a decision model, as two yes-or-no questions with a probability each |

The pools, the judges' answers and the models live in
`lemmesee/corpus/round-1d/`, which git ignores; the answers and measures are
copied beside the report.
