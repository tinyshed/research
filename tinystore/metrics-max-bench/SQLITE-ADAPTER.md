# SQLite adapter experiment

This extends the retained deep metrics prototype with independent SQLite
adapter switches. Production TinyStore and the locked rusqlite dependency are
unchanged. The experiment lives in research beside its baseline source.

`--adapter` selects a bit mask:

| Bit | Change |
| --- | --- |
| 1 | Borrow registry label-ID bytes through `Row::get_ref()` while parsing them. |
| 2 | Copy selected external payload BLOBs into one exactly reserved owned arena, then share ranges between blocks. Applies to the batched path (at least 16 selected series). |
| 4 | Disable per-connection SQLite lookaside. |
| 8 | Configure per-connection lookaside with 512-byte slots and 512 slots, using SQLite-owned storage. |

Bits 4 and 8 are mutually exclusive. Zero is the modified-source control, not
the original binary. The runner also builds the original source from research
commit `8955ce2` and remeasures that binary in every pass, detecting costs from
the shared-body representation even with the switches disabled.

An arena retains owned bytes after the statement and snapshot close. It does
not hand SQLite pointers to workers, decode payloads while rows remain live,
remove the required SQLite-to-owned-memory copy, or implement a shared native
memory admission system. The aggregate payload size has already been charged
to the existing query byte budget before arena allocation.

`--mode wrapper --case safe_64` and `raw_64` compare an already prepared safe
rusqlite statement with an independently owned raw SQLite statement. Both use
the same SQL, PERSISTENT, checked types and owned `(id, Vec<u8>)` results. The
raw statement cannot outlive its connection or cross threads. Its test covers
reset after a conversion failure and retained bytes after connection close.
This probe omits normal engine matching, budgets, decoding and aggregation.

The telemetry binary adds native connection counters and uses the preceding
Rust allocation/snapshot instrumentation. It is separate from timed binaries.
Native SQLite/zstd malloc calls are not counted by the Rust global allocator.
Lookaside hit/miss totals are SQLite high-water fields; zero current fields for
those counters do not mean zero activity.

## Reproduction

Use a Linux build environment with Go 1.27.1, Rust 1.99.0 plus rustfmt, Python
3, a C compiler/ar, taskset, lscpu and GNU time. The local run used Docker
Desktop/WSL2; its ratios are exploratory local evidence and its single-request
latencies are not bare-Linux or server-throughput promises. Keep other builds
and tests stopped throughout measurement.

Mount this research checkout at `/src` and a persistent Linux volume at `/work`.
Set `CARGO_HOME=/work/cargo`, `RUSTUP_HOME=/work/rustup`, `GOWORK=off` and
`CGO_ENABLED=0`. Install Rust at those locations. Initialize `tinystore/source`
at its pinned commit. From the host research checkout, archive the original
Rust baseline without including current edits:

```sh
git archive --format=tar 8955ce2 tinystore/metrics-max-bench/rust > baseline.tar
```

Make that archive available in the container, then run:

```sh
mkdir -p /work/baseline /work/bin
tar -xf /src/baseline.tar -C /work/baseline --strip-components=3
python3 /src/tinystore/sqlite-bench/build_native.py
go -C /src/tinystore/metrics-max-bench/go build -trimpath -o /work/bin/go-reference .
sh /src/tinystore/metrics-max-bench/adapter_build.sh
python3 /src/tinystore/metrics-max-bench/adapter_run.py --verify-only
python3 /src/tinystore/metrics-max-bench/adapter_run.py --passes 6
```

The builder reuses existing fixtures; use a fresh `/work/stores` for a fresh
round. Stores and targets stay on the Linux volume. The runner copies a closed
fixture for each child, checks 56 traces across eight configurations plus two
forced-worker configurations, error guards and selection/resource plans, and
cross-reads completed writes through Go. It performs six balanced passes over
13 complete operations, then separate allocation/native-counter profiles,
three RSS passes and six paired wrapper passes. Counts are calibrated once
from the current-session original Rust baseline and held equal across variants.

At the user's explicit request, the harness was uncommitted while measuring.
The raw environment retains exact source/binary hashes. The report identifies
that limitation; preserving the source later does not change its status during
collection. Raw results are in
`../reports/data/sqlite-adapter-2026-10-08/`.

## Small rusqlite dependency patches

The followup also builds rusqlite 0.40.1 with three isolated source patches:
an inline hint on `Statement::value_ref`, a lazy column-count cache invalidated
before every step/reset, and their combination. The per-step invalidation is
required because SQLite can reprepare `SELECT *` after a schema change. The
cache preserves checked column access and the public API.

From the same container after the first build:

```sh
python3 /src/tinystore/metrics-max-bench/rusqlite_fork.py
python3 /src/tinystore/metrics-max-bench/rusqlite_run.py
```

The builder fetches the locked crate through Cargo, copies it into `/work`,
applies exact-match source edits and retains minimal unified diffs plus source
and executable hashes. No third-party crate source is vendored in research.
It checks that changing the rusqlite source leaves every other locked package
unchanged. It runs the upstream library tests on stock and each patch with
separate test targets, plus regressions for schema reprepare, invalid indices,
conversion failure and cached-statement reuse. With the pinned `SQLITE_DQS=0`
archive, 11 upstream tests that use double-quoted string literals fail on stock
and all patches; the builder verifies the same failure set rather than skipping
or rewriting those tests. Results remain visible in the raw logs.

The second runner repeats the 56 engine traces, two-thread checks, guards and
plans. It compares the original prototype, modified-source stock dependency,
and all three patched libraries in six same-session passes. It also repeats
the head/lookaside case and compares safe/raw wrapper loops in every dependency
build. Raw output is in `../reports/data/rusqlite-fork-2026-10-08/`.
