# lemmesee

How much of a database's sense can be guessed from its schema and its own
statistics, by something small enough to carry in a binary, without a
language model, without disturbing the database — and when it is worth
looking closer.

It grew out of Dashbin's `dashbin see`, which draws any store before anyone
has made a dashboard: panels it **knows** from a store that says what its
data means, and panels it **guesses** from a table's columns. lemmesee is the
research behind the guesses. It is not a product, and Dashbin takes from it
only what passes the lines Dashbin's plan sets
([chapter 14](../dashbin/plan/14-first-version.md)).

| | |
|---|---|
| [plan.md](plan.md) | the questions, the configurations compared, the stores and corpora, how it is measured, the rounds in order |
| [reports/](reports/) | the dated rounds; `README.md` indexes them |
| [opus-brief.md](opus-brief.md) | Russian handoff for specifying the first rules and judging pilot, with the follow-up's sources |

**Status, 27 October 2026**: round 0's abstract search has a
[follow-up](reports/research-start-2026-10-27.md) from method sections,
implementation code and database documentation. It narrows the novelty
claims and proposes the first rules and judging pilot; a
[wider reading](reports/wider-reading-2026-10-27.md) adds the older and
neighbouring work both missed. Nothing is measured yet.
