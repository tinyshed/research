# Round 1p: keys on the databases others measured theirs on

**Status: specified and run on 1 November 2026, [reported](../reports/keys-external-2026-11-01.md).** A
round of [the plan](../plan.md) that needs no judges: round 1b's key model,
[as reported](../reports/keys-2026-10-28.md), on the databases the SchemaPile
paper tested its foreign-key models on, beside that paper's own T5 model run
here on the same pairs.

## The question

Round 1b measured its model on SchemaPile-Perm's own test split. A reader
will ask how it does on databases nobody wrote for that corpus, and against
the models that corpus was published with. SchemaPile's paper (Döhmen et al.,
SIGMOD 2024, §4.1) fine-tuned T5-base and StarCoder 15.5B to name the foreign
key between two tables and measured them on Spider, BIRD and CTU's relational
repository: recall at one 0.92, 0.83, 0.91 for T5 and 0.97, 0.94, 0.97 for
StarCoder, as its notebook's last cell has them.

1. **In the paper's task** — two tables known to hold one key, which columns
   — where does a model of under a megabyte stand?
2. **In round 1b's task** — every column of a database, no table named — how
   do it and the rules do on these databases?
3. **Where does it lose**, and does widening what it may pair recover it?

## The data

| | |
|---|---|
| Spider | `spider_data.zip` from Spider's [own link](https://yale-lily.github.io/spider), SHA-256 `00636695…121b`, the same bytes as the Hugging Face mirror `HAL-9001/spider-databases`; its `test_database/`, 206 SQLite files, a superset of `database/`'s 166 |
| BIRD | the train (69) and dev (11) SQLite files of round 1f's archives in `corpus/bird/raw/`; each schema read from the file's first 32 MiB inside the archive, or the whole file when that fails (30 of 80); on the 42 already unpacked the two readings are identical |
| CTU | the 48 MySQL dumps already in `corpus/ctu/`, pinned in `round-1/fixtures/hashes.txt`, at first; then all 142 databases of the repository, dumped as structure only (see below) |
| T5 | `tdoehmen/t5-schemapile-fk` at revision `cccb88f`, hashes in `corpus/round-1p/t5-schemapile-fk.sha256` |

