# Round 1g: what limits the look

**Status: specified and run on 28 October 2026 by the agent, at Dmitry's
request, after a review of round 1f by GPT; results in [the
report](../reports/limits-2026-10-28.md).** Round 1f found the learning
curve flat past forty databases, the tuned penalties swinging with two
databases' labels, and its counts on twelve databases too few to tell
configurations apart. The review added that round 1f did not show the
look's size to be the limit, that a count of kept panels cannot see two
panels repeating each other, and that its tuning on round 1d's 28 used
scores from looks that had seen the database counted. This round asks
which of capacity, context, objective and words limits the look, on every
database at once and with nothing tuned.

## What is fixed

- **The data**: round 1d's, 1e's and 1f's 93 databases, their pools and
  the three teachers' answers (Jev, DeepSeek, GLM) as they are. Two of
  talkingdata's panels failed on a compiler fault fixed now — a flag kept as
  an integer is cast before PostgreSQL tests it — and stay failed in the
  pools.
- **The five**: the look's order, a question never twice, no penalties —
  nothing tuned, so nothing leaks from a database into the choice of its
  own five.
- **The folds**: 90 families — databases whose tables overlap by 40 % or
  more are one family: Basketball men's and women's, The Simpsons' and Law
  and Order's episodes, the two football leagues — five folds by family,
  three draws of the folds.

## What is compared

A first pass crosses capacity, context and objective:

| | round 1e's look: 150 trees of four leaves | a larger look: 31 leaves, up to 2 000 trees, stopped on families held out of its training |
|---|---|---|
| **round 1e's numbers** | the baseline | capacity |
| **and the alternatives' context** | context | both |

each taught two ways: the teachers' mean keep per panel, as before, and
LambdaRank over each database's panels, the mean keep in four grades.
**The alternatives' context** is what the look could know of the other
candidates before choosing: how many share the panel's base and measure,
how many of those score higher, how many share its time or breakdown, the
largest correlation of its series with a higher one's, the largest
overlap of its labels with a higher one's, whether a higher one returned
the same rows, and how much of the pool its base table holds.

A second pass follows where the first pass's errors pointed, the words:
each panel's words by where they stand — base table, measure, time,
breakdown — as round 1c's teachers asked questions holding them, and as
this look's teachers kept panels holding them, counted on the folds taught
only and, for a database taught, without its own panels.

## How it is measured

- On every database, out of fold: AUC within a database against the
  teachers' mean, on the folds taught and on the fold left out, to see
  capacity learning or memorising.
- **On the 50 databases with judges that never teach** — round 1d's 28
  (Opus on 26, Sonnet, Gemini), round 1e's ten and round 1f's twelve
  (Sonnet, Gemini): AUC and NDCG@5 within a database against each judge's
  keep, and kept of five; each difference from the baseline bootstrapped
  over databases, the three draws averaged first.
- **Ceilings by source**: the five that each source of labels — the
  teachers' mean, each teacher, each counting judge — would choose by
  itself, counted by the others: the most a look taught by that source
  could reach.
- **The five as a set**: Sonnet and Gemini shown each of the 50 databases'
  fives side by side — round 1's, the baseline look's out of fold and the
  one the teachers' mean would choose, unnamed, shuffled, panels numbered
  after shuffling — say for each how many of its panels are worth a place
  together and which repeat another, and which five they would keep. A
  second set adds the look and the teachers' mean under a rule set before
  any five was judged: one base and measure once, one breakdown once. A
  third, under a rule read from Sonnet's repeats in the second — each time
  or breakdown column once, a push away from a second panel of one shape —
  is counted by Gemini alone.
- **A lexicon**, a third pass of the look: the 4 000 commonest words of
  SchemaPile-Perm's names, nine numbers each written by DeepSeek, a teacher
  already.

A page of disputed pairs for Dmitry was planned and left out at his word:
the round runs without him.

## Files

| Path | What |
|---|---|
| `diagnose.py` | the families, folds, the alternatives' context and the words; every configuration fitted and counted; the ceilings by source; the look's largest errors against its teachers |
| `sets.py` | the fives packed side by side for judges, under no rule, one base and measure once, or each axis once; asked of an endpoint, gathered and counted |
| `lexicon.py` | the lexicon's words from SchemaPile-Perm, asked of a language model, gathered and sized |

Outputs live in `lemmesee/corpus/round-1g/`, which git ignores; the
measures and answers are copied beside the report.
