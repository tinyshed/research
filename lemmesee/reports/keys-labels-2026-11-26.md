# The key model's labels corrected, 26 November 2026

Round 1y, as [its specification](../round-1y/README.md) set it at 15:03 UTC
on 26 November, before any fit, and wrote down step by step while it ran.
The key model's paper explains part of its distance from the language
models by incomplete labels: a training schema of ten tables or more
declares about two thirds of its keys
([keys-size-2026-11-10.md](keys-size-2026-11-10.md) §2), so the labels
teach the model to hold back in a large schema. Round 1v measured that and
taught variants the keys two language models agree on. This round applies
what the paper says it did not: the standard corrections of learning from
positive and unlabelled data (Bekker and Davis 2020; Elkan and Noto 2008),
which need no language model. It asks whether they move `eleven--wide`, the
paper's model, the way the explanation predicts. Raw output in
[data/keys-labels-2026-11-26/](data/keys-labels-2026-11-26/).

Every F1 is over every declared one-column key, a key right when its column
and its table are; every interval on it a 95 % percentile interval of 1 000
resamples of databases (seed 20261101), a difference resampling both arms
together. **Every figure here is a development reading.** Plain A and
style 3 were read once by `eleven--wide` on 10 November and are development
data since; nothing here opened a schema of plain B. A precision or a recall
counted with the blind judge's answers stands beside the declared one, never
in its place; its interval is the sampling error of the judged pairs and
holds the databases fixed.

| Question | Answer | What follows |
|---|---|---|
| **Does a positive-unlabelled correction make the model draw more keys in large schemas?** | Yes. The rule's choice, both stages fitted with the unlabelled pairs weighted by the label frequency, draws 17 842 keys on plain A where `eleven--wide` draws 15 711: 1 to 3 % more under 20 tables, 11 % more at 20 to 39, **26 % more at 40 or more** (7 485 against 5 950). The other seed: 24 % more at 40 or more | the direction the explanation predicts, with no language model anywhere |
| **Does F1 over declared keys move?** | No. Plain A 0.762 against 0.757, **+0.005 [−0.006, +0.016]**; the other seed +0.007 [−0.001, +0.017]; style 3 +0.010 [−0.025, +0.050]. Recall of declared keys 0.795 against 0.741, precision 0.731 against 0.774 | the measure the paper leads with cannot see the change; the distance to the language models over declared keys stays |
| **Does precision hold once real keys nobody declared are counted?** | **Not quite.** With the blind judge, plain A: 0.928 [0.912, 0.944] against 0.956 [0.938, 0.974], **−0.028 [−0.039, −0.016]**; in schemas of 40 tables or more 0.915 against 0.957, −0.042 [−0.067, −0.015]. Of the 2 131 keys the choice adds, 884 are declared, about 660 the judge calls real and about 590 not: 72 % real, where 96 % of what the frozen model draws is | the strong form of the explanation, more keys at the same precision, is not what was measured |
| **And recall over declared and judged-real keys?** | Up. Plain A 0.727 [0.719, 0.736] against 0.660 [0.649, 0.670], **+0.068 [+0.060, +0.076]**; at 40 tables or more 0.662 against 0.550, +0.112 [+0.097, +0.128]. F1 from the judge's precision and this recall: 0.816 against 0.781 | a trade: seven points of recall for three of precision; eleven for four in large schemas |
| **Is it more than a lower threshold?** | A little, and it repeats. The frozen model at the one threshold that draws as many keys (0.674 in place of 0.798) gets most of it: recall 0.717, precision 0.915. The choice minus that control: precision **+0.013 [+0.004, +0.022]**, recall **+0.010 [+0.003, +0.018]**; the other seed against its own control +0.017 and +0.013, both intervals over zero. At 40 tables or more: recall +0.046 [+0.033, +0.059] at the same precision. Over declared keys the two cannot be told apart, +0.004 [−0.005, +0.014] | five sixths of the gain in recall is a threshold's; what the labels' correction adds is real, small, and lies in large schemas |
| **Does a threshold by size, with no refit, do it?** | No better than one lower threshold. `@size-teachers` minus the matched control: precision +0.003, recall −0.003 | a lower threshold already draws its extra keys in large schemas; the label frequency adds nothing after the fact |
| **Were keys left undeclared at random within a size?** | No. Before any fit, the weights summed to two thirds of the undeclared keys the label frequency implies up to 39 tables and to 27 % at 40 or more. Weighted by a schema's own frequency they reach 64 %; that fit adds fewer keys on plain A and cleaner ones (79 % real at 40 tables or more against 75 %), and reads level with the choice | the survey's SCAR does not hold here; the correction undercorrects exactly where the explanation needs it |
| **How incomplete are plain A's own declarations?** | Its declared keys are 0.72 of the declared and judged-real keys of the pool, and **0.63 [0.61, 0.65] in schemas of 40 tables or more** | a third measure, on other data, beside round 1v's 0.66 and 0.71 |
| **What does a seed move?** | The choice and its twin: 0.762 and 0.759 over declared keys, 0.928 and 0.925 with the judge, 0.727 and 0.714 in recall over real keys; 0.662 and 0.628 at 40 tables or more | the direction holds in both; the size of the gain in large schemas moves by a third |
| **Is it worth plain B?** | No, by this report's reading; §8 says why and names the frozen variant that would be read | plain B stays unread |

## 1. The correction, and the label frequency it takes

A declared key is a labelled positive. Every other candidate pair is
unlabelled: a pair that is no key, or a key nobody declared. The chance that
a real key is declared is the **label frequency**. It is taken, not tuned,
from round 1v's two measures on SchemaPile's training split, as a function
of the number of tables `n`:

