# The look taught at scale, 28 October 2026

Round 1e, as [its description](../round-1e/README.md) sets it, built and run
by the agent the same day as round 1d: the look taught by three cheap judges
over 41 databases instead of one judge over 27, reading more of each result,
and the five's penalties and the look's weight tuned on what the teachers
keep of the five. Counted by judges that taught nothing, on round 1d's 28
databases left out one at a time and once on ten new ones held out.
Measured at research commit `5b679a0` with this round's files uncommitted;
raw output and every judge's answers in
[data/look-scale-2026-10-28/](data/look-scale-2026-10-28/).

**Round 1f [found](scale-2026-10-28.md) the tuned choice unstable**: one
teacher's labels on two databases of 81 swung its penalties from heavy to
none and a count on the 28 by 0.3. Round 1e's differences from round 1d of
that size are not settled by these counts; its gain over rounds 1 and 1c
is, and round 1f measured round 1e on twelve more databases at 3.17 and
3.83 of five against round 1d's 2.83 and 3.42.

| Question | Answer | What follows |
|---|---|---|
| Where did round 1d's five lose what its pool holds? | **In the order, not the pool**: a five from the same candidates could keep 4.73 by Opus, under round 1d's penalties too; round 1d's keeps 2.85. Within a database the look orders what Opus keeps at an AUC of 0.75, the ranker's score at 0.66, the two added at 0.74 | the look carries the order; the score adds nothing to it |
| On ten databases held out, does round 1e's five keep more? | **Yes: 3.4 of five by Sonnet and 3.3 by Gemini**, against round 1d's 2.5 and 2.4 and round 1's 2.2 and 2.7 | the clearest gain any round has measured, on databases nothing was tuned on |
| Is it more than chance there? | Against round 1d's look: **Gemini +0.9 [+0.4, +1.5]**. Against round 1c: Sonnet +1.0 [+0.3, +1.7], Gemini +1.4 [+0.8, +2.0]. Against round 1: Sonnet +1.2 [+0.5, +1.9], Gemini +0.6 [0.0, +1.2] | over ten databases, so wide; each clear of zero but one at its edge |
| And on round 1d's 28? | **No better than round 1d's look**: Opus 2.78 against 2.78, Sonnet 2.79 against 2.86, Gemini 2.89 against 2.68. Against round 1 and 1c, Opus +0.48 [+0.04, +0.96] and +0.41 [+0.04, +0.81] | where round 1d's look was taught by a judge of the counting judges' own camp, cheap teachers reach it and do not pass it |
| What brought the gain on the ten? | Each change, taken away, costs 0.4 to 0.6 there: taught on round 1d's 28 only, 3.0 and 2.8; without the result's new numbers, 2.8 and 2.7; with round 1d's penalties and weight, 3.0 and 2.8. On the 28 none of the three moves the count | more databases, more of the result read and the choice tuned on keep all count where the databases are new |
| What does the tuned choice look like? | **The look alone orders**, the ranker's score only breaking ties; a penalty of 0.25 for one time with the same measure and for one breakdown with any measure; nothing for one base or one measure — in 36 of 38 folds alike | round 1d's penalty of 9 for one breakdown with another measure goes |
| What does it cost? | Jev answered 710 panels for 1.9 cents; the look is still 150 trees of four leaves | labels are not what limits it |

## Where round 1d lost

`gap.py` read round 1d's five beside its pool on round 1d's 28 databases,
the look out of fold as round 1d measured it.

| | Opus | Sonnet | Gemini |
|---|---|---|---|
| the best five the pool holds | 4.73 | 4.71 | 4.75 |
| the same, under round 1d's penalties | 4.73 | 4.71 | 4.71 |
| round 1d's five | 2.85 | 2.86 | 2.68 |
| AUC within a database: the ranker's score | 0.66 | 0.65 | 0.64 |
| the look | 0.75 | 0.75 | 0.73 |
| the two added, as round 1d chose | 0.74 | 0.74 | 0.72 |

42 % of round 1d's five counted or summed what another of its panels already
did, over another time or breakdown — orders by the date ordered, required
and shipped — and Opus kept 0.47 of those against 0.64 of the rest. What
Opus called wrong in the rest was in the result: one label twice where names
are not keys, codes as bare numbers (formations 2, 6, 8; occupations 1 to
5), a year of 1129 stretching an axis, 1899-01-01 as a placeholder, ties at
the twelfth, home matches counted as all matches.

## The databases

Twenty-three more from CTU's repository, dumped once, pinned in
`round-1/fixtures/hashes.txt` and loaded by `fixtures/ctu.sh`; Sakila,
Employees, AdventureWorks, `financial` and Stack Exchange's left out as pilot
fixtures, and sets made for learning with no business behind them left out.
**Ten held out**, drawn with seed 20261030 before any was loaded:
Accidents, CDESchools, ConsumerExpenditures, FNHK, Hepatitis_std, legalActs,
nhl_draft, SAP, Seznam and Walmart_2014. Thirteen taught on with round 1d's
28: AustralianFootball, Countries, FlexiBee, geneea, GOSales, imdb_full,
SalesDB, Telstra, Toxicology, tpcc, TubePricing, Walmart and
YelpDataset3_disc_clean.

Each pool is round 1d's: round 1c's ranker's thirty best, its five and round
1d's choice, and round 1's B1 five. 710 distinct panels, 322 on the ten.
Statements past round 1's time limit stay errors, as `see` would show them:
11 of SalesDB's 33 on 6.7 million sales, 3 of SAP's.

## The judges

