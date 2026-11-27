# What reading the rows finds on Spider and BIRD, 26 November 2026

Round 1x, as [its specification](../round-1x/README.md) set it before any
row was read. The paper's key model, `eleven--wide` (lemmekey there), reads
a schema's names, types and primary keys and no row
([keys-paper-model-2026-11-14.md](keys-paper-model-2026-11-14.md)); the
paper's draft says in §6.3 that no value-based baseline was run. This round
runs the value-based line on the two collections whose rows are at hand:
unary inclusion dependencies found exactly from every row of Spider's 204
and BIRD's 76 databases, then the foreign-key decisions of Zhang et al.
(2010), Rostin et al. (2009) and HoPF (Jiang and Naumann, 2020) written
here from their papers, scored in the paper's task by the paper's measure
through round 1w's code; then two rules that let lemmekey read rows. No
model of lemmekey's was fitted and none was chosen: its three files were
checked against round 1v's `frozen.json`, and it read 0.885 on Spider and
0.829 on BIRD, round 1w's figures, before anything new was scored. Raw
output in [data/keys-values-2026-11-26/](data/keys-values-2026-11-26/).

Every F1 is over every declared one-column key, a prediction a (column,
referenced table) pair; every interval a 95 % percentile interval of 1 000
resamples of databases (seed 20261101), a difference resampling both arms
together. **Spider and BIRD are development data for lemmekey**, read many
times by it and by the models it descends from; the row methods were not
tuned on them, and one of them (Rostin's) was fitted on the other
collection.

| Question | Answer | What follows |
|---|---|---|
| **What does plain inclusion find?** | Every column included in a declared key drawn: F1 **0.523** [0.471, 0.577] on Spider, **0.566** [0.512, 0.637] on BIRD, precision 0.435 and 0.465; lemmekey minus it +0.362 [+0.297, +0.425] and +0.263 [+0.194, +0.330] | inclusion is a candidate generator, as its authors say; more than half of what it draws is accident |
| **And the published decisions on top?** | HoPF's foreign-key step with the keys given: **0.779** [0.725, 0.833] and **0.689** [0.605, 0.766]; Rostin's ten features in a tree fitted on the other collection: 0.672 and 0.701; Zhang's randomness as the ranking: 0.520 and 0.558 | the best row method is 0.106 [0.057, 0.160] and 0.140 [0.055, 0.246] of F1 under lemmekey, which reads no row |
| **Without names?** | HoPF by distribution alone 0.617 and 0.647; Rostin's eight value features 0.517 and 0.593 | the row methods' best figures come from their name features: taken out, they lose 0.04 to 0.16 |
| **What are the rows like?** | Spider: a database's median 38 rows, a table's 11; 451 of 1 045 tables under ten rows, 71 empty, seven databases with no row; 70 of 911 declared keys on an empty column. BIRD: a database's median 90 199 rows; 5 of 606 keys on an empty column | Spider is tiny, not empty; over the keys with a non-empty column the order and the gaps stay (HoPF 0.811 against lemmekey's 0.890) |
| **Do the data keep the declared keys?** | Spider: 30 of 841 non-empty keys have a value with no match. **BIRD: 105 of 601, in 31 of 76 databases**; 81 of the 105 by under a tenth of their distinct values | exact inclusion cannot reach 21.8 % of BIRD's keys; lemmekey's recall there, 0.799, is above that ceiling, 0.782 |
| **lemmekey's keys filtered by inclusion?** | Spider 0.888, lemmekey minus it −0.003 [−0.014, +0.009]: 45 keys dropped, 17 of them declared. BIRD **0.754**, +0.075 [+0.015, +0.155] worse: 123 dropped, 90 of them declared | a filter by exact inclusion costs more true keys than false ones on dirty data; not a rule to ship |
| **Its near-misses promoted by inclusion?** | Spider 0.755, BIRD 0.690: recall +0.059 on each, precision down to 0.636 and 0.576 | inclusion does not stand in for the threshold; with a floor under the score, read on the answers, the best is +0.004 and +0.007, inside the noise |
| **What does reading the rows cost?** | Inclusion into the declared keys: a database's median 1.5 ms on Spider and **0.21 s on BIRD**, 90th percentile 6.8 s, the largest 189 s (157 million rows); with the unique columns and the distributions 0.49 s, 19 s and 676 s. On Windows, indicative. lemmekey: 3.05 ms in the container | on databases of real size the rows cost about 70 times lemmekey at the median and minutes on the largest, and need the rows at all |

