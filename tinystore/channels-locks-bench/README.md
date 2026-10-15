# Channels, leases and index cost probes

Standalone research prototypes. No product code is imported or modified.
The architecture reviewed is TinyStore `76b844cdc8c3410e746a5d5dc3e9aafec83c9962`
on `rust`. This module deliberately does not move the shared Go-era source
submodule: unrelated historical Go harnesses still build against its pin.
The owner explicitly requested publication now to research/main while the
product work stays on a branch from rust; that supersedes the old measure
skill's wait-for-product-main instruction for this round.

The Rust prototypes live here because the owner requested algorithms and
measurements as an implementation reference. They are not a product Rust
implementation and do not change an accepted API. The prototype's lease
schema and Tokio actor are distinct from TinyStore's KV and grouped writer.

## Reproduce

Use Linux, Rust 1.99.0, the locked dependencies and a disk-backed scratch
directory. No build, test or lint may overlap timing. The measured image is
`rust:1.99-bookworm`; the report records its immutable digest.

```sh
cd <repo>/tinystore/channels-locks-bench
cargo build --release --locked
cargo fmt --check
cargo clippy --release --all-targets --locked -- -D warnings
```

`run.py` dispatches one fresh process at a time into a running Docker
container named `channels-locks-lab` (override with `BENCH_CONTAINER`).
Mount the research repository at `/src`, the release target at `/target`,
and a **disk-backed** named volume at `/data`. Set the container working
directory to `/src/tinystore/channels-locks-bench` for SQLite build flags.
Give the container two CPUs and six GiB; the outer VM in this round has
a two-CPU quota and eight GiB. Preserve configured proxy/CA trust for builds.

```sh
python3 run.py preflight <output>/preflight.jsonl
python3 run.py kill <output>/kill.jsonl
python3 run.py channels <output>/channels.jsonl
python3 run.py resources <output>/resources.jsonl
python3 run.py leases <output>/leases.jsonl
python3 run.py lease-controls <output>/lease-controls.jsonl
python3 run.py indexes <output>/indexes.jsonl
```

`preflight` is calibration and correctness, excluded from comparison tables.
Every timed family alternates or rotates algorithm order on the same input.
JSONL records command arguments, UTC start/end, exit code and results. Never
overwrite a raw evidence file. Each case removes its SQLite files after
measuring live WAL and per-object dbstat. The kill suite kills the actual
owner binary with SIGKILL at a flushed handshake, then reopens its database.

## What is being compared

Channels use one 64-byte payload allocation per publication, shared through
Arc, capacity 1024, one publisher and two Tokio workers. Consumers touch one
payload byte, check increasing sequence numbers and sample delivery time
every 64 sequences, offset per subscriber. No serialization, socket, TLS or
SDK is present. The three implementations are Tokio broadcast, a mutex
ring with per-reader cursors and race-safe Notify registration, and bounded
MPSC queues using try_send and drop-new on overflow. Ring/broadcast keep the
newest tail; queues keep the oldest backlog. Queue sequence gaps cannot
report a lost tail without an independent publisher watermark. The check
demonstrates this limitation; queue throughput is not a complete gap protocol.

`saturate` is an intentionally open-loop burst with publisher yields every
32 events. `slow` pauses every tenth subscriber for 2 ms per 32 deliveries.
`paced` targets 1000 publications/s using Tokio timer deadlines; this is a
light-load/overload probe, not a precision traffic generator. `paused`
defers every consumer until after publishing, for live backlog RSS. Sizes
64 B and 64 KiB share the payload, never copy it per recipient. Retention is
count-bounded here: a product also needs byte admission and connection limits.
Delivery percentiles include only received events, and must be read beside
loss counts. Publish/s alone cannot select a fan-out implementation.

Routing compares a linear list scan with a segment trie for 100 tenant
prefixes plus N exact subscriptions. Every lookup matches two entries;
results are consumed. Both structures are built in both modes, so their RSS
is not a memory comparison. A trie node is one slash-delimited segment;
`tenant/1` must not match `tenant/10`. There is no wildcard/regex algorithm.

Leases run 10/100/1000 async clients through one SQLite writer thread. Hot
cases hold one resource (or eight semaphore permits) for a 2 ms Tokio sleep;
spread cases use one resource per client with no artificial hold. All modes
use a 1 s TTL, integer writer execution time, persisted per-resource fencing
counter, owner+token checks, and FULL WAL unless explicitly NORMAL. The
writer drains at most 64 queued requests per transaction; this is a fixed
batch control, not the product's adaptive grouped-commit algorithm.

- `kv-poll`: JSON value containing counter and slots, get/check/set inside
  the same writer transaction, 1 ms timer retry.
- `sql-poll`: conditional UPDATE RETURNING plus fence counter, same retry.
- `hybrid`: the same SQL, generation/Notify plus a 10 ms TTL fallback;
  release broadcasts to all waiters. This deliberately tests herd wakeups.
- `admitted`: the same SQL preceded by a per-resource FIFO Tokio semaphore;
  only eligible clients submit acquire calls. This is an admission/wakeup
  prototype, not a complete external-owner scheduler. A production waiter
  manager must transfer reservations on cancellation and owner TTL expiry.

Latency is reported separately for end-to-end acquisition (successful
clients only) and all acquire request round trips. Fairness reports clients
with no success; queued admission deadlines are not counted as SQL attempts.
Counters include read-only failed transactions as commits, not as disk writes.
`mutations` counts changed rows, and WAL bytes measure actual WAL growth.
No allocation-free, distributed lock, HA fencing, power-loss or multi-host
claim follows from these probes.

Indexes use 100k deterministic synthetic rows, FULL WAL, batches of 1000,
FTS5 external content with maintenance triggers, R*Tree with triggers,
and a plain table baseline. Measurements include insert, 10% update/delete,
1000 queries, WAL before checkpoints and dbstat after insertion. The FTS
query has a plain LIKE control; the R*Tree query has a different predicate
and must not be compared with the LIKE latency as a spatial speedup.
Rebuild creates a second index in one writer transaction while a reader
pins the old snapshot. It proves read continuity and writer blocking, not
an online chunked rebuild. A subsequent 1000 singly committed inserts
check dual-index maintenance. The report proposes the missing delta/switch
algorithm and identifies its unmeasured gates.