**What the model sees** is round 1b's hidden schema: names, types,
nullability, single-column uniqueness and primary keys; no index (MySQL
indexes every foreign key, and round 1b's hard cases hid them too), no
declaration. SQLite's `text` is read as `varchar`, since SQLite has no other
string; an `enum` as `varchar`, as round 1b hides it.

**The paper's pairs** are built as its notebook `eval_data_prep.ipynb`
builds them, its quirks kept: a pair a declared key; a key over several
columns dropped by its second row and brought back by a third; two keys
between the same tables both dropped; a pair dropped when both its tables,
as sorted lower-cased name strings, occur together in one schema of the
training corpus, and when the two tables' text passes 8,000 characters. The
paper filtered against all of SchemaPile and its instruction data; we have
SchemaPile-Perm, which round 1b trained on, so our sets are larger: 768
Spider pairs against its 437, 381 BIRD against 242, and 1 267 CTU against
558 (the paper's sets after its own filter).

## The contestants

| | |
|---|---|
| **round 1b's model** | its two saved stages and threshold at research commit `1524905`, kept in `corpus/round-1p/models-1524905/` |
| **the name score** | round 1b's `v2 base`, the best of exact, letter and prefix matches between a column and a table, alone |
| **the rules** | round 1's `roles.rs` through `guess`, round 1b's task only |
| **T5, constrained** | every (column, referenced column) of the two tables scored by the model's log-likelihood of writing it; the best is its answer. The paper constrained StarCoder this way; it let T5 write freely |
| **T5, free** | greedy decoding, parsed as `'{' + text + '}'` and compared field by field, as the notebook does; the notebook used eight beams |

## The measures

- **The paper's task**: recall at one, a pair right when both the column and
  the referenced column are; a pair no candidate answers is wrong. Our model
  answers inside the two tables only, and inside the whole database; 95 %
  intervals by resampling databases.
- **Round 1b's task**: precision, recall and F1 at round 1b's threshold from
  its validation split, one target a column, against the rules, the
  difference's 95 % interval by resampling databases; on all databases and
  on the novel ones, sharing under half their table names with any training
  schema.

## The widened candidates

Written after the failure analysis of the reported model on these databases,
and decided on SchemaPile-Perm's validation split only:

- **own key**: a table's one-column primary key may point at another table;
- **pseudo key**: a table declaring no primary key is a target through its one
  unique column, or a column named `id` or after the table and `id`.

Four variants — neither, each, both — are fitted as graph.py's `final` fits
round 1b's model and measured on validation: F1 one target a column, and the
paper's task built on validation schemas, the column asked. **A variant is
chosen** if its validation F1 is no more than 0.005 below the reported
variant's and its validation recall at one is higher; among several, the
highest recall at one. The chosen one is then measured once on the test
split, the hard cases and these databases. This rule was written before
any variant was fitted.

**Its F1, read again after two variants** (reported, own key) and before the
other two: F1 over a variant's own candidates is not comparable between
variants, since a wider variant puts more of the declared keys — the harder
ones — among its candidates, and so into its recall's denominator. The rule
is read with F1 over every one-column key the validation schemas declare,
7 636, the same for all; F1 over candidates is reported beside it.

## Added while the round ran

Each was written after the measures above had been read, and none chose a
model:

- **Gemini 3.8 Flash** (`gemini.py`), through a CLIProxyAPI, in both tasks: the
  paper's own prompt for each pair, and one prompt a database listing its
  schema as the model sees it and asking for every key, scored against every
  one-column key declared.
- **Related or not** (`t5.py negatives`, `separate.py`): for each kept pair a
  table of the same database with no key to or from its first table; how
  sure T5 and our model are of each.
- **Nexus's sets** (`nexus.py`): TPC-H, TPC-DS and BIRD's development databases.
- **The file** (`pack.py`): both stages packed as round 1b sized them and read
  back.
- **Size against quality** (`curve.py`) and **the graph stage** (`ablate.py`)
  for the chosen variant.
- **All of CTU.** CTU's server refused connections when the round began, so
  it started on the 48 databases earlier rounds had dumped; reached later
  through a tunnel, the repository's 142 were dumped as structure only into
  `corpus/ctu-schema/`, the 48 parse identically both ways, and every measure
  was run again on the 142.

## Run

```sh
cd <repo>/lemmesee
round-1b/.venv/Scripts/python -I round-1p/external.py prepare corpus corpus/round-1p
round-1/harness/target/release/guess corpus/round-1p/snapshots.jsonl corpus/round-1p/guesses.jsonl
round-1b/.venv/Scripts/python -I round-1p/external.py score corpus/round-1p/models-1524905 corpus/round-1p
<python with torch> -I round-1p/t5.py corpus/round-1p/t5-schemapile-fk corpus/round-1p
round-1b/.venv/Scripts/python -I round-1p/widen.py dev corpus/round-1b corpus/round-1p/variants
round-1b/.venv/Scripts/python -I round-1p/widen.py final corpus/round-1b corpus/round-1p/variants both
round-1b/.venv/Scripts/python -I round-1p/widen.py external corpus/round-1p/variants both corpus/round-1p
<python with torch> -I round-1p/t5.py negatives corpus/round-1p/t5-schemapile-fk corpus/round-1p
round-1b/.venv/Scripts/python -I round-1p/separate.py corpus/round-1p/models-1524905 corpus/round-1p
round-1b/.venv/Scripts/python -I round-1p/separate.py corpus/round-1p/variants/both corpus/round-1p both
round-1b/.venv/Scripts/python -I round-1p/gemini.py cut corpus/round-1p
CLIPROXY_KEY=... round-1b/.venv/Scripts/python -I round-1p/gemini.py ask corpus/round-1p <endpoint>/v1 gemini-3.8-flash-high 8
round-1b/.venv/Scripts/python -I round-1p/gemini.py measure corpus/round-1p
round-1b/.venv/Scripts/python -I round-1p/gemini.py measure corpus/round-1p -both
round-1b/.venv/Scripts/python -I round-1p/nexus.py corpus/round-1p/variants/both corpus/round-1p corpus both
round-1b/.venv/Scripts/python -I round-1p/ablate.py corpus/round-1b corpus/round-1p/variants/both corpus/round-1p both
round-1b/.venv/Scripts/python -I round-1p/pack.py corpus/round-1p/variants/both corpus/round-1p both
round-1b/.venv/Scripts/python -I round-1p/curve.py corpus/round-1b corpus/round-1p/variants both corpus/round-1p
```

CTU's structure, through any route to relational.fel.cvut.cz:3306 (its
public `guest` account, as `round-1/fixtures/ctu.sh` uses it):
`mysqldump --no-data --column-statistics=0 --single-transaction --skip-lock-tables --no-tablespaces --skip-dump-date <database>`
for each database its `information_schema` lists, into `corpus/ctu-schema/`,
checked against `ctu-schema.sha256` in the report's data.

`t5.py` needs torch and the `tokenizers` package and nothing else; it was
run with torch 2.11.0+cu128.
