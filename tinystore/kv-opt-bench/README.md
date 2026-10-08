# KV work normalization and optimization ablations

This prototype derives from `../kv-native-bench` without changing its measured
source or results. The baseline stays the public Go API at
`e307c48a40126aad0e2873b6bf3aaedef8115483`; native builds use stock rusqlite
0.40.1, Rust 1.99.0 and the same external static SQLite 3.53.4 archive.
The original schema, 4 KiB pages, WAL/FULL, reader/writer PRAGMAs, SQL bounds,
TTL/version rules, savepoints and owning read results are retained.

The old native Set returned a fully owned Entry, cloned its value and copied
its key, and the benchmark materialized that result. Go Set calls SetEntry
internally but returns only error; its internal Entry reuses the caller's
value and key rather than cloning their bytes. This new `set_void` writes
through `write_inner -> Written` and returns `Result<()>`. `set_entry` returns
an explicitly borrowed `WrittenEntry<'a>` holding the original caller key/value
and new metadata. Read Get/Scan/Take results continue to own their bytes.
The original owning SetEntry path remains for semantic transcripts, not for
no-result performance rows. This distinction fixes work parity explicitly,
without relying on the optimizer to erase a copied result.

Tuning switches isolate additional choices:

| `--tuning` | Work |
|---|---|
| 0 | No-result/borrowed-entry normalization only; old key and transaction paths |
| 1 | Reuse precomputed branch prefix, bulk-copy runs between embedded zeroes |
| 2 | Cache and fully execute/reset safe rusqlite transaction/savepoint statements |
| 3 | Combine 1 and 2 |

Production Go already computes owner prefixes in Of. The original native
slice recomputed them per call. A new Branch handle retains the original
owners, encoded prefix and packed clear depths. Each call still owns a new
encoded key path. Prepared transaction commands preserve BEGIN IMMEDIATE,
SAVEPOINT, RELEASE and COMMIT; a failed body rolls back to its savepoint and
then the outer transaction. Commit failure attempts rollback and reports
`outcome_unknown`. Full crash/recovery and concurrent grouping are not ported.

`check.py` runs the original 56-command semantics/physical-snapshot oracle,
32 typed cross-read/rewrites, value/page/corruption/TTL/reopen guards, and a
second full trace through no-result Set. Run it for every tuning value.
`write-contract` checks no-result type, exact stored bytes, monotonically
increasing versions, and borrowed SetEntry pointer identity. Two temporary
mutants prove failed-Take rollback and SetEntry ownership regressions fail.

`run.py` compares the original Go and native executables with normalization
and all three ablations in one quiet session, six balanced passes per case.
Every candidate starts with the identical Go-created fixture; checksums and
all mutating physical states must match. New Go/native cases measure borrowed
SetEntry and 64 KiB spilled values; the unchanged old executable has no such
mode and is not invented as a comparator for them. Quiet calibration fixes
equal counts before collection, targeting at least 175 ms for the fastest
candidate. Separate telemetry and phase binaries are excluded from timing
ratios. `strace -c` diagnostics count sync/I/O calls and are not performance
timings. Whole-process GNU time CPU/RSS includes setup, warmup and shutdown.

The synchronous slice retains the original prototype's limits: UTF-8 text
keys only (arbitrary-byte values are exact), Go-created/migrated files, no
directory locks, concurrent grouped commits, process memory reservation,
cancellation, full user Tx/View, background work, custom codecs/panics,
sliding renewal, relaxed counters, configs/watch, once, quota/limiter or cache
runtime. SetIfAbsent is valid only for the single-owner synchronous slice.
Native allocation telemetry excludes SQLite C mallocs; Go and Rust allocation
bytes are not identical accounting domains. WSL2 elapsed/op figures are local
synchronous-loop measurements, not bare-Linux latency or throughput promises.

Reproduce inside the Linux container (repo mounted `/src`, Linux volume `/work`):

```sh
export PATH=/work/cargo/bin:/usr/local/go/bin:$PATH
export CARGO_HOME=/work/cargo RUSTUP_HOME=/work/rustup
export KV_WORK=/work/kv-opt
cd /src/tinystore/kv-opt-bench
python3 build.py
KV_TUNING=0 python3 check.py
KV_TUNING=1 python3 check.py
KV_TUNING=2 python3 check.py
KV_TUNING=3 python3 check.py
python3 guard_proof.py
python3 ownership_proof.py
python3 build.py --metadata-only
# Obtain the shared quiet-host window before any collection/profile.
GOMAXPROCS=1 GOGC=100 taskset -c 2 python3 run.py
python3 report.py
```

Build/check the original executables first if absent, in a separate Linux target
directory. Their expected hashes, this source/lockfile/binary hashes, native
source/compile options and PRAGMA readback are retained. The user waived the
harness-before-collection commit gate; the report labels exploratory uncommitted
collection. No original prototype or production source needs modification.
