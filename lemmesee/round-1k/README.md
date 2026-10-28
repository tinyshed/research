# Round 1k: cheaper teachers, and GitTables as a school for the look

Status: run on 28 and 29 October 2026; [reported](../reports/ceiling-2026-10-29.md).
No new model is kept: Jev (fast) and Luna replace DeepSeek and GLM as
teachers, Sonnet and Gemini now count all 93 databases, and the next gain is
a person's click.

Round 1g found the look limited by what words mean together with what a
result shows, and 93 databases too few to teach it. Round 1h taught names
alone and gained nothing. This round asks whether tables with real values,
many more of them, teach it: tables from CSV files in public GitHub
repositories, each read as a database of one table, their panels made by
the same generator and ranker and labelled by teachers that cost little.

## Teachers

Every new teacher is first measured on the 2 862 panels of rounds 1d–1f,
by round 1g's ceilings: the five its own answers would choose, counted by
the judges that never teach, and its AUC within a database against them.

| Name | What | How it is asked |
|---|---|---|
| `jevk5`, `jevk5fast` | JevK5 v0.3 4B, an open-weight decision model, as a Q8 GGUF on llama.cpp b11968 on the RTX 5070 | `local.py`: Jev's two questions; fast asks keep alone of the compact result |
| `luna` | GPT-6 Luna through Codex on a ChatGPT Plus subscription, behind the CLIProxyAPI | round 1d's per-database prompts as DeepSeek, GLM and Gemini saw them (`round-1d/gemini.py`) |
| `lunap` | the same | the same prompts with keep asked as a chance from 0 to 100 (`graded.py`) |
| `jevfast` | Jev 1.13 | keep alone, of the compact result (`jevfast.py`) |

## The pilot

`gittables.py` reads five topics of GitTables 1M (Zenodo record 6517052,
version 0.0.6: rate_of_return, growth_rate, crime_rate, inflation_rate,
time_interval; 33 671 tables, md5 as the record gives them). It keeps
tables of permissive licences (MIT, Apache, BSD, ISC, CC0, CC BY 4.0, the
Unlicense), of 50 rows to a million, three to forty named columns, a first
row that was a header, no two with the same set of column names, at most
five a repository, and none whose columns are half or more of a judged
database's table's. Each is a view in DuckDB, text that reads as times typed
as times; its snapshot holds the fields round 1's reads from `pg_stats`,
computed exactly rather than estimated; round 1d's `pool` writes its ten
best questions and both fives, with DuckDB running the SQL. A table the
generator asks fewer than five questions of is left out. Panels are packed
and cut as round 1d's, and labelled by `jevfast` and `lunap`.

## The comparison

`compare.py prepare` builds the look's 127 numbers for the GitTables panels
by round 1j's path, checks that path against round 1j's frozen numbers on
the 93 databases, and fits nothing. `compare.py run` fits round 1j's
baseline look over its family folds and draws, with round 1j's seeds:

