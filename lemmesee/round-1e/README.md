# Round 1e: the look taught at scale, and the five chosen by what is kept

**Status: built and run on 28 October 2026 by the agent, at Dmitry's
request; results in [the report](../reports/look-scale-2026-10-28.md).** Round 1c's [question and ranker](../round-1c/README.md)
unchanged; round 1d's [look and penalties](../round-1d/README.md) taught
again, from more databases and cheaper judges, and set against what judges
keep of the five instead of what teachers ask.

## Why

`gap.py` set round 1d's five beside its pool on round 1d's 28 databases,
with the look out of fold as round 1d measured it:

- **The pool is not what limits it.** Opus keeps 4.73 of five in the best
  five a pool holds, 4.73 too under round 1d's penalties; round 1d's five
  keeps 2.85. Sonnet and Gemini say the same.
- **The order is.** Within a database the look orders what Opus keeps at an
  AUC of 0.75, the ranker's score at 0.66, and the two added together at
  0.74, no better than the look alone.
- **Two of five repeat a measure.** 42 % of the five's panels count or sum
  what another panel in it already does, over another time column or
  breakdown — orders by the date ordered, required and shipped — and Opus
  keeps 0.47 of those against 0.64 of the rest. Round 1d's penalty for the
  same base and measure is 0: the teachers' fives rarely met the case.
- **What Opus calls wrong is in the result**: one label twice when names are
  not keys, codes shown as bare numbers, a stray year stretching an axis, a
  placeholder date as the commonest, ties at the twelfth, and a key that
  reaches one table by one of two — home matches counted as all.

## What changes

1. **Databases**: 23 more from CTU's repository, none a pilot fixture —
   Sakila, Employees, AdventureWorks, `financial` and Stack Exchange's are
   left out as `round-1/fixtures.md` keeps them — and none a set made for
   learning with no business behind it. **Ten held out**, drawn with seed
   20261030 before any was loaded: Accidents, CDESchools,
   ConsumerExpenditures, FNHK, Hepatitis_std, legalActs, nhl_draft, SAP,
   Seznam, Walmart_2014. Thirteen are taught on: AustralianFootball,
   Countries, FlexiBee, geneea, GOSales, imdb_full, SalesDB, Telstra,
   Toxicology, tpcc, TubePricing, Walmart, YelpDataset3_disc_clean.
2. **Teachers**: Jev, DeepSeek and GLM answer every panel of every pool;
   the look learns their mean keep. **Sonnet and Gemini never teach**: they
   answer the ten held out and only count, and Opus counts on round 1d's 28
   from the answers it gave there.
3. **The look reads more of a result**: labels that repeat, labels that are
   bare numbers, the share of a time's span its middle rows cover, the share
   its commonest bucket holds, ties at the cut; and of a question, whether
   its breakdown is reached by one of several keys to one table.
4. **The choice is tuned on what is kept**: the look's weight against the
   ranker's score and the penalties for what two panels share, by
   coordinate ascent on how many of the five the teachers keep, over
   databases left out.

## How it is measured

- **On round 1d's 28**, each database left out in turn: kept of five by
  Opus, Sonnet and Gemini, against round 1, round 1c and round 1d — round
  1d's look taught by Sonnet counted by Opus and Gemini only.
- **On the ten held out**, once, fitted on the other 41: kept of five by
  Sonnet and Gemini against the same, each difference bootstrapped over
  databases.
- The ceiling as `gap.py` reads it, again, to see how much of the gap is
  left.

## Files

| Path | What |
|---|---|
| `gap.py` | where round 1d's five loses what its pool holds |
| `look.py` | the new databases' pools and blind packets, by round 1d's code; the result's new numbers and the sibling key; the look taught by the teachers' mean keep, the penalties and weight tuned on the five, and every five counted |

The teachers and counters are asked by round 1d's `gemini.py` and `jev.py`,
and Sonnet as round 1d's agents were, a packet each.

The pools, answers and models live in `lemmesee/corpus/round-1e/`, which git
ignores; the answers and measures are copied beside the report.
