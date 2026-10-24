# TinyStore 0.1.0 as Dashbin would use it, 23 October 2026

Dashbin is being rewritten in Rust on embedded TinyStore. Before any product
code, its documentation was read as a caller would read it and every
inconvenience was run as a program against the published crate. Five
programs became the first five issues of
[tinyshed/tinystore](https://github.com/tinyshed/tinystore/issues?q=is%3Aissue+number%3A1..5).

| Question | Answer | What follows |
|---|---|---|
| Can a read nobody waits for be stopped? | No: 36 abandoned reads held all 16 readers, the next `select 1` waited 10 s and failed `unavailable` | [#1](https://github.com/tinyshed/tinystore/issues/1) |
| Does `once` collapse identical panel queries? | Only under 1 MiB: a 2 MiB answer made 4 waiters run the work one after another, 2.0 s instead of 0.5 s | [#2](https://github.com/tinyshed/tinystore/issues/2) |
| Is the `LIBSQLITE3_FLAGS` warning seen? | Not in the order the docs set logging up: it fires before any subscriber exists | [#3](https://github.com/tinyshed/tinystore/issues/3) |
| What do the flags buy on this machine? | Nothing measurable for a scan from 8 threads: 1325 and 1320 reads/s without, 1400 and 1108 with | said in #3 |
| Can a store hold more than 100 000 series? | No, `MOST_SERIES` is a constant | [#4](https://github.com/tinyshed/tinystore/issues/4) |
| Do expired series free their slot? | Yes: 100 000 past `keep`, `maintain` reclaimed 100 000, a new one was accepted | said in #4 |
| Can a scrape body be ingested? | No reader for the Prometheus text format | [#5](https://github.com/tinyshed/tinystore/issues/5) |

## Environment and reproduction

- `tinystore = "=0.1.0"` from crates.io, features `kv`, `metrics`, `records`,
  `sql`; tokio 1, tracing-subscriber 0.3.
- Windows 11, Rust 1.97.1 (stable-x86_64-pc-windows-msvc), Ryzen 7 7700. These
  are functional reproductions; the one timing table (flags, below) is two
  passes on Windows, not a performance claim for Linux.
- The harness was not committed before it ran: it lived in a session's
  scratchpad and is committed afterwards with this report, unchanged.
- No `.cargo/config.toml`, so the SQLite build is the one a caller gets by
  default; the flags variant was built with `LIBSQLITE3_FLAGS` set and its own
  target directory.

```sh
cd <research>/dashbin/tinystore-repro/0.1.0
cargo build --release --bins
./target/release/pileup     # #1, also cancel
./target/release/once       # #2
./target/release/warning; ./target/release/warning --listen-first   # #3
./target/release/readers    # #3, flags: CARGO_TARGET_DIR=target-flags LIBSQLITE3_FLAGS="SQLITE_DQS=0 -USQLITE_ENABLE_MEMORY_MANAGEMENT SQLITE_ENABLE_DBPAGE_VTAB" cargo build --release --bin readers
./target/release/series; ./target/release/churn   # #4
```

Raw output is in the issues as filed, kept in
[data/tinystore-0.1.0-from-dashbin-2026-10-23/](data/tinystore-0.1.0-from-dashbin-2026-10-23/).

## Findings

**An abandoned read cannot be stopped.** The Rust API is synchronous and has
no cancellation handle, so `spawn_blocking(...).abort()` leaves the read
running. Thirty-six reads abandoned after 100 ms ran to 6.3–13.1 s, four of
them timed out waiting for a reader, and an unrelated `select 1` waited the
10 s reader patience and failed. A single read the caller left at 107 ms
finished at 3.07 s.

**`once` re-runs its work for every waiter when the answer is too large to
keep.** A 100 KiB answer: one run, 0.50 s for four callers. A 2 MiB answer:
the kept value is refused by the 1 MiB bound, each waiter wakes, finds
nothing and runs the work itself, so four runs end at 0.50, 1.00, 1.50 and
2.00 s.

**The only warning about a slow SQLite build goes nowhere.**
`store.logger(..).layer()` opens `records.db`, which fires the warning
through `tracing` before `registry().with(log).init()` installs a subscriber,
and a `std::sync::Once` never fires it again. With a subscriber installed
first, the warning prints.

**The flags' cost did not show here.** Eight threads of 400
`select sum(v) from t where x % 7 = 3` over 200 000 rows: 1325 and 1320 reads/s
on the default build, 1400 and 1108 with the flags. A scan spends its time
computing, not fetching pages; this is a hypothesis, not separated by a second
measurement here (TinyStore's own answer in #3 measured point reads instead).

**The series cap is a constant, and churn is not a problem.** 50 hosts of 2000
gauges filled the 100 000 in 4.8 s and the next series was refused `limit`.
With a `TestClock` moved 48 h past a 1 h `keep`, `metrics::maintain` reported
`expired: 100000, reclaimed: 100000` and a new series was accepted.

## What follows

All five were closed in TinyStore 0.1.1; the recheck is
[tinystore-0.1.1-recheck-2026-10-24](tinystore-0.1.1-recheck-2026-10-24.md).
