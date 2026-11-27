# Round 1w: the paper's key model, measured whole

Written at 10:27 UTC on 14 November 2026, before anything here was run.

The owner chose on 14 November that the key model's paper reports
`eleven--wide`, round 1v's choice, in place of `nine-mended`. Round 1v read
it on the development sets and once on two clean sets
([keys-size-2026-11-10.md](../reports/keys-size-2026-11-10.md)); round 1t gave
`nine-mended` every other number the paper carries
([keys-final-2026-11-09.md](../reports/keys-final-2026-11-09.md)). This round
gives `eleven--wide` those numbers, each through the code that measured it
first, with `nine-mended` read beside it in the same run.

**No model is fitted and none is chosen.** The model is `eleven--wide` as
round 1v froze it at 00:21:24 UTC on 10 November: its three files are checked
against `corpus/round-1v/frozen.json` before every measurement, and its
threshold is the one its validation split chose then. The paper gives it a
name of its own, the owner's to choose; the reports keep the name of the fit.

## What is development data

Every set here was read before by this model or by the models it descends
from: SchemaPile's splits, Spider, BIRD, CTU, LiveSQLBench, the four GitHub
development sets, Nexus's sets and the owner's database A. Plain A and
style 3 were read once by the frozen model in round 1v and are development
data since; this round does not read them again with any tree model
(`clean.py read` refuses), and reads no schema of plain B, which stays
frozen and unread.

## The measurements

From the model's kept scores of round 1v (`corpus/round-1v/first/eleven/`
and `second/eleven--wide/sets/`), no pair built again:

1. **The main table**: over every declared key of round 1p's 398 external
   databases and of LiveSQLBench, P, R and F1 with intervals and macro F1,
   by source, and the model minus round 1's rules and minus each of round
   1s's four language models from their saved replies, as round 1t's
   `paper.py lms` counts them. No language model is asked.
2. **The fresh GitHub set** against the four language models' saved replies
   of 9 November, as `paper.py fresh-lms`. The set is development data for
   this model: version 10's cues were written from its misses, and the
   report says so beside the figure.
3. **By the number of tables**, the model beside the language models on the
   520 databases they answered, as round 1v's `sizes.py`.
4. **The recall bounds**: the candidates' coverage, F1 at its threshold, at
   the set's best threshold and with each schema at its own, as round 1t §3.

With the pairs built again, one job at a time (the machine holds one):

5. **SchemaPile-Perm's test, the pair task and related or not**, round 1s's
   `refit.score` and round 1p's protocol; **Nexus's sets**, round 1p's
   `nexus.py`; **the owner's database A**, counts only.
6. **The reader in Rust**: versions 10 and 11 of the cues and the wide
   second stage added to round 1t's reader; parity against XGBoost and
   LightGBM over the external databases and SchemaPile-Perm's test (hashed
   features, both stages' margins, the keys drawn); a database's time and
   resident memory in one run of the Linux container, round 1s's reader
   with `1s`, round 1t's with `nine-mended` and this round's with
   `eleven--wide` in turn, twice; the size of the packed file and lexicons.

Asked of language models, only with the owner's approval of the requests:

7. **The four language models on plain A and style 3**, under round 1s's
   protocol and round 1p's prompt unchanged, one reply a schema. They have
   not seen either set; the tree models' figures stay those of round 1v's
   one reading.

## The measures

Round 1v's, unchanged: precision, recall and F1 over every declared
one-column key, a key right when its column and its table are; 95 %
percentile intervals of 1 000 resamples of databases (seed 20261101), a
difference resampling both arms together; macro F1 beside every pooled F1.

## What counts as an answer

There is no test to pass: the round reports what the paper's model reads.
Two checks say the code reads it right. `nine-mended`, read through the same
functions from its kept scores, must give round 1t's figures to the third
digit (0.755 over the 398 databases, 0.542 on LiveSQLBench, 0.715 on the
fresh set); and `eleven--wide` on each development set must give round 1v's
`development.json` figure.
