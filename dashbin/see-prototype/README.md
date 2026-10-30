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

## Not done

- Chapter 14's bound on what looking costs: the numbers count whole tables;
  above 10 000 rows they should come from the engine's estimates and samples.
- A MySQL runner, and `postgres://` without a container; the snapshot is
  still made by round 1's harness.
- `--html` is still the first look, its own colours and filled columns, not
  the screen's.
- A ranking by a linked table's label groups by the row and shows the label,
  so two rows of one name are shown twice.

`out/` (documents, pictures, the golden dumps) and `.dashbin/` hold a store's
numbers and are not committed.