## 1. The rows

[rows.csv](data/keys-values-2026-11-26/rows.csv) has a line a database:
tables, empty tables, tables under ten rows, rows, declared keys, those
with a non-empty column, those the data keep and break, the inclusions
found and the times. Summed ([main.json](data/keys-values-2026-11-26/main.json),
`the rows`):

| | Spider | BIRD |
|---|---|---|
| databases declaring a one-column key | 204 | 76 |
| rows; a database's median; a table's median | 1 849 786; 38; 11 | 280 581 646; 90 199; 775 |
| tables; empty; under ten rows | 1 045; 71; 451 | 581; 2; 51 |
| databases with an empty table; with no row at all | 9; 7 | 2; 0 |
| databases under 100 rows, and the declared keys in them | 156; 500 | 2; 3 |
| columns; with no non-null value | 5 247; 370 | 4 171; 36 |
| declared one-column keys | 911 | 606 |
| with a non-empty column | 841 | 601 |
| kept by the data (the inclusion into the declared column holds) | 809 | 496 |
| broken by the data | 30 | 105 |
| with a non-empty column, not found by the file's own pragma | 2 | 0 |
| columns the schema gives as a key (K); the data make one (U) | 794; 2 282 | 462; 782 |
| inclusions into K; into U | 3 884; 4 241 | 4 506; 5 136 |
| columns with an inclusion into K | 1 372 | 945 |
| declared keys with an inclusion into their table, K (the ceiling) | 794, 0.872 | 474, 0.782 |
| the same, U | 805, 0.884 | 476, 0.785 |

Spider's reputation for empty tables is nine databases: seven have no row
at all and carry 70 keys between the nine. What Spider is, is tiny: three
databases in four hold under a hundred rows, and those hold 500 of the 911
keys. BIRD is the opposite, and dirty: SQLite does not enforce a foreign
key unless asked, and in 31 of 76 databases some declared key has a value
its referenced column does not hold.

**How broken** ([broken.json](data/keys-values-2026-11-26/broken.json),
read after the scores, `after.py broken`): the share of a broken key's
distinct values that have no match.

| | Spider, 30 keys in 13 databases | BIRD, 105 keys in 31 databases |
|---|---|---|
| under 1 % of the distinct values | 7 | 42 |
| 1 % to under 10 % | 8 | 39 |
| 10 % to under 50 % | 8 | 9 |
| 50 % or more | 7 | 15 |
| of them every value, and the referenced table empty | 7; 5 | 6; 3 |
| median share | 9.7 % | 1.8 % |

Four in five of BIRD's broken keys miss under a tenth of their values. A
method that accepts approximate inclusion, as Zhang et al.'s does with a
threshold the user sets, would have them as candidates; this round's
inclusion is exact, as SPIDER's and Metanome's is under Rostin's and HoPF's
methods, and has none of them.

## 2. The row methods beside lemmekey

On K, the keys the schema gives: a column that is its table's one-column
primary key or declared unique, which is lemmekey's input plus rows.
`ind` draws every column that has an inclusion, to the referenced column of
fewest values. `zhang` chooses the target by the least quantile earth
mover's distance and draws every such column. `hopf` is HoPF's foreign-key
half with the primary keys given: score by histogram and name, no cycle, no
key included in a key by a consecutive run, one target a column, stop when
the schema is connected. `rostin` is the ten features in a decision tree,
Spider's tree fitted on BIRD's 4 506 inclusions and BIRD's on Spider's
3 884 ([score.py](../round-1x/score.py),
[main.json](data/keys-values-2026-11-26/main.json)).

