# TinyStore 0.1.1 against the five issues, 24 October 2026

TinyStore 0.1.1 closed issues #1–#5 of
[tinyshed/tinystore](https://github.com/tinyshed/tinystore). Each was rerun
against the published 0.1.1 crate rather than taken from the closing comment.

| Issue | 0.1.0 | 0.1.1 | What follows for Dashbin |
|---|---|---|---|
| #1 an abandoned read | ran to the end, held a reader | `tinystore::aio`: a dropped future stops its read; 36 abandoned, the next `select 1` in 0.2 ms; a 200 ms tokio timeout cut a long read at 203 ms | the hub uses `aio`, no `spawn_blocking` wrapper |
| #2 `once` with a 2 MiB answer | 4 runs, 2.0 s | 1 run, 0.50 s for all four | collapsing panel requests stays in Dashbin's memory, as agreed in #2 |
| #3 the build warning | lost | printed as the logger's first line in the docs' order | the recommended flags gained `-DHAVE_FDATASYNC=1` |
| #4 series cap | constant 100 000 | `Options { max_series, .. }`: series 100 001 accepted with 1 000 000 | — |
| #5 Prometheus text | no reader | `stats.prometheus().label(..).ingest(body)`; a label both sides give is refused naming the line | a scrape over one ingest (10 000 samples) is refused, not split: watch on real exporters |

## Environment and reproduction

- `tinystore = "=0.1.1"` from crates.io, features `kv`, `metrics`, `records`,
  `sql`, `async`.
- Windows 11, Rust 1.97.1, Ryzen 7 7700; functional checks, one pass each.
- The harness was not committed before it ran; it is committed afterwards with
  this report, unchanged. `once.rs` and `warning.rs` are 0.1.0's programs
  unchanged; `pileup.rs` moved to `tinystore::aio`; `series.rs` sets
  `max_series`; `prom.rs` is new.

```sh
cd <research>/dashbin/tinystore-repro/0.1.1
cargo build --release --bins
./target/release/warning; ./target/release/once; ./target/release/series; ./target/release/prom; ./target/release/pileup
```

Raw output: [data/tinystore-0.1.1-recheck-2026-10-24/outputs.txt](data/tinystore-0.1.1-recheck-2026-10-24/outputs.txt).

## What follows

The hub is designed on `tinystore::aio`; the embedding trap is now the flags
line, which a new Dashbin repository carries in its `.cargo/config.toml`.
