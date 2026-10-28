# Round 1c: the analytical question, ordered by what teachers ask

**Status: built and run overnight on 28 October 2026 by the agent, at Dmitry's
request, uncommitted until he has read it; results in
[the report](../reports/questions-2026-10-28.md).** A round of
[the plan](../plan.md) ahead of its order: its configuration C, a small
learned ranker in place of round 1's penalties, over a generator that can
say what round 1's gold questions asked — taught by language models before
round 2's judged set exists, and to be measured on that set when it does.

## Why

Round 1's [gold diagnosis](../round-1/gold/README.md) found two losses. On its
development fixtures the order lost most questions, and the database's
subjects won some back; on three databases it had never seen, **20 of 30
blind gold questions could not be said at all** — a line's revenue is
`unit_price × quantity × (1 − discount)`, a season is an integer `yearID`, a
breakdown is a parent's name three joins away — and one reached the five.
Penalties tuned by looking cannot fix the first, and fixed the second only
on the databases looked at.

## The analytical question

An object between the schema and the chart (`aq.py`):

| Part | What it may be |
|---|---|
| **base** | the table whose rows are aggregated; its grain is the question's grain |
| **measure** | rows counted; a column's sum or average; a row's amount, `price × quantity`, less its discount where the row keeps one, or a line's quantity × its product's price one join away; parents counted once, `count(distinct customer)`; rows with a flag set; the days between two of a row's times, and the rows late by them; how many children each parent has |
| **time** | a creation or event time of the base, or a year or `YYYYMM` held as an integer; where the base has none, its header's — the times of the parents that record events, one step away, else two, never an entity's |
| **breakdown** | a category of the base or of a table reached through foreign keys, many to one, up to three steps, so the grain stays and a sum stays a sum; a parent's name, a person's first and last name, or a parent's key where it has no name — and a table of what happened, with a time of its own or a header's, has none |
| **shape** | over time; by a breakdown, or its twelve largest; over time split by a few values; a distribution |

It compiles to one statement for PostgreSQL or MySQL, joins left so no row
is lost, a parent's name grouped with its key so two of one name stay two.
Every question has an identity — base, measure's aggregate and columns, time
column, breakdown column or the parent it names — by which gold questions,
teachers' questions and round 1's candidates are matched, whatever words
they use.

Keys nobody declared are guessed by name, as ccs's `CustomerID` to
`customers`. Statistics, where read, veto what they rule out: an empty
table, a breakdown of one value or of more than 60, a split of more than
eight, a column nearly all null, the twelve passengers each with one ticket.

## The teachers

Round 1c has no judges yet; it has teachers. **480 schemas from
SchemaPile-Perm** (CC BY 4.0, each under its own permissive licence), chosen
by `teacher.py` to look like applications — 4 to 25 tables, at least two
foreign keys, a time and a number, names in ASCII, framework tables under
40 % — one a repository, after a seeded shuffle. Each was shown, as plain
text of tables, columns, types and keys, to **Gemini 3.8 Flash** through
`agy`, and the first 150 also to **Claude Haiku**, six a prompt, with the
question the blind gold questions of round 1 were written to: the ten
questions a person just given the database would want answered first, the
first five for the first screen, each as base, measure, time, breakdown,
filter and joins. Neither saw what the generator makes.

A teacher's question labels the generator's question it matches: **3** in
its first five, **2** in its ten, **1** sharing the base and the measure or
the base and both dimensions; the larger of two teachers' labels counts.

## The ranker

`rank.py` gives each question 90-odd numbers from tier S: what its base is
— a subject, a subject's main fact, an event, a lookup, a link, a
framework's — and how it stands beside the schema's other tables by the keys
pointing at it and out of it, its columns and its measures; its measure's
kind and role and place among its table's; its time's kind and place; its
breakdown's kind, distance and table; its shape. `learn.py` fits
LightGBM's LambdaRank to the labels, **600 trees of seven leaves**, measured
over five folds by repository, and `pack.py` packs it into **55 KB** — ten
bytes a node, four a leaf — with a reader of its own that scores within
3·10⁻⁸ of LightGBM's. With statistics, a small hand layer is added to the
ranker's score: a table of under 20 or 100 rows loses, the largest tables,
where what happens accumulates, gain; no schema without rows could teach
that.

## The five

The best five that add something: no two over one time column or one
breakdown, at most two from a table, no question twice in two shapes. A cap
on one kind of panel was tried and dropped: the teachers themselves show
several counts over time.

## How it is measured

| Measure | Against |
|---|---|
| teachers' agreement, out of fold | each held-out schema's teachers: the share of the five a teacher asked anywhere in its ten (precision), and of the teacher's first five in the five (recall); beside how often two teachers agree |
| round 1 on the same schemas | its rules' five for configuration A, read as analytical questions |
| the six fixtures | round 1's gold questions, the agent's on the development fixtures and Haiku's blind ones on the check fixtures — both of which round 1c was developed looking at |
| two fixtures never looked at | classicmodels and Credit from CTU's repository, with gold questions Haiku wrote blind from their schemas, run once, nothing changed after |

## Files

| Path | What |
|---|---|
| `aq.py` | the schema read for roles, subjects and keys; the analytical question; the generator; the compiler to SQL |
| `gold.py` | gold and teachers' questions read as analytical questions |
| `rank.py` | the features, round 1c's hand prior, the statistics' vetoes and points, the choice of five |
| `teacher.py` | the teacher schemas chosen, written as snapshots and as text, cut into prompts; answers gathered |
| `learn.py` | the training data, the folds, the ranker fitted and measured, the fixtures' gold in the five |
| `pack.py` | the ranker packed into bytes and read back without LightGBM |
| `baseline.py` | round 1's rules measured against the teachers |
| `see.py` | round 1c's five run on each fixture and drawn on round 1's page beside round 1's |
| `judge.py` | every fixture's pool packed blind for judges, and their answers unblinded and counted |

The corpus, the teachers' answers, the training data and the model live in
`lemmesee/corpus/round-1c/`, which git ignores. Training runs offline in
Python, in round 1b's environment.