| over all declared keys | Spider, 911 keys: P / R / F1 | macro F1 | lemmekey minus it, F1 | BIRD, 606 keys: P / R / F1 | macro F1 | lemmekey minus it, F1 |
|---|---|---|---|---|---|---|
| **lemmekey**, no row read | 0.901 / 0.869 / **0.885** [0.849, 0.916] | 0.891 | | 0.861 / 0.799 / **0.829** [0.759, 0.890] | 0.784 | |
| `ind` | 0.435 / 0.655 / **0.523** [0.471, 0.577] | 0.578 | +0.362 [+0.297, +0.425] | 0.465 / 0.724 / **0.566** [0.512, 0.637] | 0.599 | +0.263 [+0.194, +0.330] |
| `zhang` | 0.433 / 0.652 / **0.520** [0.467, 0.575] | 0.571 | +0.365 [+0.300, +0.429] | 0.458 / 0.715 / **0.558** [0.500, 0.631] | 0.597 | +0.270 [+0.198, +0.344] |
| `hopf` | 0.805 / 0.755 / **0.779** [0.725, 0.833] | 0.820 | +0.106 [+0.057, +0.160] | 0.708 / 0.672 / **0.689** [0.605, 0.766] | 0.684 | +0.140 [+0.055, +0.246] |
| `hopf, no names` | 0.640 / 0.595 / **0.617** [0.562, 0.667] | 0.699 | +0.268 [+0.209, +0.325] | 0.653 / 0.642 / **0.647** [0.570, 0.730] | 0.671 | +0.182 [+0.090, +0.289] |
| `rostin` | 0.860 / 0.552 / **0.672** [0.622, 0.719] | 0.643 | +0.212 [+0.164, +0.261] | 0.864 / 0.589 / **0.701** [0.609, 0.777] | 0.648 | +0.128 [+0.017, +0.255] |
| `rostin, no names` | 0.674 / 0.419 / **0.517** [0.455, 0.572] | 0.487 | +0.368 [+0.304, +0.439] | 0.828 / 0.462 / **0.593** [0.506, 0.693] | 0.559 | +0.236 [+0.099, +0.352] |

Every interval of a difference is above zero: on both collections lemmekey
reads more keys right than any row method run here. Against HoPF the
distance is in both halves, precision +0.096 and recall +0.114 [+0.066,
+0.164] on Spider, +0.153 and +0.127 [+0.017, +0.276] on BIRD. Against
Rostin's tree it is recall alone, +0.317 and +0.210, the precisions within
0.05: the tree draws 585 and 413 keys where lemmekey draws 879 and 562.

**Over the declared keys with a non-empty column**, a prediction for an
empty column counted neither way:

| | Spider, 841 keys: P / R / F1 | macro F1 | lemmekey minus it, F1 | BIRD, 601 keys: P / R / F1 | macro F1 | lemmekey minus it, F1 |
|---|---|---|---|---|---|---|
| **lemmekey** | 0.902 / 0.878 / **0.890** [0.852, 0.921] | 0.895 | | 0.862 / 0.799 / **0.829** [0.761, 0.890] | 0.784 | |
| `ind` | 0.435 / 0.710 / **0.540** [0.484, 0.591] | 0.599 | +0.350 [+0.284, +0.418] | 0.465 / 0.730 / **0.568** [0.513, 0.638] | 0.599 | +0.261 [+0.194, +0.328] |
| `zhang` | 0.433 / 0.706 / **0.537** [0.478, 0.590] | 0.592 | +0.353 [+0.286, +0.420] | 0.458 / 0.720 / **0.560** [0.501, 0.633] | 0.597 | +0.269 [+0.197, +0.343] |
| `hopf` | 0.805 / 0.818 / **0.811** [0.756, 0.860] | 0.849 | +0.078 [+0.034, +0.128] | 0.708 / 0.677 / **0.692** [0.607, 0.769] | 0.685 | +0.137 [+0.054, +0.245] |
| `hopf, no names` | 0.640 / 0.644 / **0.642** [0.594, 0.692] | 0.724 | +0.248 [+0.189, +0.304] | 0.653 / 0.647 / **0.650** [0.572, 0.732] | 0.672 | +0.179 [+0.090, +0.288] |
| `rostin` | 0.860 / 0.598 / **0.705** [0.664, 0.743] | 0.666 | +0.184 [+0.143, +0.229] | 0.864 / 0.594 / **0.704** [0.612, 0.781] | 0.649 | +0.125 [+0.014, +0.256] |
| `rostin, no names` | 0.674 / 0.454 / **0.543** [0.485, 0.596] | 0.506 | +0.347 [+0.281, +0.415] | 0.828 / 0.466 / **0.596** [0.510, 0.695] | 0.560 | +0.233 [+0.098, +0.350] |

Leaving out the 70 keys nothing in the rows can show takes 0.03 from
lemmekey's lead over HoPF on Spider, +0.078 [+0.034, +0.128], and leaves
BIRD where it was.

