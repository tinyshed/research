# Round 1y: the labels corrected and not only measured

Written at 15:03 UTC on 26 November 2026, before any fit, any scoring and
any reading of this round.

The key model's paper explains part of its distance from the language models
by incomplete labels: a training schema of ten tables or more declares about
two thirds of its keys ([keys-size-2026-11-10.md](../reports/keys-size-2026-11-10.md)
§2), so the labels teach the model to hold back in a large schema, and the
measure counts a real key nobody declared as false. Round 1v measured the
incompleteness and taught four variants the keys two language models agree
on; it applied none of the corrections that learning from positive and
unlabelled data offers. This round applies them. They need no language
model.

**The question.** Does a standard positive-unlabelled correction move
`eleven--wide`, the paper's model, the way the explanation predicts: more
keys predicted in large schemas, at a precision that holds once real keys
nobody declared are counted?

Nothing here asks a teacher anything. No model of this round is taught a
language model's key.

## The setting, in the survey's words

Bekker and Davis (2020, arXiv 1811.04820), read for this round with Elkan
and Noto (2008): a declared key is a labelled positive (`s = 1`); every
other candidate pair is unlabelled, a mix of pairs that are no key and keys
nobody declared. The **propensity** `e(x) = Pr(s = 1 | y = 1, x)` is the
chance that a real key is declared (§2.2); under SCAR it is one constant,
the **label frequency** `c` (§3.1.1), and `Pr(s = 1 | x) = c Pr(y = 1 | x)`.
The corrections used here are the survey's §5.3 and §4.1:

- *post-processing* (§5.3.1, Elkan and Noto): a classifier fitted with the
  unlabelled as negatives predicts `Pr(s = 1 | x)`; dividing it by `c`, or
  multiplying the threshold by `c`, gives the classifier of `y`;
- *the unlabelled weighted* (§5.3.2, Elkan and Noto): an unlabelled example
  is used twice, as a positive with weight
  `w(x) = Pr(y = 1 | s = 0, x) = (1 − c)/c · Pr(s = 1 | x) / (1 − Pr(s = 1 | x))`
  and as a negative with weight `1 − w(x)`; a labelled one is a positive
  with weight 1;
- *evaluation* (§4.1): recall can be estimated from the labelled positives,
  precision cannot; with a known propensity a labelled positive stands for
  `1/e` positives (the survey's §5.3.2 on SAR; Bekker, Robberechts and Davis
  2019), which gives the weighted counts below.

## The label frequency is taken, not tuned

It is a function of the number of tables `n` of a schema, from round 1v's
two measures on SchemaPile's training split, and nothing in this round
changes it:

