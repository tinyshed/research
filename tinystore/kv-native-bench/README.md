# KV algorithms and native SQLite engine slice

This is retained research code for the [8 October round](../reports/kv-native-2026-10-08.md),
not a production port. It builds the real public Go API from
`../source` at `e307c48a40126aad0e2873b6bf3aaedef8115483`, and a synchronous
Rust slice using **stock rusqlite 0.40.1**, Rust 1.99.0 and the externally
linked official SQLite 3.53.4 archive built by `../sqlite-bench/build_native.py`.
That archive matches the Go backend's SQLite source and chosen compile options;
native thread safety, initialization and Unix VFS remain intentional differences.
The Rust timed binary has no allocation-counting allocator. A separate
`telemetry` feature binary is used only for diagnostic allocation collection.

The original 4 KiB KV schema, 512-byte spill boundary, 1 MiB value bound,
WAL/FULL durability, reader/writer cache sizes and branch-clear SQL rules are
retained. Native results own their bytes before a statement resets. Timed
native reads black-box the complete returned entry/page/document before consuming
it, and the Go kernel's owning paths cross noinline consumers. The fixtures and
traces are identical, including the permutation of point keys.

## Implemented and checked

- Raw NULL/int64/bytes, short/empty/large byte and string values, integer rows,
  big-endian uint64 and float bits, including `-0` and signaling NaN payloads,
  JSON records, and inline/spilled values. JSON timings encode/decode the same
  fields; bytes timings use preexisting byte values on both sides.
- `Get`, `Has`, `GetEntry`, `Set`, overwrite, versioned CAS, `SetIfAbsent`,
  `Take` and `Delete`, expiry retention/recreation, explicit expiry, default
  TTL, and `Touch` without changing a version.
- Branch-local paged scans, byte ordering for the valid UTF-8 text inputs,
  `After` continuation, 1000-entry and 4 MiB page caps, embedded zero escaping,
  deep branches, small clears, marked clears past 10,000 keys, and bounded
  expiry/clear maintenance with spill cleanup.
- Durable `Add`/`Max` counters, int64 overflow refusal, expiry reset, preserved
  expiry for live counters, and monotonically increasing committed versions.
- Failed typed `Take` rolls back deletion; an absent spilled value is corruption
  and a failed `Take` keeps the cell. Explicit transaction rollback leaves a
  revision gap. Reopening after mutation retains the high-water revision.
- Full Go/Rust command transcripts compare normalized sentinel category and
  exact bucket/key path. Full physical row snapshots compare after the trace.
  The native-written value shapes reopen through Go's typed API.
- A temporary mutant disabling failed-Take validation is rejected by the oracle.
  Its source/binary hashes and broken/restored transcripts are retained.

## Deliberate limits

The native slice opens a Go-created, migrated KV file. It does not implement
schema bootstrap/migration, directory locks, lifecycle/gates, concurrent callers,
grouped commits, process memory reservation, full user transactions/views,
context cancellation, background maintenance or custom codecs. `SetIfAbsent`
uses separate reads/writes and is valid only for this single-owner synchronous
slice. Counter Get in the trace reads the same raw integer path, not a separate
ported counter read implementation. Production method scheduling and all
concurrency/crash semantics need a further port and tests.

The native research API accepts valid UTF-8 text keys/owners. Production Go
accepts arbitrary key bytes; invalid-UTF-8 keys are **not covered**. Arbitrary
byte **values** are retained without UTF-8 conversion. Native error message
wording differs; sentinel category and key identity are the comparison contract.
Config/watch, once, limiter/quota, sliding renewal, relaxed counters and a cache
runtime are omitted. The value-codec kernel measures direct owned 64-bit
encoding/decoding, not Go's complete reflection-based generic codec dispatcher.

The SQL-only rows omit the engine and have their own section in the report.
The native C-API control uses the same SQL and copied owning bytes as safe
rusqlite; it quantifies wrapper cost for that statement, not a full-engine speedup.
Go's public KV handle exposes no SQLite counters; the retained Go SQLite
counters apply only to its internal/sqlite SQL control. Native C allocations
are absent from Rust's global-allocator counters, while RSS and SQLite cache
status include the resulting resident memory. Do not equate those allocation
accounting domains.

## Reproduce

From a research clone, verify the source commit before mounting it:

```sh
git -C <repo>/tinystore/source rev-parse HEAD
# Must be e307c48a40126aad0e2873b6bf3aaedef8115483.
# Build the matched SQLite archive first, as sqlite-bench documents.
```

In a Linux container with this repository at `/src`, Go 1.27.1, Rust 1.99.0,
Python 3, GNU time, cc and a Linux volume at `/work`:

```sh
export PATH=/work/cargo/bin:/usr/local/go/bin:$PATH
export CARGO_HOME=/work/cargo RUSTUP_HOME=/work/rustup
export KV_WORK=/work/kv-native
cd /src/tinystore/kv-native-bench
python3 build.py
python3 check.py
python3 guard_proof.py
# Stop all other builds, tests and measurements on the host/WSL VM first.
GOMAXPROCS=1 GOGC=100 taskset -c 2 python3 run.py
python3 report.py
```

`build.py` pins external linkage, records source/binary hashes and keeps Rust
targets and stores on the Linux volume. The raw output records exact versions,
SQLite source IDs, compile options, PRAGMA readback, fixture hashes and UTC run
order. `check.py` regenerates deterministic fixtures through the Go public API;
the large page-bound inputs are generated on the volume and retained as a recipe
plus per-value hashes/lengths, not committed as megabytes of repeated base64.
`trace.jsonl`, `reopen.jsonl`, and `corrupt.jsonl` are small retained traces.

The user explicitly waived committing the harness before collection. The round
is labeled **exploratory uncommitted collection**, and hashes replace that
provenance gate without claiming equivalent review history. Docker Desktop WSL2
ratios are local comparisons, not bare-Linux latency or throughput promises.
