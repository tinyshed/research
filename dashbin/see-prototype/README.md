# A prototype of `dashbin see`

What chapter 14 asks for before `see` is built: a store drawn before anyone
made a dashboard, in `see`'s own screen for keeping and dropping, from
lemmesee's rounds 1c to 1o ported to Rust — the snapshot read, the questions
made and compiled, round 1l's ranker of Luna and DeepSeek, round 1m's screen
of roles, round 1n's row of numbers and round 1o's charts.

```
see <snapshot.json> --docker <container> [--user <role>]          a database on this machine
see <snapshot.json> --ssh <host> --container <container> [--key k]  one on its own host, read only
see <doc.json>                                                      a screen document already made
    [--state <dir>]        where kept, dropped and added panels are remembered (.dashbin/see.toml)
    [--shot <out.png> --size 150x46 --px 15 --press x,down,f]   the frame as a picture, keys played first
    [--html <out.html>]    the same screen as one static page
    [--doc <out.json>] [--doc-only]
    [--unbounded]          read as lemmesee's rounds read, whole tables and all
```

At the screen: `←↑↓→` a panel, `enter` that panel alone, `f` keep, `x` drop
(the next of its role takes its place), `+` add from every role's next
questions, `u` undo, `s` save the kept panels as `<store>.dash.toml`, `q`
quit. What was kept and dropped is remembered between sessions by the
panel's question, never in the store looked at.

## Held to the Python

`golden.py` writes what lemmesee's Python says of every snapshot in
`../../lemmesee/corpus/see`; `cargo test --release --test golden` checks the
Rust core against it. On 30 October: 106 snapshots (the 94 public databases'
and round 1m's twelve), 91 276 questions made with their
vetoes, 78 983 kept with their statements to the byte and the ranker's score
within 1e-6, 1 891 300 features, round 1c's five, round 1m's screen and every
question's words — every one the same; a fault planted in one title was
found on the first three snapshots.

`export.py` makes the screen document in Python; the same document made by
`see` on round 1m's twelve bench databases is the same in every number,
panel, row and spare, but for a count of the last seven days that moved
between the two runs, its window ending at `NOW()`.

## The screen

Tried in Windows Terminal and kept as little as a build tool's output: the
terminal's own background and its sixteen colours, colour only for a
panel's role, the selection and the keys. While the store is read, one line
on stderr: a spinner, the step in hand, the time, then `✓ <store> in 7.9s`.
A series is one thin line in box characters, a row a level, from its own
lowest to its highest, both written at its left and its last value where it
ends (` today` after it while the day is still going); a split, a band a
value on one scale; a ranking, a thin rule a value. A title names each
table once and is cut at a word if it still does not fit; nothing scrolls.
`--shot` draws the frame in Windows Terminal's Campbell colours, box lines
and blocks drawn as shapes, as the terminal draws them.

`--html` is the same screen in the language of the dashbin mockups: their
tokens, dark or light by the reader's setting, a card a role on six
columns, the numbers as a band of tiles. A panel is its last value over its
chart: a count or a sum by day as bars, a share or a time as a line over its
area, a split as small charts on one scale, a ranking as a list; the day
still going faded or dashed. A funnel of the numbers is a card of its own,
its steps as bars from one edge on a track of the first, the share that
goes on between them; its later steps and those shares leave the band. Each card's head says its window in mono, its
foot the database and the rows; a value's day is under the pointer. One
page, no script, no request; it fits a phone.

## What looking costs

Chapter 14's bound, held by `src/core/bound.rs` and the runner: a look reads
a source's catalog and statistics, at most 10 000 rows of a table in a
statement, inside 2 s of server time a table, and never a table above
10 000 rows whole. Every statement the core writes is planned before it
runs, cheapest first:

- as written, when every table it names is small;
- from the catalog or the statistics, reading no row: a whole table's count
  (`reltuples`), a column's distinct values (`n_distinct`), rows by a value
  (`most_common_vals`, which `ANALYZE` took from rows, not pages);
- exact through an index, when its window holds few rows;
- exact over the newest 7 000 rows of a table kept in its time's order (its
  time's correlation with its pages at 0.95 or more), off its last pages by
  a range of `ctid`, its first and cut-short day dropped, at least 7 days;
- over a sample of about 7 000 rows of the big table's pages,
  `TABLESAMPLE SYSTEM … REPEATABLE (17)`, the same pages for every
  statement, its counts and sums scaled, its averages kept;
- not at all, saying why: a count of distinct values or "two or more"
  where no exact way holds, a day of a log (a sample of a log's pages is a
  few moments), the rows of a value kept together on its pages, a sample too
  thin to say a day (20 of its rows), a value (5) or a number (30).

A window's end, `MAX` of a time, is read once a look and written into every
statement that ends there. Each statement runs in a read-only transaction,
its timeout what is left of its tables' 2 s (and never more than the 0.5 s of
`--ssh`), joins in their written order as lookups by key through an index,
and a guard before it: a `DO` block reads the statement's plan in the
database and refuses it if it would read a big table in sequence or along an
index with nothing to look up. What is not the whole's answer is said: `≈`
before it on the screen and the page, and what it is beside it.

`cost.py` measures a look: every statement of its trace (`SEE_TRACE=<file>`)
run again under `EXPLAIN (ANALYZE, BUFFERS)`, the rows each read of each
table, whether it read a big one whole, the server's time. `drift.py` sets a
bounded look's answers against the whole's; `looks.py` runs both on every
PostgreSQL store of the corpus with a table above 10 000 rows, one after the
other on each.

## Not done

- A count of distinct values over a window too big to read exactly, and
  "two or more", are refused; the statistics' `n_distinct` of a whole column
  runs low on big skewed ones.
- A number over a window of a table kept in time order comes from a sample
  of its pages, not its newest rows.
- A MySQL runner, and `postgres://` without a container; the snapshot is
  still made by round 1's harness.
- The mockups' one cursor across every chart of the page, and the table of
  values at it under a chart of several lines.
- A ranking by a linked table's label groups by the row and shows the label,
  so two rows of one name are shown twice.

`out/` (documents, pictures, the golden dumps) and `.dashbin/` hold a store's
numbers and are not committed.