| tables | `e(n)`, the name rules: `table_id` columns declared | `e(n)`, the teachers' estimate |
|---|---|---|
| under 5 | 0.953 | (the name rules' 0.953) |
| 5 to 9 | 0.867 | (the name rules' 0.867) |
| 10 to 19 | 0.793 | 0.69 |
| 20 to 39 | 0.736 | 0.62 |
| 40 or more | 0.659 | 0.71 |

**Which is used where.** The name rules' column is this round's label
frequency: for the weights of every fit, for the post-hoc thresholds and for
the weighted measures. It covers every size, falls with size as the
explanation says, and no language model made it, so the whole correction
stays free of one. The teachers' column is used once, for one post-hoc
variant read beside the others (`@size-teachers`), to say how much the
choice of measure moves the result; under ten tables it has no figure and
takes the name rules'. Training schemas have at most 100 tables; a schema of
more takes the last row.

SCAR is assumed within a size, and it is known not to hold exactly: round
1v's table shows that seven in ten training schemas of 40 tables or more
declare every `table_id` column, so the missing third is concentrated in the
other three in ten. The report says what that does.

## What is fitted and read

The recipe of `eleven--wide` is kept: version 11's cues, the drawn styles,
no hard negatives, the wide second stage with bounded leaves, 829 trees of
63 leaves and 400 of 31, seed 20261027; every fit is made again with seed
20261028 for the sample of negatives and both stages' trees, as round 1v's
`eleven-seed2` (round 1v found that a seed moves a fit by 0.005 to 0.010).

**The non-traditional classifier** is the frozen model itself: for seed
20261027 `eleven--wide`'s score of every training pair out of fold
(`corpus/round-1v/second/eleven--wide/train-oof.npy`), for the other seed
`eleven-seed2--wide`'s. Its score is fitted on a sample that keeps a tenth
of the negatives, so it is first made a probability over all pairs:
`g(x) = Pr(s = 1 | x)` is the isotonic regression of the declared label on
that score over every training pair.

**The weights** ([pu.py](pu.py) `weights`), a training pair:

- a pair of a column that has a declared key is what its label says, weight
  1: a column references one table, so its other pairs are known negatives;
- a pair of a column with no declared key is unlabelled:
  `w = min(1, (1 − e(n))/e(n) · g/(1 − g))`, and where a column's weights
  sum to more than 1 they are scaled to sum to 1 (one key a column);
- the training sample is the recipe's own, the same rows by the same seed,
  each sampled unlabelled pair a negative with weight `1 − w`; beside it
  every unlabelled pair of the training split with `w ≥ 0.01` enters once
  more as a positive with weight `w`. (The recipe keeps every positive and a
  tenth of the negatives; so does this.)

Before any fit the report's first table is read from the weights alone:
`Σ w` beside `(1 − e)/e` times the declared keys, by size. Under SCAR with a
calibrated `g` the two agree; where they do not, the classifier does not
find the undeclared keys where the label frequency says they are.

**(a) Fits with the unlabelled weighted**, decided here: all three
placements, because the second stage is what decides to draw a key and the
first is what round 1v found the gain in.

| name | first stage | second stage |
|---|---|---|
| `eleven--wide-pu` | `eleven` as frozen | weighted |
| `eleven-pu--wide` | weighted | on the declarations |
| `eleven-pu--wide-pu` | weighted | weighted |

Each has a `-seed2` twin. A second stage on a weighted first stage reads
that stage's scores out of fold, as every second stage here does.

**(b) Post-hoc, no refit**, on `eleven--wide` and `eleven-seed2--wide` as
round 1v froze them. `t` is the model's frozen threshold, `g` the isotonic
map above, `τ` a threshold on `g/e(n)`:

| name | the rule that draws a key |
|---|---|
| `@size` | `g(score)/e(n) ≥ g(t)`: the frozen threshold taken as right for a schema that declares everything; no free parameter |
| `@size-teachers` | the same with the teachers' `e(n)` |
| `@size-ipw` | `g(score)/e(n) ≥ τ`, `τ` the best weighted F1 on the validation split |
| `@ipw` | `score ≥ t′`, one threshold for every size, `t′` the best weighted F1 on the validation split: the evaluation corrected and nothing else |

**A fitted variant's threshold** is read on the validation split, with its
renamed copies beside it as the recipe reads it, twice: `@declared`, the
recipe's best F1 over declared keys, and `@ipw`, the best weighted F1. Both
enter the rule.

**The control, which is never the choice**: `eleven--wide@matched`, the
frozen model at the one threshold at which it draws as many keys on a set
as the variant it is compared with. A correction that does no more than a
lower threshold reads the same as its control.

## The measures

Round 1v's, unchanged and always reported: precision, recall and F1 over
every declared one-column key, a key right when its column and its table
are; 95 % percentile intervals of 1 000 resamples of databases (seed
20261101), a difference resampling both arms together; by the number of
tables in the bins under 5, 5 to 9, 10 to 19, 20 to 39, 40 to 99 and 100 or
more, and 40 or more together.

**F1 over declared keys cannot reward a key nobody declared.** Beside it:

1. **Weighted counts** ([pu.py](pu.py) `ipw`), no judge needed: in a
   database of `n` tables a declared key found stands for `1/e(n)` keys
   found, at most as many as the model drew there, and a declared key missed
   for `1/e(n)` missed; precision is the weighted found over the keys drawn,
   recall the weighted found over weighted found and missed. It uses
   SchemaPile's label frequency on whatever set it is read; whether that
   frequency holds on GitHub's schemas is checked against the judge.
2. **Precision with the blind judge**, as round 1v: Claude Sonnet 5.5 at
   medium effort as Claude Code agents, one packet each, round 1t's prompt
   unchanged ([judge-prompt.md](../round-1t/judge-prompt.md)), which taught
   nothing here; the keys the compared models draw that no declaration
   names, shuffled among 60 declared keys and 60 pairs drawn at random, each
   schema shown as the tables its pairs name with no declared key; the judge
   is told nothing of which model drew a pair. One change, so that large
   schemas are judged often enough: the sample is drawn by stratum (schemas
   under 40 tables, and of 40 or more) and each answer is weighted by its
   stratum's size over its sample.
3. **Recall over declared and judged-real keys**, which the paper lacks.
   The pool of a set is every undeclared pair that a compared model draws
   or that a language model of round 1w named on that set (six models on
   plain A, seven on style 3, their saved replies, nothing asked again). A
   sample of the pool that no compared model draws is judged in the same
   packets, by stratum (size as above, and named by one language model or
   by several). The real keys of the set are estimated as its declared keys
   plus the judged-real share of the pool, stratum by stratum; a model's
   recall is its declared keys found plus the estimated real ones among its
   other keys, over that. Its interval is the percentile interval of 10 000
   resamples of the judged pairs within strata (seed 20261101): it is the
   sampling error of the judged sample and holds the databases fixed. A key
   nobody in the pool names is not counted, so this recall is over the
   pooled keys and is an upper bound on recall over every real key. The
   judge is one of the pool's language models, which the report says beside
   the figure. `unsure` counts as not real.

If the judge cannot be asked (no agent of that model, or its quota spent),
the report says so and gives what was measured; no other judge takes its
place.

## Which data is what

- **Fitted on**: SchemaPile's training split, as the recipe.
- **Thresholds**: its validation split.
- **The rule reads**: the four GitHub development sets of round 1v pooled
  (452 schemas, 10 362 keys), from round 1v's kept pairs.
- **Development readings**: plain A and style 3. `eleven--wide` read each
  once on 10 November and they are development data since. Every variant is
  frozen by hash and the rule has chosen before either is scored by anything
  of this round; then every frozen variant is read on both, by size as the
  paper's Table 5. The first thing read there is `eleven--wide` itself from
  rebuilt pairs: it must draw the 15 711 and 1 458 keys of round 1v's
  reading, or nothing else is read.
- **Plain B is frozen and unread and stays so**: no schema of it is opened
  or parsed by anything here.
- The judge reads plain A and style 3 only.

## The rule of choice

Written before any scoring. Over the candidates, each `(a)` variant at
`@declared` and at `@ipw` and each `(b)` variant, six and four:

> the candidate with the highest **weighted F1 over the four GitHub
> development sets pooled, the mean of its two seeds**, is the round's one
> result, and the fit reported is the one of seed 20261027.

If that mean is not above the mean of `eleven--wide` and
`eleven-seed2--wide` at their frozen thresholds, the round's result is that
no correction is chosen over the frozen model; the highest candidate is
still the one read with the judge, so that the diagnosis is made. The
choice, its twin, `eleven--wide`, and the best candidate of the other family
(fitted if the choice is post-hoc, post-hoc if it is fitted) are the
compared models of the judge's sample, with the frozen model's matched
control for the choice on each set.

## What counts as an answer

On plain A, the larger set, with style 3 beside it:

- **the explanation is supported** if in schemas of 40 tables or more the
  choice draws more keys than `eleven--wide`, its recall over declared and
  judged-real keys is above `eleven--wide`'s with the difference's interval
  over zero, its precision with the judge is not below `eleven--wide`'s by
  more than the interval of the difference, and it is above its matched
  control in the same recall or the same precision with the interval over
  zero, in both seeds' direction;
- **not separable from a threshold change** if the first three hold and the
  choice stands within the interval of its matched control;
- **not supported** if the choice draws no more in large schemas, or what
  it adds there the judge calls real less often than what the frozen model
  draws, so that precision with the judge falls by more than the interval.

Whatever is read is reported, with the weights' table, the weighted
measures beside the judge's, and what a seed moves.

## Machine

One pair build at a time and one graphics-card fit at a time, each under
the session's lock file (`pairs.lock`, `gpu.lock`) with this round's name in
it, held only while the build or the fit runs; another round shares the
card. Round 1v's kept pairs of the development sets are read, not built
again; the training split's pairs are built once a first-stage fit, as the
recipe has no cache of them, and plain A's and style 3's once.

## Run

```sh
P=round-1b/.venv/Scripts/python
$P -I round-1y/pu.py weights                       # the weights' table, before any fit
$P -I round-1y/fitting.py first eleven-pu          # and eleven-pu-seed2: the card
$P -I round-1y/fitting.py score-first eleven-pu
$P -I round-1y/fitting.py second eleven wide-pu    # <first> <wide|wide-pu>; every pairing above
$P -I round-1y/fitting.py score-second eleven wide-pu
$P -I round-1y/variants.py thresholds              # every variant's thresholds, from the validation split
$P -I round-1y/variants.py choose                  # the rule
$P -I round-1y/variants.py freeze                  # by hash, before plain A or style 3 is scored
$P -I round-1y/reading.py plain-a                  # and style3
$P -I round-1y/judging.py pack plain-a             # then one agent a packet; then count
$P -I round-1y/tables.py rule                      # the report's tables: rule | set <set> | judge <set>
$P -I round-1y/gather.py reports/data/keys-labels-2026-11-26
```

## Added while the round ran

- **The weights' table** (15:04 UTC, [pu.py](pu.py) `weights`, before any
  fit). The frozen model's score is calibrated by the isotonic map to the
  third digit in every band of score. The weights sum to 142, 1 127, 2 536,
  2 466 and 3 733 undeclared keys by size, where the name rules' label
  frequency implies 250, 1 682, 3 710, 3 653 and 13 817: two thirds of what
  it implies up to 39 tables and **27 % at 40 tables or more** (29 % with
  the other seed). Under SCAR the two would agree. So either the keys a
  large schema leaves out are not like the ones it declares, or the model
  has learned to score them low, which a weight computed from its own score
  cannot undo. Nothing is changed for it: the fits are made as specified,
  and the report says which of the two the readings point to.
- **A module renamed** (15:07 UTC): `fit.py` became `fitting.py`, because
  round 1t's `fit.py` is on the same path; the first fit, started at 15:06
  from `fit.py`, is the same code.
- **Out of memory, once** (15:08 UTC): the first fit, started at 15:06, was
  lost while it built the training split's pairs beside the weights' table
  and two other rounds' jobs; it had fitted and written nothing. The pairs
  are now built once, by six processes in the recipe's order of rows,
  checked row by row against what the frozen fit kept, and kept
  (`fitting.py pairs`, `corpus/round-1y/pairs/`): they are the same for
  either seed and any weights, so every first stage here reads them.
- **A fit weighted by a schema's own label frequency, read beside the
  rule** (15:08 UTC, before any fit, from the weights' table alone). Round
  1v's first measure can be read a schema at a time: `e(s)` is the share of
  the schema's own `table_id` columns that it declares, counted beside five
  more at its size's `e(n)`, so that a schema with none takes `e(n)`
  ([pu.py](pu.py) `schema_e`; the five is set here and not tuned). This is
  the survey's SAR case with a known propensity. Its weights sum to 179,
  1 371, 3 033, 3 235 and 6 423 where its own frequencies imply 243, 1 566,
  3 379, 3 338 and 9 971: nine tenths up to 39 tables and 64 % at 40 or
  more, against the size's 27 %. `eleven-sar--wide-sar`, both stages
  weighted so, with its twin, is read at `@declared` and `@ipw` beside the
  candidates. **It is not a candidate of the rule**: the round's label
  frequency is the size's, and a schema's own cannot be used after the fact,
  since a schema `see` reads declares nothing. It is here to tell the two
  readings of the weights' table apart: if it moves a large schema where the
  size's weights do not, the declarations are missing by schema and not at
  random within a size. The one of its two thresholds with the higher
  weighted F1 on the GitHub development sets joins the judge's compared
  models.
- **A second control**, written with the same note: `@matched-by-size`, the
  frozen model at the five thresholds at which it draws as many keys as the
  compared variant in each size of schema. For a post-hoc variant it is the
  variant itself; for a fit it says whether the fit does more than five
  thresholds would.
- **The card and the memory were shared with two other rounds** (15:10 to
  16:33 UTC). The first fit waited 188 s for `gpu.lock`, twice passed over
  by another round's next job while this round's lock looked every 5 s and
  then every 0.5 s; it now looks every 20 ms and reads its pairs only once
  the card is its own. Two second-stage jobs were lost to memory
  (`eleven-seed2--wide-pu` at 15:18 while fitting, `eleven-sar-seed2--wide-sar`
  at 16:23 while scoring the development sets) and run again; a scoring now
  keeps what it finished and goes on from there. A repeated fit of
  `eleven-sar--wide-sar`, started by a lane that had not been told the fit
  was done, was stopped at 16:32 before it wrote anything; it had taken
  `pairs.lock` with it, which was removed by hand.
- **The fits** (15:13 to 16:31 UTC): four first stages on the card,
  `eleven-pu` 15:29–15:37, `eleven-pu-seed2` 15:38–15:49, `eleven-sar`
  15:49–15:58, `eleven-sar-seed2` 15:58–16:10, and eight second stages. A
  weighted first stage takes 63 990 unlabelled pairs once more as positives,
  weighing 7 741 keys together, beside its 920 601 sampled pairs (59 117 and
  7 765 with the other seed); by a schema's own frequency, 106 422 pairs
  weighing 10 861.
- **The rule's choice** (16:34:04 UTC, [variants.py](variants.py) `choose`,
  every candidate scored on the development sets in one run): the highest
  weighted F1 over the four GitHub development sets pooled, the mean of two
  seeds, is **`eleven-pu--wide-pu@ipw`**, both stages weighted, at the
  threshold the weighted F1 of the validation split chooses: 0.8352 and
  0.8246, mean 0.8299, against the frozen model's 0.8115 and 0.7937, mean
  0.8026. Behind it `eleven--wide-pu@ipw` 0.8280, `eleven-pu--wide-pu@declared`
  0.8244, `eleven--wide@size-teachers` 0.8236, `@size` and `@size-ipw` 0.8226,
  `eleven-pu--wide@ipw` 0.8222, `eleven-pu--wide@declared` 0.8151,
  `eleven--wide@ipw` 0.8148, `eleven--wide-pu@declared` 0.8119. Every
  candidate is above the frozen model, and the first seven stand within
  0.008 of each other where the two seeds of one fit differ by 0.011 to
  0.018: as in round 1v, the rule picks one of several that development
  data cannot tell apart. Over declared keys the choice reads precision
  0.786, recall 0.815, F1 0.801 against the frozen model's 0.823, 0.772,
  0.797. The best candidate of the other family is `eleven--wide@size-teachers`;
  of the fit by a schema's own frequency, `@ipw` (0.8141, under every
  by-size candidate but three).
