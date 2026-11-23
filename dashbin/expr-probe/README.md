# expr-probe

What Dashbin evaluates a panel's formula with: `fails / flows * 100` over the rows of two reads joined by their
keys, and a condition such as `flows >= 20`, which an alert will later reuse. Four ways are weighed in the
static binary, timed and tried on what a formula can go wrong with.

| Variant    | What it is                                                                 |
|------------|----------------------------------------------------------------------------|
| `plain`    | the arithmetic written in Rust, no formula: the floor every weight is taken from |
| `own`      | a parser and an evaluator of ours, `src/own.rs`: numbers, names, `+ - * /`, comparisons, `&& || !`, parentheses |
| `cel`      | the `cel` crate, the Common Expression Language                            |
| `evalexpr` | the `evalexpr` crate                                                       |
| `rhai`     | the `rhai` crate, a scripting language: what "pass an eval" would cost     |

## What is decided before it runs

Written on 23 November 2026, before any number was taken.

A library is **taken** over a parser of ours when it passes every line; of those that pass, the lightest:

| Measure                                                                 | To pass                          |
|-------------------------------------------------------------------------|----------------------------------|
| what it adds to the static musl binary, fat LTO, stripped                | 300 KiB at most                  |
| 10 000 rows, a panel's cap, through `fails / flows * 100` and the condition | 5 ms at most, the formula parsed once |
| the names a formula reaches are known before any row                     | yes, so that a document is refused when it is checked |
| a formula can only compute: no loop, no assignment, nothing read or written | yes                           |
| a division by zero, and a name with no value in a row                    | an error or a value we can tell from a number, never a panic and never a silent infinity drawn as a point |
| its licence goes into an AGPL-3.0 binary                                 | yes                              |

For comparison, what the binary already carries: TLS about 714 KiB, the terminal's drawing 252 KiB, the page
with its fonts and charts 348 KiB.

If no library passes, the parser of ours is taken and the report says which line each failed. If one passes
and ours is lighter by less than 100 KiB, the library is taken all the same: a grammar others wrote, test and
that a model already knows is worth that much.

## How it is run

```sh
sh weigh.sh        # in the Linux image Dashbin's `just size` builds in: each variant's bytes
cargo run --release --features all --bin tried   # what each does with what a formula can go wrong with
cargo build --release --features all             # then each of target/release/* with 1000000, timed from outside
```
