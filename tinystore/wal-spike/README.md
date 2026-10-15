# WAL capture / crash recovery research

Standalone Linux experiment, not a TinyStore replication API. Product changes
live on `experiment/wal-capture`; research lives on `main` here. See
[the architecture note](../reports/scaling-wal-design-2026-10-15.md).

```
./bootstrap.sh
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo build --release --locked
TMPDIR=<disk-backed-scratch> target/release/tinystore-wal-spike bench
TMPDIR=<disk-backed-scratch> target/release/tinystore-wal-spike network
```

`source-revision` pins the product worktree; `source/` is ignored. The old Go
submodule at `../source` stays untouched. SQLite's research-only `dbpage`
compile flag is needed for verification and page application, not capture.

The capture checks native WAL salts and rolling checksums and emits complete
commits before the caller's answer. `full` syncs the feed each commit;
`guarded` retains the already-synced SQLite WAL until the feed is synced before
checkpoint. Both require SQLite FULL. Only writes through the observed
TinyStore writer are in scope: another native writer/checkpointer is forbidden.

The in-memory history is capped indirectly at a 256 MiB feed, images at 16 MiB,
and compressed batches at 32 MiB / 1024 records. There is no retention/compaction,
authentication, encryption, automatic promotion or fencing implementation here.
`epoch=1` is fixed. Receivers are dedicated read-only copies, opened for queries
through fresh handles. Per-file ordering does not create cross-file transactions.

`bench` rotates off / original snapshot capture / full / guarded for three
passes at 4096 and 32768 deterministic 256-byte rows and 1 / 64 callers. The
snapshot baseline is frozen from research `4d95f68`, with module paths and
visibility adjusted only. Its explicit scanner must serialize write+capture;
that prevents grouped writes and is part of the old design's cost. Latencies
include its serialization wait. The new hook runs once per SQLite group commit.

`network` uses persistent loopback TCP_NODELAY, one and three actual TCP hops,
1 / 32 commits per compressed batch, and durable vs applied ACKs. Each relay
forwards the exact compressed input after durable receipt, before page apply;
compression runs outside the source writer. Echo traverses the same connections.
The optional 5 ms delay in each direction is application-level delay injection,
not a WAN or bandwidth simulation. Batch latencies start with the first write;
there is no automatic timer or source notification worker in this harness.

Process-death workers exit without destructors. Tail truncation models loss of
unsynced feed bytes while preserving synced SQLite WAL. These are not power-cut
or disk-controller tests. A separate test deletes the primary only after the
standby has synced its feed, then rebuilds its searchable SQLite copy.

Follow-up commands address two risks exposed by the first run:

```
TMPDIR=<disk-backed-scratch> target/release/tinystore-wal-spike grouped
TMPDIR=<disk-backed-scratch> target/release/tinystore-wal-spike pipeline
```

`grouped` uses 4096 updates / 64 callers (three rotated passes of off, full,
guarded), instead of the exploratory 256-update runs. The serialized snapshot
baseline is retained in the first round; it is not rerun at this longer depth.

`pipeline` isolates three schedules with durable ACKs through three TCP hops:
forward after local sync and apply on the network thread; forward after sync
with a separate applier; forward after append with a separate applier and sync
in parallel with downstream traffic. Every schedule waits for local sync and
the downstream ACK before ACKing upstream. The applier's queue has eight batch
slots, shares immutable history entries and collapses up to eight queued batches
into one SQLite apply. Backpressure is explicit. The final images and integrity
checks run after draining the appliers; durable latency is not visibility latency.

`checkpoint` is a threshold-only follow-up: three rotated passes of off,
guarded at 256 KiB dirty feed, guarded at 4 MiB, 4096 updates / 64 callers.
Both capture variants still sync the feed before any checkpoint and keep the
same independent 1024-native-WAL-frame trigger. This changes neither SQLite
FULL nor the acknowledgement contract; it changes retention/checkpoint work.