**On U**, every column the data make unique taken for a key, as the methods
run when no key is declared:

| over all declared keys | Spider: P / R / F1 | macro F1 | lemmekey minus it, F1 | BIRD: P / R / F1 | macro F1 | lemmekey minus it, F1 |
|---|---|---|---|---|---|---|
| `ind` | 0.421 / 0.658 / 0.513 [0.465, 0.564] | 0.566 | +0.372 [+0.308, +0.434] | 0.439 / 0.701 / 0.540 [0.481, 0.623] | 0.594 | +0.289 [+0.204, +0.357] |
| `zhang` | 0.419 / 0.654 / 0.510 [0.458, 0.562] | 0.559 | +0.374 [+0.312, +0.439] | 0.433 / 0.691 / 0.532 [0.470, 0.616] | 0.592 | +0.296 [+0.208, +0.372] |
| `hopf` | 0.792 / 0.763 / 0.777 [0.725, 0.828] | 0.821 | +0.108 [+0.060, +0.161] | 0.707 / 0.672 / 0.689 [0.602, 0.769] | 0.690 | +0.140 [+0.054, +0.247] |
| `hopf, no names` | 0.632 / 0.600 / 0.616 [0.566, 0.663] | 0.697 | +0.269 [+0.210, +0.326] | 0.619 / 0.617 / 0.618 [0.534, 0.726] | 0.674 | +0.211 [+0.097, +0.313] |
| `rostin` | 0.816 / 0.604 / 0.694 [0.652, 0.733] | 0.662 | +0.191 [+0.150, +0.233] | 0.785 / 0.500 / 0.611 [0.534, 0.685] | 0.580 | +0.218 [+0.106, +0.325] |
| `rostin, no names` | 0.667 / 0.415 / 0.512 [0.454, 0.564] | 0.488 | +0.373 [+0.312, +0.443] | 0.700 / 0.447 / 0.546 [0.476, 0.630] | 0.545 | +0.283 [+0.190, +0.372] |

U adds 11 and 2 declared keys to the ceiling and 357 and 630 inclusions;
nothing moves by more than its interval but Rostin's tree on BIRD, 0.701 to
0.611.

**Zhang et al.'s top X %** ([curve.json](data/keys-values-2026-11-26/curve.json)).
Their paper reports the first X % of the pairs in increasing distance and
names no X. On K, a database at a time:

| X | 10 | 20 | 30 | 40 | 50 | 60 | 70 | 80 | 90 | 100 |
|---|---|---|---|---|---|---|---|---|---|---|
| Spider, F1 | 0.220 | 0.274 | 0.363 | 0.423 | 0.446 | 0.496 | 0.517 | **0.526** | 0.526 | 0.520 |
| Spider, precision | 0.510 | 0.489 | 0.515 | 0.518 | 0.499 | 0.498 | 0.480 | 0.466 | 0.446 | 0.433 |
| BIRD, F1 | 0.251 | 0.386 | 0.502 | 0.562 | 0.588 | **0.613** | 0.606 | 0.591 | 0.574 | 0.558 |
| BIRD, precision | 0.719 | 0.733 | 0.733 | 0.699 | 0.658 | 0.620 | 0.568 | 0.524 | 0.485 | 0.458 |

The best X, chosen on the answers as no user can, gives 0.526 and 0.613. On
BIRD the distance orders the candidates (precision 0.72 in the first tenth
against 0.46 over all); on Spider it does not (0.51 against 0.43), which is
what a referenced column of eleven values leaves of a distribution.

**By the rows of a database**, read after the scores
([sizes.json](data/keys-values-2026-11-26/sizes.json)), F1 on K:

| | databases | keys | lemmekey | `ind` | `hopf` | `rostin` |
|---|---|---|---|---|---|---|
| Spider, under 100 rows | 156 | 500 | 0.904 | 0.572 | 0.811 | 0.655 |
| Spider, 100 to 9 999 | 37 | 304 | 0.882 | 0.469 | 0.825 | 0.699 |
| Spider, 10 000 or more | 11 | 107 | 0.798 | 0.460 | 0.545 | 0.678 |
| BIRD, 100 to 9 999 | 18 | 83 | 0.843 | 0.761 | 0.848 | 0.816 |
| BIRD, 10 000 or more | 56 | 520 | 0.826 | 0.537 | 0.663 | 0.679 |

