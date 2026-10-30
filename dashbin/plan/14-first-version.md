# 14. The first version, its phases, and the rounds before the code

**Status: open, 27 October 2026.** What the first version holds and in what
order it is built is not written yet. Two tracks run from here: Dashbin's own
rounds before its code, and lemmesee, the research behind `see`, which
starts first and blocks nothing — `see` ships on rules, and a ranker comes
through its seam when it earns it. The research rounds that have to run
before parts of the code are being collected here as the chapters name them;
the first is written out below.

## Research rounds before the code

### `see`: how good are its guesses

**The question.** Chapter 11's `see` draws a store before anyone made a
dashboard, from panels it **knows** — a TinyStore metric's kind, unit and
help, a stream's fields — and panels it **guesses** from a table's columns.
Would a person keep what it draws, and does a guess ever look right while
meaning something false?

**The stores**, of three kinds:

- **TinyStore's**: the stores TinyStore's own research rounds keep, a
  Dashbin's data directory with its own measurements and log, and the metrics
  a ping monitor wrote;
- **a public database with a known schema**, chosen so that anyone can run the
  round again;
- **two production databases of Dmitry's sites**, read-only, their schema and
  aggregates only and nothing a user wrote, under the rules those databases
  are read by. What is reported of them is counts, never a table's or a
  column's name, because this repository is public.

**How it is measured.**

- A prototype of `see` is written in `research/dashbin`, committed before it
  runs, as every round is: a generator of safe candidates by a column's type,
  hard rules for what may be drawn, and weights that put the rest in order
  (chapter 11).
- A person who knows each store goes through what it drew in `see`'s own
  interactive screen, keeping and dropping panels (chapter 11). What they keep
  and drop is the measurement, and it is the same instrument the product
  ships.
- For every guessed panel they kept, they also say whether it means what its
  picture suggests: a sum of `price` over orders of every status looks like
  revenue and is not.

**What is decided before it runs.**

| Measure | Known panels | Guessed panels |
|---|---|---|
| kept | 80 % or more | 50 % or more |
| harmful — kept, and meaning something false | none | at most one in twenty |

| Measure | Every store |
|---|---|
| what looking costs a source | its catalog and statistics, and at most 10 000 rows a table, inside 2 s a table — never a full scan |

Measured on 30 October, [What looking costs a store](../reports/see-bound-2026-10-30.md):
`see`'s prototype holds the bound on every PostgreSQL store of lemmesee's
corpus with a table above 10 000 rows, 36 of them, and on two production
stores — the catalog and statistics first, then an index, a log's newest
rows or a sample of pages, a guard in the database refusing a whole read.
What it costs is answers: estimates marked as such, and a quarter of the
panels refused, distinct counts above all, which the next round of `see`
owes.

If guessed panels miss either line, they are not drawn by default: `see`
draws what it knows and offers its guesses on request, marked as guesses. If
known panels miss theirs, the rules for a kind are changed and the round runs
again.

**What it decides.** Which rules `see` ships with, whether guesses are drawn
by default, and which of chapter 9's suggestion weights the first version
needs beyond Go Dashbin's.

### `see`: a small model against the rules

Runs only if the first round's guessed panels miss their lines. It is no
longer a round of Dashbin's own but a research of its own beside Dashbin,
**[lemmesee](../../lemmesee/plan.md)**, which grew from it on 27 October:
what follows is what Dashbin asks of it, and lemmesee's plan says how it is
found. Its configurations A and B are the rules this chapter's first round
ships; C and D are what lemmesee may bring.

**The question.** Does a small classifier, trained on public tables, guess
what a column means better than hand-written rules — and is it small enough
to carry?

**What it would do.** Rank the candidates the first round's generator
writes, never write one: a model that decided summing user ids is insightful
has nothing to draw it with. To rank them it learns a column's kind of
meaning — an amount of money, a
status, the moment something was made, an identifier, a count — from what
can be read without understanding the business: the tokens of its name, its
type, how many distinct values it has and how many are empty, whether it
only grows, whether it looks like money (two decimals, never negative), how it
moves over time, and the columns beside it. Gradient-boosted trees or a tiny
network over those features, not a language model.

**What it is trained on**, public corpora only, checked for their licences
before use:

| Corpus | What it gives |
|---|---|
| GitTables | about a million tables from GitHub, columns labelled with semantic types |
| SchemaPile | 221 171 schemas from SQL files on GitHub, 1.7 million tables with their keys and constraints; its permissively licensed part, SchemaPile-Perm, 22 989 — closest to a shop's `orders` |
| BIRD, Spider 2.0 | databases with real schemas and data, BIRD 95 of them with 12 751 questions written as SQL — what people actually ask a database |
| VizML's corpus, Table2Charts' | tables with the charts people drew from them |
| VizNet, WDC Web Tables, SOTAB | tables from the web and open data, with columns annotated |

The numbers are the corpora's own pages' as a review quoted them on 27
October, and are checked again before a corpus is used; so is its licence,
and only what the licence allows is trained on.

What was done before is the starting point rather than a thing to repeat:
Sherlock and Sato for a column's type, DoDuo with a whole table as context,
VizML and Table2Charts for which chart a person draws.

**Never trained on a user's schema.** Telemetry never sends a name (chapter
12), and the stores of Dmitry's sites are measured on, never learned from.
What a person keeps and drops in `see` may tune the suggestions on their own
machine, and goes nowhere.

**How it is built.** Trained offline in Python, its weights carried in a
compact format and read by a few hundred lines of Rust: no Python, no
PyTorch and no process of its own in the product. Gradient-boosted trees, as
CatBoost makes them, and a small network over the same features are both
tried; the rules with Draco-like weights are the third contestant and the one
to beat.

**How it is measured.** It is trained and tested on **different databases**,
never on rows of the same tables shuffled, or it learns the tables and
recognises nothing new. Then the same stores as the first round, the same
instrument — `see`'s own screen, keeping and dropping — and the same lines,
with the rules and the model side by side on each store.

**What is decided before it runs.**

| Measure | To ship in the binary |
|---|---|
| guessed panels kept | more than the rules kept, on the same stores |
| harmful guesses | no more than the rules, and at most one in twenty |
| weight | 2 MiB at most added to the binary |
| time | under a millisecond a column |

**What it decides.** The model ships in the binary if it passes every line;
if it beats the rules and weighs too much, it is an optional plugin; if it
does not beat them, the rules stay. Either way the first round's split holds:
a model ranks what a column probably is, and which statuses are paid is still
the person's to say — once, as a confirmed meaning (chapter 11), after which
nothing guesses it again.

### Named by other chapters, not written out yet

- The load run of the hub against 64 MiB, which sets the budgets of chapters
  4, 5, 6 and 12.
- Ten agents end to end, with dropped connections, cancellation reaching
  PostgreSQL and MySQL, and one connection overloaded (chapter 8).
- The access model's test over sequences of changes (chapter 10).
- A line of 50 000 points in ECharts and in uPlot, and twenty sandboxed frames
  on one dashboard (chapter 9).
- A thousand flows in `dashbin.db`, and a process killed inside a commit unit
  (chapters 4 and 7).
- The TOML file's round trip over generated documents (chapter 11).

## Open

- What the first version holds. Chapters say what they propose — chapter 10
  step 0 and the audit, chapter 11 `see`, `show` and `query`, chapter 9 the
  browser's dashboards — and the order matters: the terminal must not hold
  back the first dashboard in a browser.
- The phases after it.