| Config | The 93 taught by | GitTables |
|---|---|---|
| `old` | Jev, DeepSeek, GLM (round 1j's baseline, repeated) | not used |
| `new` | `jevfast`, `lunap` | not used |
| `new+gt` | `jevfast`, `lunap` | every training fold, never counted |
| `old+gt` | Jev, DeepSeek, GLM | every training fold, never counted |

Each is counted as round 1j counts: AUC within a database and kept of five,
with no penalties and with each axis once, by Opus, Sonnet and Gemini, the
differences bootstrapped over families. These are development
comparisons on databases already looked at, not a held-out confirmation,
and per-panel labels, not fives judged as sets.

## The ideas

Probes in `ideas/`, each run on what is already here, none fitting a
model; their outputs are in `corpus/round-1k/ideas/`. Development numbers
on databases already looked at, per-panel labels, not a report.

| Probe | What it asks | Where it stands |
|---|---|---|
| `second_click.py` | what a person's first one to three answers are worth: the teachers' mean drops the worst panel of the look's five, which is chosen again; carried, candidates sharing its parts lose too | one click, 0.58 used on average, keeps +0.31, +0.30, +0.24 more of five by Opus, Sonnet, Gemini, each clear of zero; three, +0.42, +0.44, +0.38; carrying by shared axes does worse than dropping alone, Sonnet −0.28 [−0.58, −0.02] |
| `extensive.py` | whether a summed column's spread, p95 over p5 of its positive values read from the database, tells what a sum means | over 586 sums, Spearman with the teachers 0.26, the judges 0.18–0.28, the look 0.08; sums in a band under half a decade kept 5–10 % by the judges against 24–34 % over one and a half, the look scoring them alike; averages the other way |
| `head.py` | the head word of a name apart from its modifiers, against round 1g's largest over all | the measure's head `id` meets the teachers at −0.20 against −0.09, `sum_ok` 0.16 against 0.13; CreditCardNumber's `sum_ok` 0.3 by its head, 0.8 by its largest word |
| `fingerprints.py` | how often a table has a near copy among SchemaPile-Perm's 164 475 tables | of the judged databases' 545 tables, 15 % at Jaccard 0.9 or more; of the production databases' 90, 1 %; of the pilot's, under 1 % |
| `intent.py` | what the databases declare of themselves | across 94 databases: no ON UPDATE, generated column, trigger, partial index or updated row; CHECK in one, enums in three, defaults in thirteen, nearly all numbers; the views are pg_stat_statements' but for the airlines demo's |
| `semantic.py` | LookML, dbt semantic models and Malloy on GitHub as people's own judgment of measures and dimensions | code search counts 6 552 LookML files summing, 3 848 averaging, 677 dbt semantic models, 670 Malloy files; `list`, `licence`, `fetch` and `parse` written, not run |
| `funsearch.py` | a scoring program a language model writes and rewrites, kept by its order of the pilot's panels and counted once on the 93 | inputs built and a hand rule checked, 0.60 on the pilot and 0.55–0.57 by the judges; `evolve` not run. The restricted builtins guard against accidents, not an adversary |
| `features.py` | head and spread readings as numbers beside round 1j's | built for the 2 632 panels; `compare.py` fits them as `old+head`, `old+spread`, `old+both` |

The fingerprints also say that 15 % of the judged tables have near copies
in SchemaPile-Perm, which taught round 1c's ranker; whether its questions
were asked of those schemas is not yet checked.

## Reproduction

    python -I round-1k/gittables.py survey corpus/gittables/raw corpus/round-1k/gt
    python -I round-1k/gittables.py select corpus/round-1k/gt 12000
    python -I round-1k/gittables.py build corpus/round-1k/gt
    JUDGE_KEY=<key> python -I round-1k/jevfast.py corpus/round-1k/gt/look 32
    CLIPROXY_KEY=<key> python -I round-1k/graded.py ask corpus/round-1k/gt/look <endpoint> gpt-6-luna lunap 4
    python -I round-1k/graded.py gather corpus/round-1k/gt/look lunap

The ideas, each from the research repository's lemmesee folder:

    python -I round-1k/ideas/second_click.py corpus/round-1k/ideas
    python -I round-1k/ideas/extensive.py corpus/round-1k/ideas
    python -I round-1k/ideas/head.py corpus/round-1k/ideas
    python -I round-1k/ideas/fingerprints.py corpus/round-1k/ideas
    python -I round-1k/ideas/intent.py corpus/round-1k/ideas
    python -I round-1k/ideas/features.py corpus/round-1k/ideas
    python -I round-1k/ideas/funsearch.py data corpus/round-1k/ideas/funsearch
    python -I round-1k/ideas/semantic.py list corpus/round-1k/semantic

After the harness is committed, in the Linux container of round 1j:

    docker run --rm --cpus=4 -v <repo>:/src -w /src/lemmesee lemmesee-joint python round-1k/compare.py prepare corpus/round-1k/compare
    docker run --rm --cpus=4 -v <repo>:/src -w /src/lemmesee lemmesee-joint python round-1k/compare.py run corpus/round-1k/compare

The GitTables files, the pools and every answer stay in the ignored corpus.