Plain inclusion's precision does not rise with the rows: 0.482, 0.388 and
0.367 on Spider's three bands, 0.438 on BIRD's largest. Accidental
inclusion is not a matter of tiny tables. A hypothesis, not separated from
others here: it is small integers, a count or a code inside the range of
some table's serial key, and a larger table has a longer serial key to fall
into. On BIRD's 18 databases of 100 to 9 999 rows HoPF is level with
lemmekey, 0.848 against 0.843 over 83 keys; that is the one band where a
row method reaches it.

## 3. What reading rows adds to lemmekey

Two rules written before any was scored, on lemmekey's kept scores, no
refit ([main.json](data/keys-values-2026-11-26/main.json)). **These are
figures on development data, and exploratory.**

What the data say of the keys lemmekey draws: on Spider, of 879, 773 hold,
61 are on an empty column and 45 fail; on BIRD, of 562, 434 hold, 5 are
empty and 123 fail.

| over all declared keys | Spider: P / R / F1 | macro F1 | lemmekey minus it, F1 | BIRD: P / R / F1 | macro F1 | lemmekey minus it, F1 |
|---|---|---|---|---|---|---|
| lemmekey | 0.901 / 0.869 / 0.885 [0.849, 0.916] | 0.891 | | 0.861 / 0.799 / 0.829 [0.759, 0.890] | 0.784 | |
| **filtered**: a drawn key dropped when its column is not included in the target's key | 0.929 / 0.851 / 0.888 [0.854, 0.918] | 0.883 | −0.003 [−0.014, +0.009] | 0.897 / 0.650 / 0.754 [0.675, 0.825] | 0.687 | **+0.075 [+0.015, +0.155]** |
| **promoted**: a column with no key takes its best candidate when the inclusion holds | 0.636 / 0.929 / 0.755 [0.729, 0.782] | 0.754 | **+0.130 [+0.102, +0.159]** | 0.576 / 0.858 / 0.690 [0.581, 0.786] | 0.710 | **+0.139 [+0.081, +0.196]** |
| both | 0.645 / 0.910 / 0.755 [0.728, 0.780] | 0.749 | +0.130 [+0.100, +0.161] | 0.552 / 0.710 / 0.621 [0.524, 0.713] | 0.622 | +0.208 [+0.130, +0.286] |

Neither rule helps, and each for a reason the rows of §1 give.

**Filtered.** On Spider the filter drops 45 keys, 28 of them false and 17
declared: precision +0.028, recall −0.019 [−0.039, −0.005], F1 the same. On
BIRD it drops 123, 33 false and 90 declared, because BIRD's data break 105
of its declared keys: recall falls from 0.799 to 0.650 and F1 by 0.075.
The declaration is the label here, and the data are not held to it.

**Promoted.** The rule adds 451 keys on Spider, 54 of them declared, and
340 on BIRD, 36 declared: recall +0.059 on each and precision 0.636 and
0.576. A column's best candidate with a score near zero, included in its
target's key, is mostly the accident of §2.

With a floor under the candidate's score, **read on the answers after the
scores** ([floor.json](data/keys-values-2026-11-26/floor.json)):

| the floor | 0 (the rule) | 0.05 | 0.1 | 0.2 | 0.3 | 0.4 | 0.5 | 0.6 | 0.7 | lemmekey, threshold 0.798 |
|---|---|---|---|---|---|---|---|---|---|---|
| Spider, F1 | 0.755 | 0.855 | 0.878 | 0.882 | 0.886 | 0.888 | 0.888 | **0.889** | 0.887 | 0.885 |
| BIRD, F1 | 0.690 | 0.763 | 0.783 | 0.804 | 0.816 | 0.834 | **0.835** | 0.830 | 0.828 | 0.829 |

The best floor gives +0.004 on Spider (lemmekey minus it −0.004 [−0.013,
+0.003]) and +0.007 on BIRD (−0.007 [−0.018, +0.004]); both intervals hold
zero. Round 1w's bound for the best single threshold of each set, no row
read, was 0.889 and 0.829: on Spider a lower threshold alone reaches what
the floor reaches, and on BIRD the floor's 0.835 is 0.006 above it. On
these two sets exact inclusion buys lemmekey nothing that can be told from
noise.

