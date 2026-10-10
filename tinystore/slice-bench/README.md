# The Rust slice against the Go engines

The harness of the [10 October round](../reports/rust-slice-2026-10-10.md):
TinyStore's Rust core, its kv, sql and jobs as an application calls them,
beside the Go engines it replaces, under one load, in process and through
Bun. It measures the product's own code on both sides, not a prototype.

| Path           | What it is                                                                                     |
|----------------|------------------------------------------------------------------------------------------------|
| `rust/`        | the Rust program: a member of the measured checkout's workspace once `build.sh` copies it in   |
| `go/`          | the Go program, on the public Go API                                                           |
| `bun/bench.ts` | the Bun program, through either SDK: the Rust core in process or beside it, or Go's server     |
| `build.sh`     | builds each side from a `git archive` of its own commit, into `/perf/slice` on the volume      |
| `run.py`       | the passes: a process and a fresh store a case, the programs' order turned round a pass        |
| `summarize.py` | a round's `runs.jsonl` as the report's tables                                                  |
| `mutex.sh`     | a diagnosis: reads by a few callers with SQLite's mutexes made to spin before they park        |

Both sides write the same keys, rows and jobs, pick the same keys by the same
xorshift, and commit as their defaults do: WAL and `synchronous=FULL`, a
write returned once it is synced. The Rust side builds with CI's flags on
Linux, `-DHAVE_FDATASYNC=1` among them. Each side keeps its own defaults
otherwise: its readers, its group commit, its threads or goroutines.

The Go module here builds against `../../source` like every other; the round
measures the Go commit `build.sh` names, whose tree it lays beside a copy of
`go/` and points the copy's `replace` at. The submodule's pointer does not
move with this round: every other module here builds against the Go tree.

```sh
# from Git Bash, MSYS_NO_PATHCONV=1; <tinystore> has both commits
docker run --rm -v <tinystore>:/tiny:ro -v <research>:/src -v tinystore-perf:/perf \
  golang:1.27 sh /src/tinystore/slice-bench/build.sh sources
docker run --rm -v <research>:/src -v tinystore-perf:/perf -v tinystore-go:/go \
  -v tinystore-gocache:/root/.cache/go-build -e GOWORK=off golang:1.27 sh /src/tinystore/slice-bench/build.sh go
docker run --rm -v <research>:/src -v tinystore-perf:/perf -v tinystore-cargo:/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/perf/slice/target rust:1.99 sh /src/tinystore/slice-bench/build.sh rust
docker run --rm -v <research>:/src -v tinystore-perf:/perf -e PASSES=3 -e SECONDS_A_CASE=3 \
  -w /src/tinystore/slice-bench golang:1.27 python3 run.py
python3 summarize.py
```

Bun is `/perf/bin/bun` on the volume, as the `measure` skill has it.

A round that compares builds of the Rust core gives each a tree of its own:
`WORK=/perf/<tree>` and `RUST_COMMIT` for `build.sh`, and `SQLITE_FLAGS` for
what the bundled SQLite is built with beside libsqlite3-sys's own flags.
`run.py` takes them as `ALSO=label=/perf/<tree>,…` and runs each tree's
programs beside the round's, as `rust-<label>` and `bun-rust-<mode>-<label>`,
`ONLY` and `CASES` choosing what runs and `RUNS` the file it is written to.