| Judge | How it was asked | Role |
|---|---|---|
| Jev 1.13 | each panel alone, OpenRouter's decisions endpoint, a probability | teaches |
| DeepSeek V4.1 Flash | a prompt per database, OpenRouter, all at once | teaches |
| GLM-5.3 Flash | the same; two of 23 answers took twenty minutes | teaches |
| Claude Sonnet, medium effort | an agent per packet of five databases; the ten only | counts |
| Gemini 3.8 Flash | a prompt per database through the CLIProxyAPI; the ten only | counts |
| Claude Opus | its answers from round 1d; the 28 only | counts |

The look learns the teachers' mean keep, Jev's probability with the others'
verdicts, as cross-entropy. On the ten, the teachers agree with the counting
judges on keep for 65 to 74 % of panels, Sonnet and Gemini with each other
for 84 %.

## The fives

Kept of five, averaged over the databases each judge counts: on round 1d's
28, each left out in turn, the look fitted on the other 40 taught databases
and the penalties tuned on them; on the ten, fitted and tuned on all 41.
Round 1d's look is round 1d's own, taught by Sonnet on the 28, and Sonnet
never counts it.

| Five | 28: Opus | Sonnet | Gemini | ten: Sonnet | Gemini |
|---|---|---|---|---|---|
| round 1, B1 | 2.30 | 2.43 | 2.36 | 2.2 | 2.7 |
| round 1c | 2.37 | 2.46 | 2.54 | 2.4 | 1.9 |
| round 1d, learned choice | 2.37 | 2.54 | 2.46 | 2.4 | 2.4 |
| round 1d, the look taught by Sonnet | 2.78 | (2.86) | 2.68 | (2.5) | 2.4 |
| **round 1e** | **2.78** | **2.79** | **2.89** | **3.4** | **3.3** |
| round 1e, taught on round 1d's 28 only | 2.81 | 2.86 | 2.75 | 3.0 | 2.8 |
| round 1e, without the new numbers | 2.81 | 2.79 | 2.86 | 2.8 | 2.7 |
| round 1e, round 1d's penalties and weight | 2.74 | 2.75 | 2.82 | 3.0 | 2.8 |
| the best five the pool holds | 4.74 | 4.71 | 4.75 | 5.0 | 5.0 |

Opus is counted on 27 of the 28, where it answered every panel. 40 % of
round 1e's fives still repeat a measure, as 45 % of round 1d's did; Gemini
keeps 0.58 of round 1e's repeats against 0.61 of the rest, so the look
chooses better ones rather than fewer.

The ten were counted twice, with the same configuration: first with 21 of
GLM's 23 answers while two hung, then with all; the second gave Sonnet 3.4
for 3.3, Gemini 3.3 for 3.3, and is the one reported.

## What it means, and what does not follow

- **New databases gain, judged ones do not.** On the ten, every change
  counts and round 1e keeps about one panel more of five than round 1d. On
  the 28 it does not pass round 1d's look, whose teacher, Sonnet, is of the
  camp of Opus and Gemini. Ten databases is a small sample, and the gain
  there may be smaller than it looks.
- **A teacher of another camp reaches the counting judges' own.** Jev,
  DeepSeek and GLM agree with the counters on 65 to 77 % of panels, and
  what they teach keeps as much by Opus as what Sonnet taught.
- **The ceiling is far.** A five from the same candidates could keep all
  five on the ten and 4.7 on the 28; round 1e keeps 3.3 and 2.8.
- **Every judge is still a model**, and fifty-one databases are still few.

## What follows

1. More databases with data: BIRD's 69 training databases, those not
   already CTU's nor pilot fixtures, loaded to PostgreSQL; and, with a
   teacher of the counting camp, more labels per database.
2. Carry the question, the ranker, the penalties and the look into Rust.
3. Round 2's judged set, with people.

## Reproducing

```bash
round-1b/.venv/Scripts/python -I round-1e/gap.py corpus/round-1d/look corpus/see corpus/round-1d <fixture-engine>...
sh corpus/see-1e.sh <ctu database>...
round-1b/.venv/Scripts/python -I round-1e/look.py pool corpus/round-1c/learn corpus/round-1d corpus/see corpus/round-1e/look <fixture-engine>...
round-1b/.venv/Scripts/python -I round-1e/look.py pack corpus/round-1e/look <the ten>
round-1b/.venv/Scripts/python -I round-1e/look.py pack corpus/round-1e/look <the thirteen>
python -I round-1d/gemini.py cut corpus/round-1e/look
JUDGE_KEY=<key> python -I round-1d/jev.py ask corpus/round-1e/look 64
JUDGE_KEY=<key> python -I round-1d/gemini.py ask corpus/round-1e/look <endpoint> <model> <judge> 23
JUDGE_KEY=<key> python -I round-1d/gemini.py ask corpus/round-1e/look <endpoint> <model> gemini 8 --packets=1,2
python -I round-1d/gemini.py gather corpus/round-1e/look <judge>
round-1b/.venv/Scripts/python -I round-1e/look.py evaluate corpus/round-1d/look corpus/round-1e/look corpus/see corpus/round-1d
```

`corpus/see-1e.sh` is local: for each database it dumps CTU's copy, pins
its hash, runs `fixtures/ctu.sh` and round 1's `see.sh`, with a line on what
the database holds. Sonnet answered packets 1 and 2 as agents into
`corpus/round-1e/look/sonnet-<n>.json`. Keys were read from where they are
kept into the environment of the one command that used them, never printed
or written. Measured on Windows 11 with Python 3.13, LightGBM 4.7.0,
PostgreSQL 18.6 and MySQL 8.4.13 in Docker.