## 4. The cost

One pass a database, one process and one thread, Python's `sqlite3`, the
files opened immutable, **on Windows on the bare machine and not in the
Linux container**, while other rounds of this repository worked on the same
machine. The times are indicative. lemmekey's 3.05 ms is round 1w's, a
compiled reader on one processor in the container, over 398 databases; the
two are not on the same terms and are put side by side for the order of
magnitude only ([cost.json](data/keys-values-2026-11-26/cost.json),
[rows.csv](data/keys-values-2026-11-26/rows.csv)).

| a database | Spider, 204: median; 90th percentile; largest; sum | BIRD, 76: median; 90th percentile; largest; sum |
|---|---|---|
| rows read at least once | 38; 231; 553 693; 1.85 million | 90 199; 4.74 million; 157 million; 281 million |
| values (rows times columns) | 186; 1 251; 10.4 million; 24.1 million | 0.80 million; 21.6 million; 497 million; 1 417 million |
| file | 28 KiB; 80 KiB; 302 MiB; 469 MiB | 11.0 MiB; 579 MiB; 4.19 GiB; 21.1 GiB |
| `ind` on K: the counts and every inclusion into a declared key | 1.5 ms; 6.7 ms; 5.5 s; 9.2 s | **0.21 s; 6.8 s; 189 s; 477 s** |
| and the unique columns and the inclusions into U | 2.7 ms; 17 ms; 5.5 s; 13.6 s | 0.35 s; 8.1 s; 280 s; 681 s |
| and the values for `zhang`, `hopf` and `rostin` | 3.6 ms; 25 ms; 11.4 s; 21.8 s | 0.49 s; 19.3 s; 676 s; 1 372 s |
| the two hybrid rules' tests beyond those | 0.005 ms; 0.04 ms; 0.09 s; 0.12 s | 0.03 ms; 0.05 s; 5.1 s; 6.1 s |
| lemmekey (round 1w, in the container), over Spider, BIRD and CTU together, 398 databases | 3.05 ms; 31 to 33 ms; 0.43 to 0.46 s at the 99th percentile | |

102 412 inclusion tests were run and 65 407 pairs were settled by the
columns' ranges without one. The hybrid rules' own line is what they cost
past the tests already run for K and U in the same pass; alone they need
the first line's scan of every table as well. BIRD's files not unpacked
before were written to disk just before they were read, so their first
phase is not a cold read.

On Spider, where a database is 38 rows, plain inclusion costs what lemmekey
costs. On BIRD, where a database is ninety thousand rows, its median is
about 70 times lemmekey's, its 90th percentile is seconds, and one database
of 157 million rows takes three minutes for inclusion alone and eleven with
the distributions. The cost that does not show in the table is the first
one: the method needs the rows, read access to every table, where lemmekey
needs the schema.

## What this does not show

- **These are our implementations, from the papers' texts.** No public
  implementation of any of the three foreign-key decisions was found in a
  short search. Where a paper leaves a choice open the specification says
  which was taken: HoPF's token frequency and its buckets for strings,
  Rostin's name feature, scikit-learn's tree for WEKA's J48, the tie rule.
  A figure of an author's own code could differ.
- **HoPF was run with the primary keys given**, not discovered, which is
  the paper's easier setting, and without its primary-key reduction.
  **Fast-FK** (Chen et al., 2014) was not run: its paper was not read for
  this round and its scoring is hand-weighted inside a product.
- **Inclusion is exact.** Zhang et al. accept approximate inclusion at a
  threshold the user sets. With BIRD's data, 81 declared keys that miss
  under a tenth of their values would become candidates, lifting the
  ceiling from 0.782 towards 0.916, at the price of more accidental
  candidates; how a row method does with it was not measured.
- **Rostin's tree was fitted on the other collection only**, 3 884 or
  4 506 inclusions; its authors add a sample of the judged database's own
  inclusions and go from 0.78 to 0.99 on theirs. That was not done.
- **Development data on one side.** lemmekey was shaped on sets that
  include Spider and BIRD; the row methods saw them once. The paper's clean
  sets (plain A, style 3) are schemas from GitHub with no rows, so no row
  method can be run where lemmekey's clean figures are.
- **CTU was not measured.** Rows of 48 of its 142 databases lie here as
  MySQL dumps; the rest are on a remote server. Its relational-learning
  sets, whose keys are named for roles and which lemmekey reads worst
  (0.664), are where rows might do best.