| tables | `e(n)`, the name rules: `table_id` columns declared | `e(n)`, the teachers' estimate |
|---|---|---|
| under 5 | 0.953 | (the name rules') |
| 5 to 9 | 0.867 | (the name rules') |
| 10 to 19 | 0.793 | 0.69 |
| 20 to 39 | 0.736 | 0.62 |
| 40 or more | 0.659 | 0.71 |

The name rules' column is used everywhere: no language model made it. The
teachers' is used for one post-hoc variant read beside the others.

**The non-traditional classifier** is the frozen model: its score of every
training pair out of fold, made a probability `g` over all 8 603 420
training pairs by an isotonic regression on the declared label (the model
is fitted on a sample that keeps a tenth of the negatives, so its raw score
is not one). `g` follows the declared share to the third digit in every
band of score; a pair the model scores 0.8 to 0.9 is a declared key 43 %
of the time.

**The weights** (Elkan and Noto; the survey's §5.3.2). A pair of a column
with a declared key is what its label says. A pair of any other column is
unlabelled and is used twice: as a positive with weight
`w = min(1, (1 − e)/e · g/(1 − g))`, a column's weights scaled to sum to at
most 1, and as a negative with weight `1 − w`. The recipe's sample is kept,
the same rows by the same seed; beside it every unlabelled pair with
`w ≥ 0.01` enters once more as a positive.

**Read before any fit**, from the weights alone
([weights.json](data/keys-labels-2026-11-26/weights.json)), seed 20261027;
seed 20261028 in brackets:

| tables | declared keys | undeclared keys `e(n)` implies | the weights' sum | of what is implied |
|---|---|---|---|---|
| under 5 | 5 070 | 250 | 142 (144) | 57 % |
| 5 to 9 | 10 965 | 1 682 | 1 127 (1 050) | 67 % |
| 10 to 19 | 14 214 | 3 710 | 2 536 (2 407) | 68 % |
| 20 to 39 | 10 185 | 3 653 | 2 466 (2 444) | 68 % |
| 40 or more | 26 702 | 13 817 | 3 733 (3 987) | **27 %** |

If keys went undeclared at random within a size, the two columns would
agree. Up to 39 tables the weights find two thirds of what the label
frequency implies; at 40 tables or more, a quarter. So the correction that
the survey calls standard gives a large schema the least of what it is owed:
it reads the model's own score, and the model already scores the undeclared
keys of a large schema low. §5 comes back to this.

## 2. What was fitted

The recipe of `eleven--wide` is kept: version 11's cues, the drawn styles,
no hard negatives, the wide second stage with bounded leaves, 829 trees of
63 leaves and 400 of 31, seed 20261027, and every fit again with seed
20261028 for the sample of negatives and both stages' trees
([fitting.py](../round-1y/fitting.py), [fits.json](data/keys-labels-2026-11-26/fits.json)).

**(a) Fitted with the unlabelled weighted**, all three placements:

| | first stage | second stage | positives added, their weight | threshold, declared F1 / weighted F1 | validation F1, declared, at each |
|---|---|---|---|---|---|
| `eleven--wide`, frozen | on the declarations | on the declarations | | 0.798 | 0.795 |
| `eleven--wide-pu` | as frozen | weighted | 63 990 pairs, 7 741 | 0.877 / 0.836 | 0.796 / 0.793 |
| `eleven-pu--wide` | weighted | on the declarations | 63 990, 7 741 | 0.766 / 0.735 | 0.780 / 0.779 |
| `eleven-pu--wide-pu` | weighted | weighted | 63 990, 7 741 | 0.842 / 0.824 | 0.779 / 0.778 |
| `eleven-sar--wide-sar`, beside | weighted by a schema's own frequency | the same | 106 422, 10 861 | 0.909 / 0.824 | 0.778 / 0.773 |

With the other seed the thresholds are 0.895 / 0.820, 0.779 / 0.735,
0.845 / 0.802 and 0.899 / 0.887, and 59 117 pairs enter weighing 7 765.

**(b) Post-hoc, no refit**, on the frozen model's scores: a threshold for
each size ([variants.json](data/keys-labels-2026-11-26/variants.json)).
`@size` draws a key when `g(score)/e(n)` reaches `g` of the frozen
threshold, the survey's §5.3.1 with no free parameter; `@size-teachers` is
the same with the teachers' column; `@size-ipw` chooses the threshold on
`g/e(n)` by the weighted F1 of the validation split; `@ipw` is one threshold
for every size chosen by that weighted F1, the evaluation corrected and
nothing else.

| | under 5 | 5 to 9 | 10 to 19 | 20 to 39 | 40 or more |
|---|---|---|---|---|---|
| `eleven--wide`, frozen | 0.798 | 0.798 | 0.798 | 0.798 | 0.798 |
| `@size` | 0.785 | 0.773 | 0.771 | 0.691 | 0.679 |
| `@size-teachers` | 0.785 | 0.773 | 0.679 | 0.663 | 0.679 |
| `@size-ipw` | 0.785 | 0.773 | 0.773 | 0.710 | 0.679 |
| `@ipw` | 0.748 | 0.748 | 0.748 | 0.748 | 0.748 |

The isotonic map is a staircase, which is why two sizes can share a
threshold.

**A fit by a schema's own label frequency** was added at 15:08 UTC, before
any fit, when the table of §1 was read: `e(s)` is the share of the schema's
own `table_id` columns that it declares, counted beside five more at its
size's `e(n)`. Its weights sum to 6 423 at 40 tables or more, 64 % of what
its own frequencies imply (9 971), against the size's 27 %. It is read
beside the candidates and is not one: the round's label frequency is the
size's, and a schema's own cannot be used after the fact, since a schema
`see` reads declares nothing.

**The controls**, never the choice: the frozen model at the one threshold
at which it draws as many keys on a set as the variant it is held against
(*matched*), and at the five thresholds at which it draws as many in each
size (*matched by size*). A correction that is only a lower threshold reads
as its control does.

**The weighted measure** (the survey's §4.1 and its propensity weighting):
in a database of `n` tables a declared key found stands for `1/e(n)` keys
found, at most as many as the model drew there, and one missed for `1/e(n)`
missed. It needs no judge, and it is what the rule reads.

## 3. The rule's choice

Written before any scoring: the candidate with the highest weighted F1 over
the four GitHub development sets pooled (452 schemas, 10 362 keys), the mean
of its two seeds ([variants.py](../round-1y/variants.py),
[choice.json](data/keys-labels-2026-11-26/choice.json),
[development.json](data/keys-labels-2026-11-26/development.json)). Seed
20261027's row is shown; development data.

| | P / R / F1 declared | minus the frozen model | keys drawn (in schemas of 40 tables or more) | weighted F1, each seed | mean | SchemaPile's complete test |
|---|---|---|---|---|---|---|
| **`eleven-pu--wide-pu@ipw`**, the rule's | 0.786 / 0.815 / 0.801 | +0.004 [−0.007, +0.015] | 10 748 (4 748) | 0.8352, 0.8246 | **0.8299** | 0.789 |
| `eleven--wide-pu@ipw` | 0.798 / 0.808 / 0.803 | +0.006 [−0.004, +0.018] | 10 496 (4 533) | 0.8322, 0.8238 | 0.8280 | 0.794 |
| `eleven-pu--wide-pu@declared` | 0.794 / 0.809 / 0.801 | +0.005 [−0.006, +0.015] | 10 562 (4 669) | 0.8317, 0.8170 | 0.8244 | 0.791 |
| `eleven--wide@size-teachers` | 0.784 / 0.812 / 0.798 | +0.001 [−0.006, +0.010] | 10 728 (4 445) | 0.8323, 0.8149 | 0.8236 | 0.789 |
| `eleven--wide@size` | 0.791 / 0.805 / 0.798 | +0.001 [−0.006, +0.009] | 10 540 (4 445) | 0.8298, 0.8153 | 0.8226 | 0.795 |
| `eleven--wide@size-ipw` | 0.793 / 0.804 / 0.799 | +0.002 [−0.005, +0.010] | 10 500 (4 445) | 0.8299, 0.8152 | 0.8226 | 0.795 |
| `eleven-pu--wide@ipw` | 0.790 / 0.807 / 0.799 | +0.002 [−0.007, +0.012] | 10 590 (4 534) | 0.8298, 0.8147 | 0.8222 | 0.788 |
| `eleven-pu--wide@declared` | 0.798 / 0.798 / 0.798 | +0.002 [−0.007, +0.011] | 10 362 (4 417) | 0.8248, 0.8054 | 0.8151 | 0.791 |
| `eleven--wide@ipw` | 0.805 / 0.792 / 0.798 | +0.002 [−0.002, +0.006] | 10 191 (4 169) | 0.8223, 0.8074 | 0.8148 | 0.796 |
| `eleven-sar--wide-sar@ipw`, beside | 0.795 / 0.796 / 0.795 | −0.001 [−0.011, +0.010] | 10 373 (4 394) | 0.8239, 0.8043 | 0.8141 | 0.780 |
| `eleven--wide-pu@declared` | 0.816 / 0.783 / 0.799 | +0.003 [−0.003, +0.009] | 9 939 (4 227) | 0.8179, 0.8058 | 0.8119 | 0.794 |
| `eleven--wide`, frozen | 0.823 / 0.772 / 0.797 | | 9 723 (3 934) | 0.8115, 0.7937 | 0.8026 | 0.798 |
| `eleven-sar--wide-sar@declared`, beside | 0.833 / 0.752 / 0.790 | −0.006 [−0.017, +0.005] | 9 363 (3 886) | 0.7976, 0.8015 | 0.7995 | 0.785 |

The rule picks **`eleven-pu--wide-pu@ipw`**: both stages weighted, at the
threshold the weighted F1 of the validation split chooses (0.824; 0.802 with
the other seed). Three things are plain before any judge is asked.

- **Every candidate is above the frozen model, and the first seven cannot
  be told apart.** They stand within 0.008 of each other; the two seeds of
  one fit differ by 0.011 to 0.018.
- **F1 over declared keys does not move**: +0.004 [−0.007, +0.015] for the
  choice, +0.010 [−0.000, +0.020] for its twin against its own frozen
  model. Each candidate trades precision against the declarations for
  recall, three to four points of each.
- **The keys added are in large schemas**, as the explanation predicts. The
  choice draws 10 748 keys where the frozen model draws 9 723. By size, 5 to
  9 tables: 1 184 against 1 167; 10 to 19: 2 208 against 2 118; 20 to 39:
  2 608 against 2 504; 40 to 99: 2 425 against 2 041; 100 or more: 2 323
  against 1 893. Recall of declared keys rises from 0.708 to 0.780 at 40 to
  99 tables and from 0.667 to 0.748 at 100 or more, and by 0.01 to 0.02
  under 40. The other seed: 0.693 to 0.765, and 0.614 to 0.699.

On SchemaPile's own test split the choice reads 0.749 against the frozen
model's 0.761, −0.012 [−0.019, −0.005] (the twin −0.013): it draws keys a
SchemaPile schema does not declare, and the measure counts them false. That
is what round 1v's guard was written against, and why this round's rule
does not read declared F1.

## 4. Plain A and style 3, development readings

Every variant was frozen by hash at 16:34:09 UTC, after the rule had chosen
and before a pair of either set was built
([frozen.json](data/keys-labels-2026-11-26/frozen.json)). The first thing
read on each set was the frozen model from rebuilt pairs: it gives round
1v's count of every database, 15 711 keys on plain A and 1 458 on style 3
(15 614 and 1 399 for its twin). Then every variant and its two controls
([reading.py](../round-1y/reading.py),
[heldout-plain-a.json](data/keys-labels-2026-11-26/heldout-plain-a.json),
[heldout-style3.json](data/keys-labels-2026-11-26/heldout-style3.json)).

**Plain A**, 718 schemas, 16 399 declared keys. Each row is held against
the frozen model of its own seed.

| | P / R / F1 declared [95 %] | minus the frozen model | 40 tables or more: P / R / F1 | minus the frozen model | keys drawn (40 or more) |
|---|---|---|---|---|---|
| `eleven--wide`, frozen | 0.774 / 0.741 / 0.757 [0.727, 0.787] | | 0.682 / 0.622 / 0.651 | | 15 711 (5 950) |
| **`eleven-pu--wide-pu@ipw`**, the choice | 0.731 / 0.795 / **0.762** [0.730, 0.793] | **+0.005 [−0.006, +0.016]** | 0.628 / 0.721 / 0.672 | +0.021 [−0.003, +0.049] | 17 842 (7 485) |
| its control, matched | 0.727 / 0.791 / 0.758 [0.728, 0.787] | +0.001 [−0.004, +0.006] | 0.642 / 0.691 / 0.666 | +0.015 [+0.006, +0.024] | 17 842 (7 014) |
| its control, matched by size | 0.728 / 0.792 / 0.759 [0.729, 0.789] | +0.002 [−0.004, +0.008] | 0.623 / 0.715 / 0.666 | +0.015 [+0.002, +0.029] | 17 842 (7 485) |
| `eleven-seed2--wide`, frozen, the other seed | 0.771 / 0.734 / 0.752 [0.722, 0.781] | | 0.687 / 0.610 / 0.647 | | 15 614 (5 788) |
| `eleven-pu-seed2--wide-pu@ipw`, the twin | 0.734 / 0.786 / 0.759 [0.730, 0.790] | +0.007 [−0.001, +0.017] | 0.635 / 0.699 / 0.665 | +0.019 [−0.000, +0.041] | 17 569 (7 168) |
| its control, matched | 0.725 / 0.777 / 0.750 [0.721, 0.779] | −0.001 [−0.007, +0.004] | 0.647 / 0.672 / 0.660 | +0.013 [+0.003, +0.024] | 17 569 (6 770) |
| its control, matched by size | 0.727 / 0.779 / 0.752 [0.724, 0.782] | +0.001 [−0.005, +0.007] | 0.630 / 0.693 / 0.660 | +0.014 [+0.000, +0.028] | 17 569 (7 168) |
| `eleven--wide@size-teachers`, the best post-hoc | 0.731 / 0.788 / 0.758 [0.729, 0.789] | +0.001 [−0.004, +0.007] | 0.645 / 0.688 / 0.666 | +0.015 [+0.007, +0.025] | 17 696 (6 957) |
| `eleven-sar--wide-sar@ipw`, beside | 0.743 / 0.787 / 0.765 [0.733, 0.795] | +0.008 [−0.002, +0.018] | 0.650 / 0.691 / 0.670 | +0.019 [−0.003, +0.042] | 17 373 (6 934) |

The matched control's threshold is 0.674 where the frozen threshold is
0.798 (0.688 against 0.815 for the other seed): the fit draws as many keys
as the frozen model would at a threshold 0.12 lower.

By the number of tables, as the paper's Table 5, the frozen model and the
choice; the other seed in brackets:

| tables | schemas | declared keys | F1, frozen | F1, the choice | recall, frozen | recall, the choice | precision, frozen | precision, the choice | keys drawn, frozen | keys drawn, the choice |
|---|---|---|---|---|---|---|---|---|---|---|
| 5 to 9 | 248 | 1 569 | 0.862 (0.859) | 0.856 (0.857) | 0.878 (0.869) | 0.878 (0.876) | 0.845 (0.850) | 0.835 (0.840) | 1 630 (1 603) | 1 650 (1 636) |
| 10 to 19 | 246 | 3 527 | 0.843 (0.845) | 0.845 (0.849) | 0.856 (0.855) | 0.870 (0.871) | 0.831 (0.834) | 0.821 (0.828) | 3 631 (3 613) | 3 734 (3 709) |
| 20 to 39 | 156 | 4 782 | 0.797 (0.783) | 0.798 (0.791) | 0.774 (0.769) | 0.813 (0.814) | 0.822 (0.797) | 0.782 (0.770) | 4 500 (4 610) | 4 973 (5 056) |
| 40 to 99 | 56 | 4 026 | 0.705 (0.697) | **0.734** (0.718) | 0.664 (0.657) | 0.754 (0.731) | 0.750 (0.741) | 0.716 (0.706) | 3 562 (3 573) | 4 239 (4 174) |
| 100 or more | 12 | 2 495 | 0.568 (0.566) | 0.582 (0.587) | 0.556 (0.534) | 0.669 (0.645) | 0.580 (0.601) | 0.514 (0.538) | 2 388 (2 215) | 3 246 (2 994) |
| 40 or more | 68 | 6 521 | 0.651 (0.647) | 0.672 (0.665) | 0.622 (0.610) | 0.721 (0.699) | 0.682 (0.687) | 0.628 (0.635) | 5 950 (5 788) | 7 485 (7 168) |

Under 20 tables nothing changes. From 20 tables up the choice finds more of
the declared keys, four points at 20 to 39, nine at 40 to 99 and eleven at
100 or more, and draws more that are not declared. F1 over declared keys
rises by 0.029 at 40 to 99 tables and by 0.014 at 100 or more; over the 68
schemas of 40 tables or more the difference is +0.021 [−0.003, +0.049]. The
slope of the paper's Table 5 stays: 0.856 to 0.582.

**Style 3**, 100 schemas, 1 762 declared keys:

| | P / R / F1 declared [95 %] | minus the frozen model | 40 tables or more: P / R / F1 | keys drawn (40 or more) |
|---|---|---|---|---|
| `eleven--wide`, frozen | 0.810 / 0.670 / 0.734 [0.621, 0.841] | | 0.632 / 0.363 / 0.461 | 1 458 (359) |
| **the choice** | 0.759 / 0.729 / **0.744** [0.663, 0.834] | **+0.010 [−0.025, +0.050]** | 0.568 / 0.519 / 0.543 | 1 693 (572) |
| its control, matched | 0.740 / 0.711 / 0.725 [0.627, 0.827] | −0.009 [−0.024, +0.009] | 0.535 / 0.428 / 0.476 | 1 693 (501) |
| its control, matched by size | 0.738 / 0.709 / 0.724 [0.633, 0.827] | −0.010 [−0.036, +0.016] | 0.507 / 0.463 / 0.484 | 1 693 (572) |
| frozen, the other seed | 0.818 / 0.650 / 0.724 [0.599, 0.841] | | 0.635 / 0.327 / 0.432 | 1 399 (323) |
| the twin | 0.772 / 0.696 / 0.732 [0.637, 0.836] | +0.007 [−0.018, +0.041] | 0.577 / 0.438 / 0.498 | 1 589 (475) |
| its control, matched | 0.766 / 0.691 / 0.726 [0.620, 0.832] | +0.002 [−0.017, +0.022] | 0.584 / 0.385 / 0.464 | 1 589 (413) |
| `eleven--wide@size-teachers` | 0.768 / 0.692 / 0.728 [0.624, 0.833] | −0.006 [−0.016, +0.005] | 0.571 / 0.399 / 0.470 | 1 587 (438) |
| `eleven-sar--wide-sar@ipw`, beside | 0.776 / 0.691 / 0.731 [0.625, 0.839] | −0.003 [−0.017, +0.011] | 0.559 / 0.407 / 0.471 | 1 568 (456) |

Style 3 has eight schemas of 40 tables or more, and one of them, of 291
keys, carries the change: the choice draws 256 keys there where the frozen
model draws 85, and finds 0.488 of its declared keys against 0.175 (0.330
against 0.103 with the other seed). Under 40 tables the two read the same
to 0.007.

## 5. The judge: what the added keys are

Claude Sonnet 5.5 at medium effort, which taught nothing here: the Agent
tool's `sonnet`, one agent a packet, round 1t's prompt unchanged, twenty
agents in all ([judging.py](../round-1y/judging.py)). A set's pool is every
pair no declaration names that a compared model draws, and every such pair
that a language model of round 1w named on the set and no compared model
draws (six models' saved replies on plain A, seven on style 3; nothing was
asked again). The sample is drawn by stratum with seed 20261110 and each
answer weighted by its stratum's size over its sample; `unsure` counts as
not real. The judge is told nothing of which model drew a pair.

| | pool | sampled | yes | no | unsure |
|---|---|---|---|---|---|
| **plain A**: drawn by a compared model, schemas under 40 tables | 3 070 | 250 | 135 | 104 | 11 |
| drawn, 40 tables or more | 3 464 | 250 | 162 | 76 | 12 |
| named by one language model only, under 40 | 532 | 60 | 14 | 32 | 14 |
| named by one language model only, 40 or more | 789 | 60 | 34 | 13 | 13 |
| named by several language models, under 40 | 947 | 60 | 48 | 5 | 7 |
| named by several language models, 40 or more | 1 391 | 60 | 49 | 2 | 9 |
| declared keys (controls) | | 60 | 60 | 0 | 0 |
| pairs drawn at random (controls) | | 60 | 0 | 59 | 1 |
| **style 3**: drawn, under 40 | 225 | 120 | 57 | 59 | 4 |
| drawn, 40 or more | 352 | 120 | 64 | 36 | 20 |
| named by one language model only, under 40 / 40 or more | 89 / 280 | 30 / 30 | 19 / 5 | 10 / 4 | 1 / 21 |
| named by several, under 40 / 40 or more | 124 / 186 | 30 / 30 | 24 / 13 | 3 / 2 | 3 / 15 |
| declared keys (controls) | | 60 | 58 | 1 | 1 |
| pairs drawn at random (controls) | | 60 | 1 | 59 | 0 |

Compared on plain A: the frozen model, the choice, its twin, the best
post-hoc candidate, the fit by a schema's own frequency, and the matched
control of each seed. A model's **precision with the judge** is its declared
keys found and the judged-real share of its other keys, over all it draws,
as round 1v counted it. Its **recall over declared and judged-real keys**,
which the paper lacks, is the same numerator over the set's declared keys
plus the judged-real keys of the whole pool. Intervals are 95 % percentile
intervals of 10 000 resamples of the judged pairs within strata
([judge-plain-a-count.json](data/keys-labels-2026-11-26/judge-plain-a-count.json),
[judge-style3-count.json](data/keys-labels-2026-11-26/judge-style3-count.json)).

**Plain A, every schema**: 16 399 declared keys; with the pool's judged-real
ones, 22 766 [22 411, 23 108].

| | keys drawn | undeclared | sampled | called keys | precision, declared | with the judge [95 %] | recall, declared | over declared and judged-real [95 %] | F1 from the last two |
|---|---|---|---|---|---|---|---|---|---|
| `eleven--wide`, frozen | 15 711 | 3 557 | 276 | 218, 79.0 % | 0.774 | 0.956 [0.938, 0.974] | 0.741 | 0.660 [0.649, 0.670] | 0.781 |
| **the choice** | 17 842 | 4 804 | 372 | 267, 71.8 % | 0.731 | **0.928** [0.912, 0.944] | 0.795 | **0.727** [0.719, 0.736] | 0.816 |
| the choice's control, matched | 17 842 | 4 871 | 378 | 256, 67.7 % | 0.727 | 0.915 [0.899, 0.931] | 0.791 | 0.717 [0.708, 0.727] | 0.804 |
| the twin | 17 569 | 4 677 | 378 | 256, 67.7 % | 0.734 | 0.925 [0.909, 0.942] | 0.786 | 0.714 [0.705, 0.723] | 0.806 |
| the twin's control, matched | 17 569 | 4 825 | 385 | 246, 63.9 % | 0.725 | 0.908 [0.892, 0.925] | 0.777 | 0.701 [0.691, 0.711] | 0.791 |
| `eleven--wide@size-teachers` | 17 696 | 4 766 | 370 | 253, 68.4 % | 0.731 | 0.918 [0.902, 0.934] | 0.788 | 0.714 [0.704, 0.723] | 0.803 |
| `eleven-sar--wide-sar@ipw`, beside | 17 373 | 4 461 | 357 | 254, 71.1 % | 0.743 | 0.936 [0.919, 0.952] | 0.787 | 0.714 [0.705, 0.723] | 0.810 |

**Plain A, schemas of 40 tables or more**: 6 521 declared keys; with the
pool's judged-real ones, 10 349 [10 076, 10 615].

| | keys drawn | undeclared | sampled | called keys | precision, declared | with the judge [95 %] | recall, declared | over declared and judged-real [95 %] | F1 from the last two |
|---|---|---|---|---|---|---|---|---|---|
| `eleven--wide`, frozen | 5 950 | 1 891 | 136 | 118, 86.8 % | 0.682 | 0.957 [0.920, 0.994] | 0.622 | 0.550 [0.533, 0.568] | 0.699 |
| **the choice** | 7 485 | 2 781 | 208 | 155, 74.5 % | 0.628 | **0.915** [0.888, 0.943] | 0.721 | **0.662** [0.648, 0.676] | 0.768 |
| the choice's control, matched | 7 014 | 2 508 | 175 | 135, 77.1 % | 0.642 | 0.909 [0.878, 0.941] | 0.691 | 0.616 [0.600, 0.633] | 0.735 |
| the twin | 7 168 | 2 613 | 189 | 140, 74.1 % | 0.635 | 0.906 [0.877, 0.935] | 0.699 | 0.628 [0.612, 0.644] | 0.742 |
| the twin's control, matched | 6 770 | 2 387 | 179 | 124, 69.3 % | 0.647 | 0.901 [0.868, 0.934] | 0.672 | 0.590 [0.572, 0.607] | 0.713 |
| `eleven--wide@size-teachers` | 6 957 | 2 469 | 174 | 134, 77.0 % | 0.645 | 0.912 [0.880, 0.942] | 0.688 | 0.613 [0.597, 0.630] | 0.733 |
| `eleven-sar--wide-sar@ipw`, beside | 6 934 | 2 429 | 182 | 142, 78.0 % | 0.650 | 0.933 [0.901, 0.963] | 0.691 | 0.625 [0.610, 0.642] | 0.749 |

**The differences**, on the same resamples of the judged pairs:

| plain A | every schema: precision with the judge | recall over real keys | 40 tables or more: precision | recall |
|---|---|---|---|---|
| the choice minus the frozen model | **−0.028 [−0.039, −0.016]** | **+0.068 [+0.060, +0.076]** | −0.042 [−0.067, −0.015] | +0.112 [+0.097, +0.128] |
| the twin minus the frozen model (of the other seed) | −0.031 [−0.041, −0.019] | +0.054 [+0.047, +0.063] | −0.051 [−0.074, −0.026] | +0.077 [+0.064, +0.092] |
| the choice's control minus the frozen model | −0.041 [−0.050, −0.031] | +0.058 [+0.051, +0.064] | −0.048 [−0.065, −0.029] | +0.066 [+0.056, +0.077] |
| **the choice minus its control** | **+0.013 [+0.004, +0.022]** | **+0.010 [+0.003, +0.018]** | +0.006 [−0.012, +0.025] | +0.046 [+0.033, +0.059] |
| **the twin minus its control** | +0.017 [+0.007, +0.027] | +0.013 [+0.005, +0.021] | +0.005 [−0.014, +0.025] | +0.038 [+0.025, +0.052] |
| `@size-teachers` minus the frozen model | −0.038 [−0.046, −0.028] | +0.054 [+0.048, +0.061] | −0.045 [−0.061, −0.027] | +0.063 [+0.053, +0.073] |
| `@size-teachers` minus the choice's control | +0.003 [+0.000, +0.005] | −0.003 [−0.006, −0.002] | +0.003 [−0.001, +0.005] | −0.003 [−0.006, −0.002] |
| the fit by a schema's own frequency minus the frozen model | −0.020 [−0.031, −0.009] | +0.054 [+0.047, +0.062] | −0.024 [−0.045, +0.001] | +0.075 [+0.062, +0.090] |

**What the added keys are.** From the same estimates: the frozen model's
15 711 keys are 12 154 declared, about 2 860 judged real and about 690 not.

| plain A | keys added | declared | judged real | not | real share of the added |
|---|---|---|---|---|---|
| the choice, every schema | 2 131 | 884 | about 660 | about 590 | 72 % |
| its control, matched | 2 131 | 817 | about 490 | about 820 | 62 % |
| the twin (over the frozen model of seed 20261027) | 1 858 | 738 | about 500 | about 620 | 67 % |
| the twin's control | 1 858 | 590 | about 350 | about 920 | 51 % |
| the choice, 40 tables or more | 1 535 | 645 | about 510 | about 380 | 75 % |
| its control, 40 tables or more | 1 064 | 447 | about 240 | about 380 | 64 % |
| the fit by a schema's own frequency, 40 or more | 984 | 446 | about 330 | about 200 | 79 % |

**Style 3**: 1 762 declared keys; with the pool's judged-real ones, 2 339
[2 273, 2 406]; at 40 tables or more 626 and 941 [883, 1 000]. Its sample
was packed without the twin's own control.

| | keys drawn | undeclared | sampled | called keys | precision, declared | with the judge [95 %] | recall, declared | over declared and judged-real [95 %] |
|---|---|---|---|---|---|---|---|---|
| `eleven--wide`, frozen | 1 458 | 277 | 114 | 81, 71.1 % | 0.810 | 0.937 [0.913, 0.960] | 0.670 | 0.584 [0.567, 0.602] |
| **the choice** | 1 693 | 408 | 163 | 109, 66.9 % | 0.759 | 0.916 [0.894, 0.938] | 0.729 | 0.663 [0.646, 0.680] |
| the choice's control, matched | 1 693 | 441 | 195 | 106, 54.4 % | 0.740 | 0.888 [0.866, 0.910] | 0.711 | 0.643 [0.625, 0.660] |
| the twin | 1 589 | 363 | 155 | 101, 65.2 % | 0.772 | 0.921 [0.897, 0.944] | 0.696 | 0.625 [0.608, 0.643] |
| `eleven--wide@size-teachers` | 1 587 | 368 | 154 | 95, 61.7 % | 0.768 | 0.908 [0.884, 0.931] | 0.692 | 0.616 [0.598, 0.633] |
| `eleven-sar--wide-sar@ipw`, beside | 1 568 | 351 | 143 | 91, 63.6 % | 0.776 | 0.912 [0.888, 0.935] | 0.691 | 0.611 [0.594, 0.629] |
| the choice, 40 tables or more | 572 | 247 | 81 | 58, 71.6 % | 0.568 | 0.866 [0.809, 0.922] | 0.519 | 0.526 [0.493, 0.560] |
| the frozen model, 40 tables or more | 359 | 132 | 41 | 31, 75.6 % | 0.632 | 0.886 [0.812, 0.967] | 0.363 | 0.338 [0.308, 0.369] |

On style 3 the choice minus the frozen model is −0.021 [−0.038, −0.004] in
precision with the judge and +0.079 [+0.068, +0.090] in recall over real
keys; minus its control, +0.028 [+0.015, +0.042] and +0.020 [+0.011,
+0.030]. At 40 tables or more: −0.020 [−0.080, +0.041] and +0.188 [+0.162,
+0.217] against the frozen model, eight schemas, most of it one.

**The weighted measure agreed with the judge.** On plain A the weighted
counts of §2, which no judge touched, put the frozen model at precision
0.903 and recall 0.700 and the choice at 0.871 and 0.765: a precision 0.032
lower and a recall 0.065 higher, where the judge says 0.028 and 0.068. Its
level is five points under the judge's in precision. The weighted F1 moves
+0.026 [+0.013, +0.041] (the twin +0.027 [+0.016, +0.041]); F1 from the
judge's two figures, +0.035.

**Plain A's own declarations.** The pool gives a third reading of how much
a schema declares, on GitHub's schemas and by another road than round 1v's
two: the declared keys are 0.72 [0.71, 0.73] of the declared and judged-real
keys of plain A, **0.63 [0.61, 0.65] in schemas of 40 tables or more** and
0.80 under 40. It is an upper bound on the share: a real key nobody in the
pool names is not counted.

## 6. A threshold, or the labels?

The specification asked for a diagnosis and not a verdict alone. There are
three readings to tell apart, and the round has a measurement for each.

**The model holds back, and it need not: yes.** One lower threshold on the
frozen model, 0.674 for 0.798, draws 2 131 more keys on plain A, of which
62 % are declared or judged real. Its precision with the judge falls from
0.956 to 0.915 and its recall over real keys rises from 0.660 to 0.717; F1
from the two, 0.781 to 0.804. Over declared keys it reads +0.001 [−0.004,
+0.006]. So the paper's sentence that no threshold mends a large schema is
true of the measure it was read by and not of the model: the threshold that
is best over declared keys is too high once undeclared keys count. Half of
what the lower threshold adds, 1 064 keys, lies in the 68 schemas of 40
tables or more, which hold two fifths of the declared keys: a pair that the
model scores first and under its threshold is mostly a pair of a large
schema, as round 1v found.

**The correction of the labels adds to that: yes, a little, in both
seeds.** At the same number of keys the choice is +0.013 [+0.004, +0.022]
in precision and +0.010 [+0.003, +0.018] in recall over its control, and
the twin +0.017 [+0.007, +0.027] and +0.013 [+0.005, +0.021] over its own.
What it adds is 72 % real against the threshold's 62 % (67 % against 51 %).
It also sends more of its keys to large schemas than a threshold does,
7 485 against 7 014: there its recall is +0.046 [+0.033, +0.059] above the
control's at the same precision (+0.038 [+0.025, +0.052] for the twin). On
style 3 the same: +0.028 and +0.020 over the control. Over declared keys
none of this can be seen: the choice minus its control is +0.004 [−0.005,
+0.014] on plain A, and minus the control matched by size +0.003 [−0.005,
+0.012].

**A threshold by size from the label frequency, with no refit: nothing
beyond one threshold.** `@size-teachers` reads +0.003 and −0.003 against the
one-threshold control. The post-hoc correction of the survey's §5.3.1
lowers the threshold most where the label frequency is lowest; a single
lower threshold already draws there.

So of the choice's seven points of recall over real keys, about six are a
threshold's and one is the labels' correction; in large schemas, about
seven and four and a half. And none of it is free: the precision with the
judge falls by 0.028, and by 0.042 in large schemas, with both intervals
clear of zero.

**Why the correction is weak where it should be strong.** §1's table said
so before any fit. The weight of an unlabelled pair is computed from the
model's own score, and at 40 tables or more the weights sum to 27 % of the
undeclared keys that the label frequency implies. Two readings were left
open there. The fit by a schema's own frequency was added to tell them
apart, and it does so only in part. Its weights reach 64 % of what is
implied, which says that the declarations are missing by schema and not at
random within a size: seven training schemas in ten of 40 tables or more
declare every `table_id` column, and the missing third sits in the others.
The keys it adds on plain A are cleaner (79 % real at 40 tables or more
against the choice's 75 %, precision with the judge 0.936 against 0.928)
and fewer (984 against 1 535), and its F1 from the judge's two figures,
0.810, lies between the choice's 0.816 and the twin's 0.806. Its two seeds
chose thresholds far apart, 0.824 and 0.888, so the comparison is of two
operating points and not of two methods. What stays unseparated: whether
the remaining third of the implied keys are keys the model cannot see from
names at all, or keys whose label frequency the name rules overstate.

## 7. What this means for the paper's explanation

Against the specification's test, written before any fit:

- *draws more keys in schemas of 40 tables or more*: **yes**, 26 % and 24 %
  more in the two seeds;
- *recall over declared and judged-real keys above the frozen model's, the
  interval over zero*: **yes**, +0.112 [+0.097, +0.128] in large schemas,
  +0.068 over the set;
- *precision with the judge not below the frozen model's beyond its
  interval*: **no**, −0.042 [−0.067, −0.015] in large schemas and −0.028
  [−0.039, −0.016] over the set;
- *above its matched control with the interval over zero, in both seeds*:
  **yes** in recall in large schemas, and in both precision and recall over
  the set.

The test's "supported" needed all four, so **the explanation is not
supported in the form the question gave it**, more keys in large schemas at
a precision that holds. Its "not supported" clause applies by its letter:
what the choice adds the judge calls real less often than what the frozen
model draws, 72 % against 96 %. What the measurements do support is
narrower, and it is three statements.

1. **The labels do hold the model back, and most in large schemas.**
   Corrected by a standard method that asks no language model, the model
   finds 0.662 of the real keys of a large plain schema in place of 0.550.
2. **Most of that is not specific to the labels.** A lower threshold finds
   0.616. The part that only the correction gives is 0.046 in large schemas
   and 0.010 over the set, and it repeats with another seed.
3. **Incomplete labels do not explain the distance to the language models
   over declared keys.** That distance is what the paper's Table 5 shows,
   and the corrected model's F1 there is 0.762 against 0.757. Its recall of
   declared keys rises to 0.795 at a precision against the declarations of
   0.731; the six language models of round 1w find 0.845 to 0.977 of plain
   A's declared keys at 0.695 to 0.801. At the precision of the language
   models the corrected model is still 0.05 to 0.18 of recall short.

What the paper can say that it could not: its declared precision
understates the model, the labels' correction and a lower threshold both
buy recall at a known price, and a measure weighted by the label frequency
follows the judge without one.

### Sentences for the paper

**§5.3**, after the paragraph on the two measures:

> A third reading, on other data, agrees. On plain A we pooled every
> undeclared pair that our models or the six language models name, and the
> judge read a sample of the pool. The declared keys are 0.72 of the
> declared and judged-real keys, and 0.63 [0.61, 0.65] in schemas of 40
> tables or more. This is a development reading, made after the set had
> been read.

**§5.4**, in place of the sentence that the taught variant "shows the
direction that better labels would allow":

> We then corrected the labels without any language model. Each undeclared
> candidate was used twice in training, as a key with the weight that
> positive-unlabelled learning gives it from the label frequencies of
> Table 6, and as a non-key with the rest [elkan2008learning; bekker2020pu].
> On plain A, by then development data, the corrected model predicts 14 %
> more keys, and 26 % more in schemas of 40 tables or more. Its F1 over
> declared keys does not move: 0.762 against 0.757, +0.005 [−0.006,
> +0.016]. Counted with the judge, its recall over declared and judged-real
> keys rises from 0.660 to 0.727 and its precision falls from 0.956 to
> 0.928; in schemas of 40 tables or more, from 0.550 to 0.662 and from 0.957
> to 0.915. Lowering lemmekey's threshold until it predicts as many keys
> gives most of this, a recall of 0.717 at a precision of 0.915, and the
> corrected model is above that by 0.010 and 0.013, with both intervals
> over zero and the same with another seed. So the labels do make the model
> hold back in large schemas, but correcting them trades precision for
> recall, and it does not close the distance to the language models over
> declared keys.

**§6.4**, in place of "We measured how many keys are undeclared, but we did
not apply the corrections that positive-unlabelled learning offers, such as
weighting by an estimated label frequency [bekker2020pu].":

> We applied one correction that positive-unlabelled learning offers, the
> weighting of unlabelled examples by an estimated label frequency
> [bekker2020pu], after the clean sets had been read, so its figures are
> development readings (§5.4). The correction assumes that keys go
> undeclared at random within a schema size. They do not: seven in ten
> training schemas of 40 tables or more declare every column named
> `table_id`, and the weights account for a quarter of the undeclared keys
> that the label frequency implies there. The recall over declared and
> judged-real keys is over a pool of keys that some model named, so it is
> an upper bound, and the judge is one of the pool's models.

`elkan2008learning` is a new reference: Elkan and Noto, "Learning
classifiers from only positive and unlabeled data", KDD 2008.

## 8. Plain B

**Recommendation: do not spend plain B on this result.**

- The paper's measure would not see it. Over declared keys the choice is
  +0.005 [−0.006, +0.016] on plain A's 718 schemas, and plain B has as many:
  a clean reading would most likely return an interval across zero.
- The measure that does see it needs the judge again, on a sample, and what
  it would confirm is a trade of 0.03 of precision for 0.07 of recall of
  which a threshold gives most. That does not change which model the paper
  reports.
- Plain B is the last clean set of this pool. It is worth more for a model
  that claims to move F1 over declared keys, or for a model chosen to ship.

If the owner decides otherwise, what would be read is frozen and nothing
may be chosen again: **`eleven-pu--wide-pu@ipw`**, first stage
`corpus/round-1y/first/eleven-pu/keys-first.ubj` (SHA-256 `db8d3713e5180644f105fa92d0a5071f4a191bb4392aa55a3ff842ed315e2a96`), second
stage `corpus/round-1y/second/eleven-pu--wide-pu/keys-second.txt`
(`0724c4d7f7710c682f08c74d9c34d6e0d4a19b320c5538355e20381831458172`), one threshold, 0.824020, for every size, as
[frozen.json](data/keys-labels-2026-11-26/frozen.json) and
[variants.json](data/keys-labels-2026-11-26/variants.json) have them; beside
it its twin `eleven-pu-seed2--wide-pu@ipw` (threshold 0.801915),
`eleven--wide` and `eleven-seed2--wide` as round 1v froze them, and each
seed's control matched on plain B itself; the judge's sample drawn as §5
draws it.

## Limits

- **Every figure is a development reading.** Plain A and style 3 were read
  by `eleven--wide` on 10 November; version 10's and 11's cues were not
  written from them, and nothing of this round was chosen on them, but they
  are not clean.
- **The rule chose one of seven candidates that development data cannot
  tell apart**, as round 1v's did. What is shown is the recipe's direction
  in two seeds, not the pick.
- **The judge is one model, Claude Sonnet 5.5**, as the Agent tool serves
  it under the name `sonnet`; an agent started the same way and asked gave
  its identifier as `claude-sonnet-5-5`. It is one of the six language models
  whose keys make the pool's second half, so it may call its own family's
  keys real more readily; the precision figures do not use that half, the
  recall figures do. Its controls are easier than the pairs it judged. On
  style 3 it refused one declared key of sixty, was unsure of one, and
  called one random pair a key.
- **The recall is over a pool**: a real key that no compared model and no
  language model named is not counted, so every recall over declared and
  judged-real keys is an upper bound, and the declared shares of §5 with it.
- **The judge's intervals hold the databases fixed.** They are the sampling
  error of 860 and 480 judged pairs, not of 718 and 100 schemas; the
  intervals over declared keys resample the schemas and are wider.
- **A matched control is matched on the set it is read on**, so its
  threshold is chosen with the set in view; it is a yardstick and not a
  model. The twin was held against its own control on plain A only.
- **The label frequency is SchemaPile's**, read on GitHub's schemas. §5's
  pool puts plain A's at 0.63 in large schemas where the name rules say
  0.66; under 40 tables the two were not compared size by size.
- **The fit by a schema's own frequency** uses a count of five that nothing
  chose, and its two seeds' thresholds are far apart. It was not a
  candidate.
- **The specification's test was written too coarsely.** Its three verdicts
  did not foresee a correction that passes every part but the precision's;
  §7 reports the parts one by one for that reason.
- **Keys of several columns are left out of every set**, and a column of
  one drawn counts as false, as in every earlier round.
- **No reader in Rust** was built for any fit here; the choice has the
  frozen model's trees' shape and cues and would pack to the same bytes,
  which was not measured.
- **Shared machine.** Two other rounds ran beside this one. One first-stage
  fit, two second-stage jobs and one reading of plain A were lost to memory
  and run again; nothing a lost job had written was kept or counted. The
  specification's notes have each.
- **Nothing here is committed**: the round's code ran from the working tree
  on `9043b62`, where the `measure` skill asks for the harness to be
  committed first; this round was told not to commit.

## What follows

- **For the paper**: the three passages of §7. Table 5's slope and the
  distance to the language models stay as they are.
- **For the model**: a variant for a reader who wants recall exists in two
  forms, a threshold of about 0.67 on `eleven--wide`, which needs no new
  file, and `eleven-pu--wide-pu@ipw`, 0.012 of F1 with the judge above it.
  Whether `see` should draw at the lower threshold is a product question:
  round 1u's measure of questions through keys would say what three points
  of precision cost a first screen.
- **For the labels**: the correction undercorrects where declarations are
  missing by schema. The next step that needs no language model is a
  propensity read from the schema itself and not from its size, estimated
  and not set by hand; round 1v's teachers remain the other road.
- **Plain B stays clean**, unread, for the next model.

## Environment and reproduction

| | |
|---|---|
| machine | AMD Ryzen 7 7700, 31 GB, NVIDIA GeForce RTX 5070, Windows 11 Pro; shared with two other rounds' jobs throughout, the card and the pair builds under lock files |
| Python | 3.13.15; xgboost 3.4.1, lightgbm 4.7.0, scikit-learn 1.9.1, numpy 2.5.4, scipy 1.18.1 |
| frozen model | `eleven--wide` and `eleven-seed2--wide` as round 1v froze them, checked against `corpus/round-1v/frozen.json` when this round froze; thresholds 0.798 and 0.815 |
| language models | none asked for labels or keys. The judge: Claude Sonnet 5.5 at medium effort as Claude Code agents (`sonnet`), 26 November 2026, twenty packets. The pool's second half: round 1w's saved replies of 14 November. Claude Fable 5.2 as the round's agent, which wrote the code and this report |
| research commit | `9043b62` with the round's files in the working tree, uncommitted |
| seeds | 20261027 for every fit and 20261028 for its twin; 20261101 for the resamples; 20261110 for the judge's samples |
| times (UTC, 26 November) | the specification 15:03; the weights' table 15:04; the training pairs built 15:10–15:14; first stages on the card 15:29–15:37, 15:38–15:49, 15:49–15:58, 15:58–16:10; second stages and their scorings 15:13–16:31; thresholds 16:32; the rule 16:33–16:34; frozen 16:34:09; style 3 read 16:34–16:35, its judge 16:36–16:37; plain A read 16:53–17:19, its judge 17:19–17:21 |

```sh
cd <repo>/lemmesee
P=round-1b/.venv/Scripts/python
export LEMMESEE_LOCKS=<the session's lock folder>     # optional
$P -I round-1y/pu.py weights
$P -I round-1y/fitting.py pairs
for f in eleven-pu eleven-pu-seed2 eleven-sar eleven-sar-seed2; do
  $P -I round-1y/fitting.py first $f && $P -I round-1y/fitting.py score-first $f
done
for s in "eleven wide-pu" "eleven-seed2 wide-pu" "eleven-pu wide" "eleven-pu wide-pu" \
         "eleven-pu-seed2 wide" "eleven-pu-seed2 wide-pu" "eleven-sar wide-sar" "eleven-sar-seed2 wide-sar"; do
  $P -I round-1y/fitting.py second $s && $P -I round-1y/fitting.py score-second $s
done
$P -I round-1y/variants.py thresholds
$P -I round-1y/variants.py choose
$P -I round-1y/variants.py freeze
$P -I round-1y/reading.py style3
$P -I round-1y/reading.py plain-a
$P -I round-1y/judging.py pack style3      # then one agent a packet, round-1t/judge-prompt.md; then:
$P -I round-1y/judging.py count style3     # the same for plain-a
$P -I round-1y/tables.py rule              # and: set <set> [<variant> ...]; judge <set>
$P -I round-1y/gather.py reports/data/keys-labels-2026-11-26
```

It needs round 1v's corpus as that round left it: its kept pairs of the
development sets, its frozen models with their scores out of fold, and the
hidden schemas of plain A and style 3. The judge's answers are in
`judge-<set>-pairs.json` as they came; they are not reproducible by asking
again. No schema's text is published, and plain B is its hash.
