# Sustained WAL capture: experiment plan — 15 October 2026

The owner asked to optimize capture under sustained mixed load before building
snapshots, retention and a persistent sender. This round keeps those later
steps out of scope and preserves every previously measured source revision.
The product seam changes live only on `experiment/wal-load`; research is main.

| Question | Mechanism/control | Evidence required |
| --- | --- | --- |
| Are repeated dirty ancestors expensive? | Same SHA-256 roots, batched dirty leaves and unique ancestors versus one ancestor walk per page. | Equivalent roots through growth/shrink, hash counts, stage times, matched throughput/latency. |
| Are packet copies expensive? | Keep page ownership and encode directly into a pre-sized frame versus encode, copy and decode back into owned pages. | Same versioned records readable by recovery; known payload copy bytes and encoding time. |
| Can history remain bounded in RAM? | 8 MiB conservative cache reservation, on-disk journal remains authoritative; sequence/cursor independent of cache eviction. | RSS over time, cache reservations, recovery without a retained seed and with history beyond the cache. |
| Is sync or checkpoint the writer's limit? | Separate feed-sync/checkpoint timers, FULL feed control, nonblocking PASSIVE versus TRUNCATE, independent strace by file descriptor. | No durability reduction; isolate component controls and quantify native SQLite WAL sync separately. |
| What happens when a reader pins WAL? | Admission before BEGIN, reserve for the next bounded transaction, sync covered feed before checkpoint, nonblocking attempt at watermark. | Definite Unavailable before mutation, no dropped ACKs, reader release permits retry. |
| Does this strictly cap arbitrary SQL transactions? | No: native WAL growth is known after COMMIT. A transaction exceeding its declared reserve is OutcomeUnknown and stops the writer. | Enforced transaction envelope or VFS write admission before a product hard byte-cap promise. |

## Harness and invariants

[wal-load](../wal-load/README.md) is a separate Rust crate copied from the
preceding WAL experiment. The original scanner/WAL reports remain reproducible.
All comparisons use SQLite FULL, identical fixtures and the same instrumented
binary. The legacy control preserves per-page hashing, codec round-trip,
resident history and TRUNCATE, but shares instrumentation and admission and
raises the feed disk guard to 2 GiB for sustained work. It is not the exact
binary from the short round; those older throughput numbers are not controls.

Each case is a new process: 32768 rows, 256-byte bodies, 64 concurrent writers
on 32 hot keys, eight readers over the full file, including 128-row aggregates,
and a periodically held reader. Warmup is outside timing. Final version sums
must equal successful writes, source image must match feed root and SQLite
integrity_check must pass. Histograms have fixed memory; samples include RSS,
HWM, WAL length, feed size, cache charge, stage counters and per-second progress.

Main controls will run for 60 seconds with three rotated passes, with shorter
component-isolation controls and a longer optimized soak. Heavy measurements
start only after committing the source pin and harness; no builds/tests/lint
may overlap timing. Raw intervals, exact revisions, environment and commands
will accompany the final report. Trace diagnostics are separate from rate runs.

The cache charge is a conservative accounting estimate, not a native allocator
hook or whole-process cap. SQLite caches, tree/scratch and process RSS are
reported separately. Native WAL admission uses an 8 MiB watermark and a 1 MiB
transaction reserve in this bounded workload. The feed disk guard refuses
before BEGIN; the journal is not deleted to make room. Retention is still a
later task, so an exhausted disk budget must eventually backpressure writers.

The experimental product seam is pinned at `31b10e4` on `experiment/wal-load`.
Default and capture-enabled core checks pass locally; the Docker actionlint
wrapper cannot see this checkout, while local actionlint passes. Timing results
and CI status will be recorded in the measured follow-up.

The [measured follow-up](scaling-wal-load-2026-10-15.md) records the completed
controls, raw data, bounds and implementation handoff.
