# Round 1x: what reading the rows finds on Spider and BIRD

**Status: specified and run on 26 November 2026,
[reported](../reports/keys-values-2026-11-26.md).**

Written at 15:05 UTC on 26 November 2026, before any row of any database
was read by this round and before anything here was scored.

The paper's key model, `eleven--wide` (lemmekey there), reads a schema's
names, types and primary keys and no row
([keys-paper-model-2026-11-14.md](../reports/keys-paper-model-2026-11-14.md)).
Every method it is compared with in the literature reads values, and the
paper's draft says in §6.3 that no value-based baseline was run although
Spider's and BIRD's rows are at hand. This round runs one.

## The question

1. What does a method that reads the rows find on the databases the paper
   reports, in the paper's task and by the paper's measure?
2. What are the rows like: how much of Spider and BIRD is empty or tiny,
   where inclusion says nothing or holds by accident?
3. What would reading rows add to lemmekey, its keys filtered and its
   near-misses promoted by inclusion?
4. What does reading the rows cost beside lemmekey's 3 ms a database?

**No model is fitted on lemmekey's side and none is chosen.** lemmekey is
read from the scores round 1v kept, through round 1w's `kept.py`, its three
files checked against `corpus/round-1v/frozen.json` first. Before anything
new is scored it must read 0.885 on Spider and 0.829 on BIRD, round 1w's
figures.

## The data

| | |
|---|---|
| Spider | the 204 databases of `spider_data.zip`'s `test_database/` that declare a one-column key, 911 keys; the SQLite files round 1p unpacked into `corpus/spider/` |
| BIRD | the 76 of its 80 train and dev databases that declare one, 606 keys; the SQLite files of round 1f's archives in `corpus/bird/raw/`, the ones not yet unpacked extracted one at a time, measured and deleted |
| CTU | **not measured.** Its rows are on a remote MariaDB server; 48 of its 142 databases lie in `corpus/ctu/` as MySQL dumps with rows, the other 94 as structure only. A third of a collection loaded into another engine is not the collection the paper reports, so this round leaves it out and says so |

The schema of a database, its declared keys and its identity are those of
round 1v's kept sets (`corpus/round-1v/sets/spider.pkl`, `bird.pkl`), which
every figure of the paper's model was scored against. `rows.py` stops when a
file's tables and columns are not the kept schema's.

## What a method is shown

The hidden schema lemmekey is shown (names, types, nullability, one-column
uniqueness, primary keys; no foreign key, no index) **and every row**. The
foreign-key declarations are read from the file only as labels and to say
whether the data keep a declared key.

## Inclusion

A unary inclusion dependency *A ⊆ B* holds when column *A* has at least one
non-null value and every non-null value of *A* occurs in column *B*, *A* and
*B* different columns of the same database (the same table allowed). Values
are compared as SQLite compares them in a join, with the columns'
affinities: the test is

```sql
SELECT 1 FROM "T" WHERE "A" IS NOT NULL
  AND "A" NOT IN (SELECT "B" FROM "R" WHERE "B" IS NOT NULL) LIMIT 1
```

and holds when it returns no row. It is exact; no sketch, no sample. A
referenced column with no index of its own, in a table of more than a
thousand rows, is copied to a temporary table with one. A pair is skipped without the test only when the two columns'
minima and maxima are of the same storage class (and, for strings, neither
table declares a collation) and *A*'s range is not inside *B*'s, which
cannot change an answer. An **empty** dependent column
(no non-null value) has no inclusion: every method below predicts nothing
for it, as HoPF's authors filter it (their §5.1) and as inclusion would
otherwise hold into everything.

Two sets of referenced columns:

- **K, the keys the schema gives**: a column that is its table's one-column
  primary key or is declared unique. This is the paper's input plus rows,
  and the main reading.