- **Frozen** at 16:34:09 UTC ([variants.py](variants.py) `freeze`,
  `corpus/round-1y/frozen.json`): every variant's three files, the
  thresholds, the choice and the two calibrations by hash, before any pair
  of plain A or style 3 was built or scored by this round.
- **Style 3 read** (16:34–16:35 UTC, [reading.py](reading.py)): the frozen
  model and its twin from rebuilt pairs give round 1v's count of every
  database (1 458 and 1 399 keys); then every frozen variant. Its judge's
  sample was packed at 16:36 (480 pairs, seven packets) and answered by
  seven agents at once at 16:36–16:37.
- **Plain A's reading was lost to memory once** (16:35–16:52 UTC), at the
  first second stage's features, having verified and written nothing. The
  reading now keeps the pairs, each first stage's scores and each second
  stage's as it makes them, and builds with four processes; it was started
  again at 16:53.
- **The twin's own control joins plain A's compared models** (written at
  16:59 UTC, before plain A was read or packed): `eleven-pu-seed2--wide-pu@ipw`
  is held against `eleven-seed2--wide` at the threshold that draws as many
  keys, so that "in both seeds' direction" can be read against a threshold
  change. Style 3 had been packed and judged without it: there the twin is
  compared with the frozen model only.
- **Plain A read** (16:53–17:19 UTC): the frozen model and its twin from
  rebuilt pairs give round 1v's count of every database (15 711 and 15 614
  keys); then every frozen variant and control. Its judge's sample was
  packed at 17:19 (860 pairs, thirteen packets) and answered by thirteen
  agents, seven and then six at once, by 17:21. The judge is the Agent
  tool's `sonnet` at medium effort, one agent a packet, round 1t's prompt
  with the two file paths after it; each agent read its packet and wrote its
  answers and nothing else. No agent was asked twice and none failed.
- **What the readings say** (17:21 UTC; the report has every figure). On
  plain A the choice draws 17 842 keys where the frozen model draws 15 711,
  26 % more in schemas of 40 tables or more; over declared keys 0.762
  against 0.757, +0.005 [−0.006, +0.016]; with the judge its precision is
  0.928 against 0.956, −0.028 [−0.039, −0.016], and its recall over declared
  and judged-real keys 0.727 against 0.660, +0.068 [+0.060, +0.076]; against
  the frozen model at the threshold that draws as many keys, +0.013 [+0.004,
  +0.022] and +0.010 [+0.003, +0.018], the twin against its own control
  +0.017 and +0.013. By the test above, "supported" fails on its third
  part, the precision, and passes the other three; the report gives the
  parts one by one and says that the test's three verdicts were written too
  coarsely for a result of this shape.
- **An agent asked for its identifier** (17:27 UTC): one more agent started
  as the judges were, given no packet and asked only which model it is,
  answered Sonnet 5.5, `claude-sonnet-5-5`.
- **Not run**: no language model was asked for a key or a label; the twin's
  own control was not in style 3's judged sample; no reader in Rust was
  built; plain B was not opened.