- **Labels are declarations.** A true link nobody declared counts against
  every method; an inclusion into the right table that the schema does not
  declare is counted as an accident.
- **The hybrid rules are two rules of many**, on development data, and the
  floor was read on the answers. They say that exact inclusion is no free
  gain; they do not say that no use of rows helps. A learned use (inclusion
  and coverage as features of the second stage) was not tried.
- **The timings are one pass on Windows** beside other work, through
  Python; a compiled SPIDER or BINDER would be faster by a factor not
  measured here, and still reads every row.
- **Values were compared as SQLite compares them in a join.** Two keys of
  BIRD are declared from a column to itself and no inclusion can give them.
  Four of Spider's declared keys were not found by the file's own pragma
  under the kept schema's names; two of them have a non-empty column and
  are counted as neither kept nor broken in §1.
- **Nothing here is committed**: the round's code ran from the working tree
  on `9043b62`.

## What follows

- **For the paper, §5.** On Spider and BIRD the strongest value-based
  method run, HoPF's foreign-key step over exact inclusion with the keys
  given, reads 0.779 and 0.689 of F1 where lemmekey, reading no row, reads
  0.885 and 0.829; plain inclusion reads 0.523 and 0.566 at a precision
  under one half. The claim holds for these two collections, which are
  development data for lemmekey, and for our implementations.
- **For §6.3**, in place of the open mark: what was run, that the
  implementations are ours, that inclusion was exact while BIRD's data
  break 105 of its 601 non-empty declared keys, and that CTU and Fast-FK
  were not measured.
- **For `see`**: a filter by inclusion is not worth shipping as written;
  it would drop a sixth of the true keys on data like BIRD's. If rows are
  ever used, they should enter as evidence with a tolerance, not as a veto.
- **A round worth running next**, if the paper's reviewers ask: the same
  methods with approximate inclusion at a published threshold, and CTU's 48
  dumped databases loaded into SQLite.

## Environment and reproduction

| | |
|---|---|
| machine | AMD Ryzen 7 7700, 31 GB, Windows 11 Pro 26300; not in the Linux container; [environment.txt](data/keys-values-2026-11-26/environment.txt) |
| Python | 3.13.15; SQLite 3.50.4 through `sqlite3`; numpy 2.5.4; scikit-learn 1.9.1 |
| lemmekey | `eleven--wide` as round 1v froze it, checked against `corpus/round-1v/frozen.json`; threshold 0.798; its kept scores of `corpus/round-1v/` |
| Spider | `spider_data.zip`, SHA-256 `00636695dabed6b5f4b8328a16b13e069a2f16591d5efcce57660669c85b121b`; `test_database/`, the 204 databases that declare a one-column key |
| BIRD | round 1f's `train_databases.zip` and `dev_databases.zip` in `corpus/bird/raw/`; `train.zip`'s SHA-256 `66e9e3115b59559554013aa3b124156249f30437a6b4e4f96de3d2dfb5ae8cbc`; the 76 of 80 databases that declare a one-column key |
| the papers | Rostin et al., WebDB 2009; Zhang et al., PVLDB 3(1), 2010; Jiang and Naumann, JIIS 54(3), 2020; read on 26 November 2026 |
| research commit | `9043b62` with the round's files in the working tree, uncommitted |
| the runs, UTC, 26 November | specification 15:05; `rows.py profile` 15:06 to 15:31, Spider then BIRD, each by file size; `score.py` 15:31 to 15:33; `after.py` 15:34 to 15:38 |
| seeds | 20261101 for the resamples; 0 for the tree |
| the round's agent | Claude Fable 5.2, which wrote the code and this report |

```sh
cd <repo>/lemmesee
P=round-1b/.venv/Scripts/python
$P -I round-1x/rows.py profile <corpus>/round-1x
$P -I round-1x/score.py <corpus>/round-1x reports/data/keys-values-2026-11-26
$P -I round-1x/after.py sizes <corpus>/round-1x reports/data/keys-values-2026-11-26
$P -I round-1x/after.py broken <corpus>/round-1x reports/data/keys-values-2026-11-26
```

`<corpus>` is `<repo>/lemmesee/corpus`, where the earlier rounds fetched
Spider and BIRD and kept lemmekey's scores.
