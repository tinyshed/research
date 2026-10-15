# Sustained WAL capture research

This follows `../wal-spike` without editing its measured revisions. Product
seams are isolated on `experiment/wal-load`; this crate and dated reports land
on research/main. No snapshots, retention or persistent sender is implemented.

```
./bootstrap.sh
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
TMPDIR=<scratch> target/release/tinystore-wal-load optimized 60 100
```

Arguments: mode, measurement seconds, long-reader hold milliseconds (0 disables).
Each process prepares 32768 rows with 256-byte bodies, warms for 2 seconds,
then runs 64 writers on 32 hot keys, 8 readers on the whole database (1/64
queries aggregate 128 rows), and one reader holding a snapshot for the selected
interval, then pausing for 2 seconds. All modes use SQLite FULL. Update counters, source
Merkle root and SQLite integrity_check are verified after drain. Native startup,
seed and warmup are outside timing. Samples are emitted each second.

Modes isolate: off; legacy (per-page ancestor hashing, encode/decode copies,
full history, TRUNCATE checkpoint); bounded (8 MiB cache); hash (batched dirty
ancestors); copy (owned packet encoding); combined (both); optimized (combined
plus PASSIVE checkpoint with frame-stride retries); full (optimized plus feed sync per group). These are
mechanism controls in a common instrumented harness, not identical binaries
from the previous short round. The legacy feed cap is raised to 2 GiB in all
modes; memory grows in legacy by design. Feed eviction removes only RAM cache,
never history on disk. Sequence and native cursor are independent of cache.

Stage timers include native WAL read/checksum/copy, dirty tree hashing,
encoding/copies, feed SHA, file append, feed sync, checkpoint and admission.
They are wall times under the writer, not CPU attribution. Native SQLite sync
needs a separate strace diagnostic. Known copied-byte counters count explicit codec
payload copies (three vs one), not WAL scratch, allocator/library or kernel copies. Histograms use 1 us buckets
below 4096 us and 4096 us buckets above, saturating at about 1 second; reported
quantiles are bucket lower bounds, and not appropriate for precise tail claims.

Capture cache reservation: 8 MiB, with conservative page/map metadata charges.
Tree capacity and cached payload capacities are reported separately. Process
RSS/HWM and cgroup usage include other allocations and page cache respectively;
this is not a complete production memory-budget implementation. Images remain
bounded at 16 MiB and feed at 2 GiB, with admission reserve before BEGIN.

WAL admission watermark: 8 MiB, reserving 1 MiB per transaction/group. The
workload's transactions fit this envelope. At the watermark the observer syncs
feed and attempts nonblocking TRUNCATE; a pinned reader causes a definite
Unavailable before BEGIN. Retry can resume after reader release. A transaction
exceeding its declared reserve is detected *after* native commit, produces
OutcomeUnknown and stops further writes. Therefore this is not a strict byte
cap for arbitrary application SQL/spill behavior; that needs a VFS write bound
or a separately enforced transaction-size contract. No committed WAL is dropped
or recycled to satisfy the limit. Long-reader and disk-pressure tests distinguish
these cases. Every checkpoint follows feed sync.

The nonblocking policy retries checkpoint at a configured frame stride rather
than on every commit once a reader has pinned the threshold. Writer retries
back off 1 ms after a definite admission refusal; refusals are counted.


Additional control: `nocache` keeps no historical packets in RAM. Recovery still
uses the same durable feed and canonical roots. No sender consumes this cache
yet; compare it with the bounded cache before deciding what a persistent sender
should retain. This control does not remove hashing, serialization or sync.
