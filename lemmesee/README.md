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
| [round-1/](round-1/README.md) | round 1 before its code: what it reads, what it proposes, on which databases, how it is judged |
| [round-1b/](round-1b/README.md) | round 1b: guessing foreign keys and closed lists from what public schemas declare |
| [reports/](reports/) | the dated rounds; `README.md` indexes them |
| [opus-brief.md](opus-brief.md) | Russian handoff for specifying the first rules and judging pilot, with the follow-up's sources |

**Status, 27 October 2026**: round 0's abstract search has a
[follow-up](reports/research-start-2026-10-27.md) from method sections,
implementation code and database documentation. It narrows the novelty
claims and proposes the first rules and judging pilot; a
[wider reading](reports/wider-reading-2026-10-27.md) adds the older and
neighbouring work both missed. Round 1 is [specified](round-1/README.md),
its snapshot and rules built; round 1b, [run](reports/declarations-2026-10-27.md),
found a small model guesses foreign keys better than the rules, and its
[second pass](reports/keys-2026-10-28.md) took them to F1 0.77 against 0.44.
Nothing is judged yet.