- **U, the keys the data give**: any column with at least one row, no null
  and no repeated value, as SPIDER's users and HoPF take them when no key is
  declared (HoPF's definition of a unary unique column combination). U
  need not contain K: a declared key of an empty table is in K only.

Every method is run on K and on U. A method's answer for a column is one
referenced **table**, as the paper's task has it.

No public implementation of the foreign-key decision of any of the four
methods was found in a short web search (three queries) (Metanome publishes SPIDER
and BINDER, which find the inclusions and no keys; HoPF's paper names no
repository). Inclusion found exactly is the same set whichever exact
algorithm finds it, so the inclusions are found by the test above and the
decisions are written here, from the papers' texts, read on 26 November
2026: Rostin et al., WebDB 2009; Zhang et al., PVLDB 3(1), 2010; Jiang and
Naumann, JIIS 54(3), 2020.

## The methods

Each is fixed here. Nothing is chosen on Spider or BIRD: where a method has
a constant its paper's value is taken, and where a paper leaves a choice
open the reading taken is written down below as ours.

**(a) Plain inclusion, `ind`.** Every column with an inclusion into a
referenced column of the set is a predicted key. One target a column, the
tie rule: the referenced column with the fewest distinct values (the
highest coverage, Rostin's F2), then the first in the schema's order of
tables and columns. With it is reported the **ceiling** of the candidates:
the declared keys for which the inclusion into the declared table holds at
all, the recall no rule choosing among these inclusions can pass.

**(b1) Randomness, `zhang`** (Zhang et al., §3). For an inclusion *F ⊆ P*:
the distinct values of *P* in order (numbers numerically before strings
lexicographically) cut into λ = 256 equal-depth cells, or as many as *P* has
values when fewer; *P*'s histogram is the share of its values in each cell
and *F*'s the share of *F*'s **distinct** values in each cell (their
Definition 4); the distance between cells *i* and *j* is |*i* − *j*| / λ
(the quantile distance), and the score is the earth mover's distance
between the two histograms, in one dimension the mean absolute difference
of their cumulative sums. Exact quantiles, as their §5.1 settles on. The
paper outputs the pairs in increasing distance and reports the top X %; it
prescribes no cut. Reported here: one target a column, the least distance
(ties as in (a)), every such column drawn (`zhang`, X = 100 %); and the
curve of precision, recall and F1 over the top X % of those columns by
distance, X = 10, 20, …, 100, with the best X marked as chosen on the
answers, which no user has. Name-free. Not implemented: their bottom-k
estimate of approximate inclusion (inclusion here is exact) and multi-column
keys (not in the task).

**(b2) HoPF's foreign-key half, `hopf`** (Jiang and Naumann, §4.2, §5.1,
§5.2, §5.4, Algorithm 3), with the primary keys given, not discovered: on
K the declared keys, on U every unique column. Candidates: the inclusions
whose dependent column is not empty. A candidate's score is the mean of
- *data distribution*: twenty buckets over the referenced column's value
  range, the referenced column's values and the dependent column's values
  (every row's, not the distinct ones) put into them, and the Bhattacharyya
  coefficient of the two normalised histograms. For numbers the buckets are
  of equal width between the minimum and the maximum; for a referenced
  column holding any string the paper does not say, and ours are twenty
  equal ranges of the rank among the referenced column's sorted values;
- *column name*: the fuzzy similarity of Chaudhuri et al. as the paper
  gives it: table and column name of each side split into lower-cased
  tokens at underscores, spaces, digits' and capitals' boundaries; each
  token of the referenced side matched to its most similar unmatched token
  of the dependent side by normalised Levenshtein similarity (nothing left
  to match scores zero); the mean of those weighted by ln(1 / *f*), *f* the
  share of the schema's columns whose table-and-column tokens hold the
  token (the paper does not say what *f* is counted over; this is ours; a
  token in every column weighs nothing, and when every weight is zero the
  mean is unweighted).

Candidates are taken in falling score (ties as in (a)). One is dropped when
it would close a directed cycle of tables, a table's reference to itself
counted as one; or when its dependent column is its table's one-column
primary key and its sorted distinct values are consecutive among the
referenced column's sorted values (the PK ⊆ PK filter). One taken removes
every other candidate of the same dependent column. The algorithm stops
when the tables of the schema are connected, or no candidate is left.
`hopf` uses both features, as published; `hopf, no names` the distribution
alone, as the authors ran their primary-key side. Not implemented: the
discovery of primary keys from unique column combinations, its cliff
pruning and the primary-key reduction, since the task gives the keys (K) or
takes every unique column for one (U); multi-column keys.

**(b3) The learned classifier, `rostin`** (Rostin et al., §3), every
inclusion of the set with its ten features: F1 the dependent column's
distinct values; F2 coverage, dependent over referenced distinct values;
F3, F4, F5 how often the dependent column is referenced, the dependent
column dependent and the referenced column referenced among the set's
inclusions; F6 column name (ours: 2 when the dependent column's name,
lower-cased and stripped to letters and digits, equals the referenced
column's or the referenced table's name followed by it, 1 when one of
those contains the other, 0 otherwise; the paper says exact match or
containment, the referenced table's name considered); F7 the difference of
the mean lengths of the distinct values as strings; F8 the share of the
referenced column's values outside the dependent column's minimum and
maximum; F9 the dependent name ending in `id`, `key` or `nr`; F10 the
dependent table's rows over the referenced table's. Labels: an inclusion
is positive when the schema declares that column's key into that column.
The paper's protocol is to learn on other databases than the one judged:
**the tree for Spider is fitted on BIRD's inclusions and the tree for BIRD
on Spider's.** The classifier stands for the paper's J48 (WEKA's C4.5,
which this round does not run): scikit-learn's `DecisionTreeClassifier`,
`criterion="entropy"`, `min_samples_leaf=2` (J48's default smallest leaf),
`random_state=0`, nothing else set, no pruning searched. A column takes the
positive inclusion of the highest leaf probability, ties as in (a).
`rostin` has the ten features; `rostin, no names` the eight without F6 and
F9.

**Not implemented: Fast-FK** (Chen et al., PVLDB 2014). Its scoring
function is hand-weighted over names and cardinalities inside PowerPivot;
its paper was not read for this round and no implementation is public, so
it is not run and not approximated.

## The measures

Round 1w's, through its code (`kept.against`): a prediction is a (column,
referenced table) pair, right when the schema declares that key; precision,
recall and F1 pooled over every declared one-column key of Spider (911) and
of BIRD (606), macro F1 over databases, 95 % percentile intervals of 1 000
resamples of databases (seed 20261101), a difference from lemmekey
resampling both arms together.

**The rows.** For each database: tables, empty tables, tables under ten
rows, rows; declared keys; declared keys whose dependent column is not
empty; declared keys the data keep (the inclusion into the declared
referenced column holds). Every method and lemmekey are scored twice: over
all declared keys, and over the declared keys with a non-empty dependent
column, where a prediction for an empty column is not counted for or
against (lemmekey may make one; the row methods cannot).

## What reading rows adds to lemmekey

From lemmekey's kept candidates and scores, with no refit, each one rule,
written here before any is scored. A candidate is a column, a target table
and that table's key column as lemmekey's candidates name it; its inclusion
is tested as above, whatever that key column is.

- **Filtered**: a key lemmekey draws is dropped when the data contradict
  it, that is when its column is not empty and is not included in the
  target's key column. Nothing else changes; the column takes no other
  target.
- **Promoted**: a column for which lemmekey draws no key, and whose
  best-scored candidate is under the threshold, takes that candidate when
  its inclusion holds (the column not empty). Only the best-scored
  candidate is looked at, at any score.
- **Both** rules together.

These are figures on development data, of rules written after the model was
seen on these sets many times, and exploratory: they say what there is to
gain, not what a shipped rule would read on a new database.

## The cost

For each database, wall time of one pass and the rows it holds (every row
is read at least once, a table's scan for its counts, minima and maxima):
the statistics and the inclusions into K (what `ind` needs); the search for
unique columns and the inclusions into U; the values fetched for the
distributions and features; the tests the two hybrid rules need. One
process, one thread, Python's `sqlite3`, files opened immutable, **on
Windows on the bare machine, not in the Linux container**: the databases are
32 GB on a Windows disk, which a container reads through a bind mount at a
fraction of the disk's speed. The times are indicative and are not on the
terms of lemmekey's 3.05 ms (one processor, in the container, a compiled
reader); the report says so wherever it puts them side by side. A database
is read once, so the first phase carries the cold read of its file.

## What counts as an answer

There is no bar to pass. The round reports what the row methods read; the
paper takes the figures as they come. Two checks say the code reads right:
lemmekey through `kept.against` reads 0.885 and 0.829; and every declared
key the data keep, whose referenced column is in the set, is among that
set's inclusions.

## Made exact before the first score

Written at 15:08 UTC on 26 November, while the rows were being read and
before `score.py` had run once; none of it was chosen on a figure.

- The inclusion test is written with `NOT IN`, as above, not with
  `NOT EXISTS` as the first draft of this file had it: the answer is the
  same and SQLite never falls back to a scan of the referenced table for
  every row.
- Zhang et al.'s top X % is taken a database at a time, as their paper
  reports it: the first ⌈X % · n⌉ of a database's *n* columns that have an
  inclusion.
- HoPF's PK ⊆ PK filter reads "primary key" as the declared one-column
  primary key on K and on U alike.
- HoPF's connectivity is checked after each key taken, as its Algorithm 3
  does, so a schema of one table stops after its first key.
- The hybrid rules' tests run inside `rows.py profile`, in the same pass.

## After the scores

Written at 15:34 UTC, once `score.py`'s figures had been seen; `after.py`
holds it and changes no figure above. The row methods stood well below
lemmekey and the filtered rule lost recall on BIRD, so three readings ask
why: for each declared key the data do not keep, the share of its values
with no match (`after.py broken`, which reads the databases again); every
method by the rows of a database; and the promoted rule with a floor under
the candidate's score, read on the answers (`after.py sizes`). The check of
kept keys against K's inclusions was also made to leave out a column
declared to reference itself, which the definition of inclusion excludes
(two keys of BIRD).

## Run

```sh
cd <repo>/lemmesee
P=round-1b/.venv/Scripts/python
$P -I round-1x/rows.py profile corpus/round-1x          # reads every row, the hybrid rules' tests with it
$P -I round-1x/score.py corpus/round-1x reports/data/keys-values-2026-11-26
$P -I round-1x/after.py sizes corpus/round-1x reports/data/keys-values-2026-11-26
$P -I round-1x/after.py broken corpus/round-1x reports/data/keys-values-2026-11-26
```
