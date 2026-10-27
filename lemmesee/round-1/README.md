# Round 1: rules on the schema and statistics

**Status: specified 27 October 2026; nothing built, nothing run.** This folder
is round 1 before its code: what it reads, what it proposes, on which
databases, and how people judge it. The harness is built here against these
files and committed before it runs, as every round is; the report goes to
`../reports/rules-<date>.md`. A change to a line below after the harness is
committed is a change to the round, said in its report.

It answers [the plan](../plan.md)'s first question for configurations A and B
and pilots the judging the later rounds depend on. It draws on the
[methods review](../reports/research-start-2026-10-27.md), the
[wider reading](../reports/wider-reading-2026-10-27.md) and the
[brief](../opus-brief.md), and changes nothing in Dashbin's
[chapter 14](../../dashbin/plan/14-first-version.md).

| File | What it fixes |
|---|---|
| [contract.md](contract.md) | every feature: its exact query on PostgreSQL 18 and MySQL 8.4, whether it carries values, its states, and how the database's work is counted |
| [candidates.md](candidates.md) | column roles and their evidence, the templates, the hard rules, the reasons to abstain, the penalties that order what is left, and the documents passed between steps |
| [fixtures.md](fixtures.md) | the databases: public ones with their licences, schemas filled with generated rows, the states of their statistics and privileges |
| [traps.md](traps.md) | the cases built to mislead, each with what a careless rule draws and what is right |
| [judging.md](judging.md) | the form, the protocol, the measures and what the pilot can and cannot show |

## The questions of this round

1. **How good are rules** on the schema alone (A) and with the database's
   numbers about its columns (B), judged by people, on PostgreSQL and MySQL —
   and against Metabase's X-rays and a plain order on the same databases?
2. **What does reading cost each database**: every catalog and statistics
   query counted on the server, and, apart from it, what drawing each
   candidate would cost.
3. **Where do the rules fail**, and is each failure one a learned ranker or a
   look at rows could fix, or one only a person can?
4. **How judging goes**: minutes a judgment, agreement between two people,
   and whether judges catch the traps whose truth is known.

## The configurations

| | Reads | Orders by | Where |
|---|---|---|---|
| **O** | the schema | the schema's own order: tables, then columns as declared | everywhere; the floor |
| **A** | the schema: tier S | the penalties of [candidates.md](candidates.md) | everywhere |
| **B1** | and the numbers about columns: tier N | the same penalties | everywhere |
| **B2** | and the values inside statistics: tier V | the same penalties | public and generated fixtures only |
| **M** | whatever Metabase reads, which includes the first 10 000 rows of each table | Metabase's X-ray templates and scores | everywhere it connects; its reading counted like ours |

O, A, B1 and B2 share **one generator** and differ only in what they may
read and therefore in which hard rules can fire and how candidates are
penalised; a bigger pool is never mistaken for a better order. M's cards are
translated to the same candidate form and judged in the same pool. Tiers S, N
and V, and tier Q — the application's own queries — are defined in
[contract.md](contract.md); Q is collected for its availability only, since
freshly loaded fixtures have no workload.

## Decided before it runs

- **No rows are read to recommend.** Discovery reads tiers S, N, V and Q;
  rows are read only to draw what is judged, on the ledger of execution, and
  that ledger is reported apart.
- **Statistics are never refreshed** by the harness: no `ANALYZE`, no
  histogram made. Fixtures are prepared in the states [fixtures.md](fixtures.md)
  sets, and discovery reads what it finds.
- **The penalties are written before any judgment** and not tuned on the
  pilot's; tuning belongs to round 2 on a set held apart.
- **Chapter 14's lines are reported, not claimed.** Kept guessed panels at
  50 % or more and harmful at most one in twenty need, for the second, at
  least 59 kept guessed panels with none harmful to show at 95 %; the pilot
  reports what it saw with exact intervals and sizes round 2 by it.
- **Nothing a user wrote is read** on any database that is not public or
  generated. Dmitry's databases are not in this round; they come after the
  harness has run on public ones, with tier S and N only, reported as counts.

## Not in this round

The learned ranker (C) and looking at rows (D); SQLite and TinyStore's own
stores, whose known panels are Dashbin's round in chapter 14; MariaDB, whose
column statistics are not collected by a plain `ANALYZE TABLE`; confirming a
meaning, whose interface [candidates.md](candidates.md) reserves.

## What will be built

In `lemmesee/round-1/`, once these files are agreed:

| Path | What |
|---|---|
| `harness/` | a Cargo workspace: `snapshot` reads tiers S, N, V and Q into a snapshot; `rules` turns a snapshot into candidates, roles and penalties with no I/O, the part that could move into Dashbin as it is; `run` executes candidates on the ledger of execution and counts the work |
| `fixtures/` | a fetch script for each public database, writing into `lemmesee/corpus/`, which git ignores, and a file of SHA-256 hashes pinned at the first fetch |
| `generators/` | the table generators a language model wrote for the generated fixtures, kept because it will not write them the same way twice, with the model, version and prompt |
| `judging/` | the judging page or command, which reads candidates and writes decisions to a local file and nowhere else |
| `compose.yaml` | PostgreSQL 18 and MySQL 8.4 from their official images pinned by digest, `pg_stat_statements` and the Performance Schema on, and Metabase at a pinned release |

Heavy runs go in the Linux container as the `measure` skill says, with the
commits, versions, machine and commands in the report. The harness is Rust
because the rules that survive are the rules Dashbin ships; Python is used
only offline, to run generators and to analyse judgments.
