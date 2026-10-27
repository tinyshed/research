# 14. The first version, its phases, and the rounds before the code

**Status: open, 27 October 2026.** What the first version holds and in what
order it is built is not written yet. The research rounds that have to run
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
  runs, as every round is.
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

If guessed panels miss either line, they are not drawn by default: `see`
draws what it knows and offers its guesses on request, marked as guesses. If
known panels miss theirs, the rules for a kind are changed and the round runs
again.

**What it decides.** Which rules `see` ships with, whether guesses are drawn
by default, and which of chapter 9's suggestion weights the first version
needs beyond Go Dashbin's.

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
