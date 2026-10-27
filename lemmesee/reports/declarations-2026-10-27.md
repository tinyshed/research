# Guessing what a schema did not declare, 27 October 2026

Round 1b of [the plan](../plan.md), as [its specification](../round-1b/README.md)
sets it: whether a small model, learned from what SchemaPile-Perm's schemas
declare, guesses which column is a foreign key and to which table, and which
is a closed list of values, better than round 1's rules, with the
declarations hidden from both. Measured at research commit `a7a2f40`; raw
output in [data/declarations-2026-10-27/](data/declarations-2026-10-27/).

| Question | Answer | What follows |
|---|---|---|
| Does a small model guess foreign keys better than the rules? | **Yes.** The trees reach F1 0.660 against the rules' 0.442, and 0.652 against 0.447 on the test schemas no training schema resembles, the difference's 95 % interval there [+0.158, +0.256]. At the rules' own precision, 0.77, they find 46 % of the keys where the rules find 31 % | the trees are the candidate to enter `roles.rs` as evidence of a kind of their own, once read in Rust within chapter 14's millisecond a column and the plan's mebibyte |
| And the logistic model? | It passes the line too, 0.643 and 0.610, at 128 KiB — but at the rules' precision it finds fewer keys than they do, 29 % against 31 %: its gain is a different trade, more keys for less precision | the fallback, if the trees' memory fails |
| Closed lists? | Both models pass the line as written: F1 0.30 and 0.30 against the rules' 0.08, and 0.20 to 0.25 on the novel schemas. The label counts every list nobody closed as a negative, so every precision here is a lower bound | not carried until the fixtures of round 1, judged, say whether the guesses are right |
| Does an index give the declaration away? | No: every model, with and without it, within 0.03 of F1 | indexes stay among the features |
| Did the models learn the template schemas? | Little for keys, whose F1 fell 0.01 to 0.03 on the novel schemas; more for closed lists, which fell 0.08 to 0.10 | the novel schemas are the measure to quote |
| What did the dry run find? | Two bugs in the rules: a link table's columns were never guessed foreign keys, and plurals in `-ies` were not read. Fixed, they raised the rules' F1 for keys from 0.352 to 0.442 | fixed in round 1's rules, and said in its `candidates.md` |
| Where do the rules still miss? | 9 737 of the 13 461 one-column keys declared in the test: 4 905 whose name does not end in `id`, 4 267 named for something else than their table (`author_id` to `users`), 437 named for their table, 128 named `id` alone | what no rule of names reaches, and what the model learns |

## The data

SchemaPile-Perm, Zenodo record 12682521, version 0.2, the file's hash pinned
in `round-1b/hashes.txt`. Schemas of over 100 tables were left out — 126,
which hold most of the possible pairs — and 22 863 kept, from 14 537
repositories. Split by repository, seed 20261027:

| | Schemas |
|---|---|
| training | 16 244 |
| validation, for thresholds | 2 095 |
| test | 4 320, of which 3 209 novel: no training schema shares half its tables' names |
| dropped, their signature also in training | 204 |

In them, 90 804 one-column foreign keys are declared, in 11 471 schemas, and
6 688 closed lists; 2 980 keys over several columns were left out.

| Task | Training rows (positive) | Test rows (positive) | Novel test rows (positive) |
|---|---|---|---|
| keys, as pairs | 516 847 (61 776), negatives thinned to 10 % | 784 893 (11 561) | 447 487 (8 954) |
| closed lists | 476 612 (4 674) | 115 414 (1 163) | 80 418 (946) |

## Foreign keys

Pairs of a column and a table with a one-column key of the same kind of type,
in the schemas that declare at least one foreign key. Thresholds chosen on
the validation split for the best F1; the interval is of the model's F1 less
the rules', from 1 000 resamples of test repositories.

| | Precision | Recall | F1 | Interval | Novel: P | R | F1 | Interval | Weights |
|---|---|---|---|---|---|---|---|---|---|
| rules | 0.769 | 0.310 | 0.442 | — | 0.782 | 0.313 | 0.447 | — | — |
| logistic, with the index | 0.569 | 0.738 | 0.643 | +0.155 to +0.245 | 0.537 | 0.705 | 0.610 | +0.106 to +0.221 | 128.0 KiB |
| logistic, without | 0.580 | 0.729 | 0.646 | +0.159 to +0.248 | 0.548 | 0.698 | 0.614 | +0.109 to +0.225 | 128.0 KiB |
| trees, with the index | 0.575 | 0.774 | 0.660 | +0.177 to +0.261 | 0.573 | 0.758 | 0.652 | +0.158 to +0.256 | 966.7 KiB |
| trees, without | 0.574 | 0.772 | 0.658 | +0.175 to +0.259 | 0.568 | 0.757 | 0.649 | +0.156 to +0.253 | 967.1 KiB |

**At the rules' own point** the models compare without a threshold of their
own: where they recall as much as the rules, 31 %, the trees are 0.831
precise and the logistic model 0.766, against the rules' 0.769; where they
are as precise as the rules, the trees recall 0.457 and the logistic model
0.291, against 0.310. **Only the trees beat the rules at the rules' own
trade.** For `see`, where a wrong key draws a wrong join, that trade is the
one that matters; F1's, with a precision near 0.57, is not obviously better
for a panel than the rules' precision of 0.77.

Average precision, the area under the curve: 0.634 and 0.629 for the
logistic model, 0.682 and 0.677 for the trees.

## Closed lists

Text and integer columns, keys excepted, positive when declared `Enum` or
checked against a list.

| | Precision | Recall | F1 | Interval | Novel: P | R | F1 | Interval | Weights |
|---|---|---|---|---|---|---|---|---|---|
| rules | 0.044 | 0.372 | 0.079 | — | 0.061 | 0.418 | 0.107 | — | — |
| logistic, with the index | 0.428 | 0.233 | 0.302 | +0.141 to +0.304 | 0.352 | 0.145 | 0.205 | +0.045 to +0.160 | 124.5 KiB |
| logistic, without | 0.445 | 0.223 | 0.297 | +0.136 to +0.303 | 0.366 | 0.134 | 0.196 | +0.035 to +0.154 | 124.5 KiB |
| trees, with the index | 0.335 | 0.273 | 0.301 | +0.148 to +0.302 | 0.267 | 0.185 | 0.218 | +0.053 to +0.172 | 967.5 KiB |
| trees, without | 0.326 | 0.337 | 0.331 | +0.181 to +0.328 | 0.262 | 0.238 | 0.249 | +0.083 to +0.203 | 966.8 KiB |

The rules call a column a category by its name — `status`, `type`, `country`
— and are right about a declared list one time in twenty-three: most such
columns were never closed by their designers, which this label counts as a
miss. The models are precise four to ten times as often; on the novel
schemas they lose a quarter to a third of their F1, the clearest sign in
this round of what the template schemas taught them.

## The lines set before the run

A model replaces a rule only if its F1's interval excludes zero, on all the
test schemas and on the novel ones, and its weights fit chapter 14's 2 MiB
in the binary; the plan's 256 KiB is reported beside it.

| | Keys | Closed lists | 2 MiB | 256 KiB |
|---|---|---|---|---|
| logistic | passes | passes | 128 KiB | yes |
| trees | passes | passes | 967 KiB, a text file | no |

The spec named 256 KiB as the deciding line until the dry run; it was set to
chapter 14's before this run, as the plan says chapter 14 decides, and both
are given. The trees' size is LightGBM's text model; a format of our own
would be smaller, and what they take in memory once read, against the plan's
1 MiB, is not measured here.

## The rules' misses

Of the 13 461 one-column keys declared in the test — the pairs above hold
11 561 of them, the rest pointing at a table whose key is over several
columns or of another kind of type — the rules missed 9 737:

| Why | Keys |
|---|---|
| the name does not end in `id`: `owner`, `author`, `ProductCode` | 4 905 |
| named for something else than its table: `author_id` to `users` | 4 267 |
| named for its table, and missed for its type or its parent's key | 437 |
| named `id` alone | 128 |

Before the fixes, 1 358 keys named for their table were missed, 88 of them
plurals in `-ies`; the fixes took the count to 437. These counts come from a snippet over
`guesses.jsonl`, not from the harness, and are reproducible from the files
listed below.

## Environment and reproduction

| | |
|---|---|
| research commit | `a7a2f40b4f2b68eb90ae7d573f70ac61ad359908` |
| corpus | SchemaPile-Perm, Zenodo 12682521, `schemapile-perm.json.gz`, SHA-256 `4c48ff18…ecc6` |
| machine | AMD Ryzen 7 7700, 8 cores, 16 threads, 31 GiB visible to Windows; Windows 11 Pro, build 26300 |
| Python | 3.13.15, numpy 2.5.4, scikit-learn 1.9.1, scipy 1.18.1, LightGBM 4.7.0 |
| Rust | rustc 1.97.1, the `rules` crate at the commit |
| run | 27 October 2026, 18:26:50 to 18:34:00 UTC: fetch and check, prepare 14 s, the rules' guesses 6 s, the models 6 min 49 s |

```sh
cd <repo>/lemmesee
sh round-1b/fetch.sh
python -m venv round-1b/.venv
round-1b/.venv/Scripts/python -m pip install -r round-1b/requirements.txt
round-1b/.venv/Scripts/python -I round-1b/prepare.py corpus corpus/round-1b
cargo build --release --manifest-path round-1/harness/Cargo.toml -p rules
round-1/harness/target/release/guess corpus/round-1b/snapshots.jsonl corpus/round-1b/guesses.jsonl
round-1b/.venv/Scripts/python -I round-1b/train.py corpus/round-1b
```

Every number above follows from the seed and the data; the times of fitting
in `results.json` are a desktop's and an anecdote. Accuracy is not a
performance measurement, which is why this ran on Windows rather than in the
Linux container.

## What follows

- **The trees are the model to carry for keys**, in a format read by Rust
  in `roles.rs`, with the memory it takes and its time a column measured in
  the container against the plan's mebibyte and chapter 14's millisecond.
  Their guesses enter as evidence of a kind of their own, weaker than a
  declaration, and a guessed join stays flagged.
- **For closed lists nothing is carried yet.** The label is too weak to
  trust its precision; round 1's fixtures, judged, will say how often the
  models' lists are categories a person would group by.
- **Round 1's A on MySQL and on undeclared schemas** is where this helps:
  the next run of round 1 can compare A with the rules' keys and A with the
  trees' on the same fixtures, Chinook's keys hidden.
- **Creation times** need a corpus that keeps a column's default; SchemaPile's
  raw SQL files may, its JSON does not.
