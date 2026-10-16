# Channels with real receivers — 15 October 2026

Both passes of the transport matrix are complete: 108 fresh Bun processes.
See [per-pass tables](index-foundations-measurements-2026-10-15.md) for every
scenario, memory delta, workload CPU, daemon CPU and idle CPU sample.

This extends [the first channel round](channels-2026-10-15.md), whose in-process
publisher rates did not establish delivered SDK throughput. Source is
[`channels-wire-lab`](../channels-wire-lab/README.md); raw runs are in
`data/index-foundations-2026-10-15/channels.jsonl`.

## Integration boundary

There is no native Channels router or SDK Channels handle at the pinned Rust
revision. This round does not modify Core, generated protocol or public API.
A research adapter reserves private methods 60000–60005 around a real Core
Pipe. HELLO/WELCOME and ordinary traffic use unmodified Core; experimental
subscriptions, log dispatch, CREDIT scheduling and channel frames belong to
the adapter. The receiver uses unchanged Bun SDK Session, Connection, frame
parser, stream lifecycle and Bun runtime sockets. Embedded uses a research FFI
wrapper; sidecar means the adapter's Unix socket. Production SERVE discovery,
sidecar startup/proof, TLS/auth and a future public Channels iterator are not
included. The experimental event envelope is a minimal binary codec; the cost
of a future generated Channels MessagePack/JSON codec is not included.

Ordinary publications are volatile. A separate `commit` profile follows a real
FULL Core KV write for each event. Ten deliberately rolled-back Core transactions
in every profile must produce no events. The adapter serializes
commit completion and sequence assignment; it **does not** measure a native
grouped-commit notification hook. It polls connections at 1 ms; that scheduler's
CPU/latency is part of the prototype result, not the recommended production
path. `Connection::take` also holds the shared Hub state mutex while draining
and framing a connection budget. That imposed critical section constrains both
candidates and does not measure unconstrained parallel broadcast receivers.
These limits are material when interpreting the numbers as Core →
experimental Wire boundary → real Bun receiver, rather than an existing product
Channels feature.

## Compared paths

Both candidates retain at most 256 shared 1024-byte Arc payloads. Baseline is
`tokio::sync::broadcast`; candidate is a simple mutex/VecDeque log with cursors.
No new lock-free algorithm is built. Both have identical framing, per-stream
send allowances, output scheduling and Core commit path. Fast runs offer
1000 publishes at 200/s, with 100 or 1000 subscriptions, multiplexed over one
connection or one connection per subscription. An unpaced `burst` profile offers
2000 events with 32 publish calls in flight. Thus the matrix separates
subscription overhead from transport/session overhead.

In the slow/credit scenario, 1% of subscriptions voluntarily start with zero
send allowance. After 1200 events at an offered 400/s, real Wire CREDIT resumes
them. The bounded log has already overflowed, so they receive a gap followed by
the retained tail. This measures absent credit and resynchronization, not a
particular slow socket's bandwidth. A separate `slow-window` profile pauses 1%
of real SDK consumers for 2600 events, exhausting their ordinary 2 MiB windows,
then resumes them. Filling all 1000 windows is a different multi-GiB workload;
a small shared log does not make it free.

Latency is measured on one Bun monotonic clock from payload timestamp before
the publish call to actual receipt/decoding, sampled every 16 delivered events.
It includes scheduling/transports and, in the `commit` profile, the Core commit.
It excludes time before
the publisher actually starts that call. `publishedPerS` measures achieved rate,
which can fall below the offered schedule. Delivered counts, missed gap counts,
wire-sent counts and per-subscriber counts are separate. Undelivered-by-drain
deadline must not automatically be called confirmed permanent loss.

Client RSS/heap and CPU are measured in Bun; daemon RSS comes from the actual
server process and its CPU from `/proc`. Daemon CPU includes connection setup
and cleanup, whereas client CPU counters cover publish/drain, so those windows
must not be summed and labelled one precise steady-state CPU rate. Embedded
Core and client share a process; counting its RSS twice would be wrong.

## Measured result

At 1000 subscriptions over one connection, 1000 volatile publications at 200/s
have 1,000,000 expected logical deliveries. Values below are pass 1; pass 2:

| Transport | Algorithm | Delivered by drain | Receipt p99 ms |
|---|---|---|---|
| Embedded | broadcast | 980,395; 1,000,000 | 683; 555 |
| Embedded | shared log | 932,321; 1,000,000 | 972; 566 |
| Unix sidecar adapter | broadcast | 1,000,000; 1,000,000 | 337; 127 |
| Unix sidecar adapter | shared log | 1,000,000; 1,000,000 | 69; 131 |
| TCP loopback | broadcast | 868,423; 1,000,000 | 1109; 93 |
| TCP loopback | shared log | 1,000,000; 1,000,000 | 66; 316 |

Every 1000-connection fast case delivers all 1m events, both algorithms and
passes. Embedded p99 there is 9.80/7.94 ms for broadcast and 7.41/8.31 ms for
the shared log. Unix/TCP tails are much more variable; they cannot be averaged
with embedded or one-connection results. The scheduling/transport difference
is larger than a stable backend winner. Two passes do not establish statistical
significance or long-run delivery capacity.

An unpaced embedded burst at 1000 subscribers publishes 2000 events (2m expected
deliveries). Broadcast publishes at 23,612/18,089 per second but delivers only
115,478/109,659 by drain; the shared log publishes at 20,591/22,019 and delivers
114,719/113,960. Both report 1,728,863 confirmed missed events. High publication
rate here principally measures successful dropping under overload.

At 400 publications/s, slow/no-credit runs also overload the fast consumers
through the adapter's finite scheduling path. The results are not solely the
1% slow subscribers' losses. `slow-window` does exercise a real unread SDK
window, but is not a bandwidth-limited external network experiment. The
separate post-commit profile can also miss the drain deadline; completion of
a durable data write does not imply timely delivery of its notification.

For embedded multiplexed broadcast at 1000 subscriptions, the subscription RSS
delta is 6.87/7.81 KiB per subscriber; with one connection per subscription it
is 17.81/18.84 KiB. At only 100 subscriptions fixed setup dominates that division,
so it is not a constant object-size estimate. Embedded 1000-connection idle
samples consume 54.8/47.1 ms of CPU in about 200 ms: copying this polling loop
would impose material idle cost. Full server/client memory and separate CPU
windows are retained in raw output and tables, without double-counting embedded.

All 108 runs report zero ordering errors, zero active subscriptions after cancel,
no replay to a fresh subscription and no event for ten rolled-back transactions.
The comparison supplies no consistent across-scenario advantage large enough to
justify replacing standard broadcast with a custom log as the default. The log
remains a simple alternative if a future native route demonstrates a concrete
benefit after fixing scheduling and global buffer admission.

## Correctness interpretation

Each run checks total committed publishes versus rollback attempts, per-stream
monotonic delivery, cancellation/removal of every subscription, and a fresh
connection receiving no replay. Gaps count only overflow visible within the
live subscription. Reconnection has a new tail and cannot count events lost
while disconnected. A restart/epoch reset is an API contract still to be
implemented in a native module; reconnection here does not prove a production
reset event format.

Cancellation removes subscriber state, but the shared-log prototype retains its
last 256 payloads after the last subscriber leaves; broadcast can release
consumed slots. This is bounded for the one measured topic, not an idle-topic
reclamation policy. Product topic registries must reclaim empty topics and
account for their retained bytes.

A gap itself cannot be delivered without credit. The server must coalesce
missed ranges/counters in bounded state until the subscriber resumes; enqueueing
one gap for every missed event would defeat bounded memory. The prototype
implements this through broadcast lag state or cursor distance, not an
unbounded per-subscriber event queue.

No slow subscription may stall publisher progress or grow the log beyond its
bound. Per-connection output, kernel buffers, SDK DATA copies and application
retention are additional buffers. The measured one-versus-many connections is
therefore as relevant as the core ring's payload storage. Millions of successful
publishes with very few deliveries are a loss policy measurement, not useful
fanout throughput.

The shared history holds 256 KiB payload, data-drain budget is 256 KiB per
connection and initial stream credit is 2 MiB per subscription. At 1000 streams
the initial permissions sum to 2000 MiB; 1000 per-connection drain budgets sum
to 250 MiB. These are not a proof of a hard aggregate output cap: private unary
replies/cancel END frames append outside the data scheduling check. The bench
uses a well-behaved SDK with bounded calls in flight. Native admission must
include those control buffers too. Credit is permission, not preallocated RSS. RSS measures resident
user-space buffers, not socket kernel memory or a complete instantaneous sum of
SDK/transport queues. This adapter does not expose every queue high-water
counter; an exact aggregate-buffer admission proof remains a native-path gate.
The fixed polling/output budget also constrains delivery capacity. These rates
are not a limit of the future event-driven implementation.

## Recommendation for Opus

Start with standard broadcast unless a native-path delivered-throughput/RSS
gate demonstrates a material shared-log advantage. Keep one immutable payload
per event; bound by bytes as well as event count; cap message size, topics,
subscriptions and total connection send allowance. Wake/drain per connection
with coalescing and a fair byte budget. Do not copy the adapter's polling loop
or serialize every production publish behind an extra global commit mutex.
Clone/reserve a payload under the necessary broker synchronization, then release
it before connection framing/network work; independent connections should not
share that whole critical section.

Attach staged publications to successful savepoints and commit completion in
the real writer. Drop publications on savepoint rollback or failed group COMMIT.
Assign sequence/order at an explicit commit-dispatch boundary; waking a caller
and publishing when it resumes can reorder events from separate completed
commits. A commit hook must remain bounded and cannot wait for slow network
consumers. Process death after data COMMIT but before ephemeral dispatch can
still lose a notification; that is compatible with at-most-once Channels and
must be documented.

A separate **unmeasured** candidate reduces the measured wire amplification:
coalesce identical topic subscriptions within one SDK
connection into one Wire stream, then fan out a shared immutable payload through
bounded local subscriber cursors. At 1000 duplicate subscriptions, the current
fast fixture sends 1m payload frames for 1000 publishes (about 1024 MB payload);
one connection-level subscription would send 1000 frames (about 1.024 MB).
That is a 1000× theoretical wire-byte reduction for this special case, not a
measured throughput gain. Application callbacks still fan out 1000 ways, and
1000 distinct remote connections still require 1000 network deliveries.

Start evaluation with exact-topic reference counting; overlapping path filters,
authorization/codec options, local slow-consumer gaps and cancellation semantics
need a deliberate contract. A connection feed plus SDK selector routing is a
broader protocol candidate, not automatically the preferred design. Measure it
against the unchanged per-stream baseline before adoption. Shared local payloads
and bounded cursors must not turn one slow iterator into connection-wide stalls
or move the unbounded queue into the SDK. No such SDK/public API change is made
by this round.

Guarantee an order within a named channel/defined writer scope. Separate SQLite
files have no implicit atomic transaction or universal commit order. If path
subscriptions are wanted, begin with an owner-approved exact topic or prefix
model; arbitrary glob/regex matching and authorization are not proven cheap by
this log benchmark. Clear unneeded channel state when the last subscriber goes,
and bound the registry as well as individual queues.

For UI recovery, subscribe and await acknowledgement **before** fetching a
snapshot, then reconcile versioned changes; snapshot-then-subscribe has a race.
On gap/reset reconnect, reload authoritative state. Channels do not become a
durable replay system by accepting a last-seen cursor. Durable history, if
required later, belongs to a separately specified Records-based stream with
Channels as a wakeup hint.

## Concrete writer seam (source inspection, not native-path measurement)

In pinned [`Group::lead`](https://github.com/tinyshed/tinystore/blob/76b844cdc8c3410e746a5d5dc3e9aafec83c9962/crates/tinystore/src/sqlite/group.rs#L175),
`commit` returns, the writer mutex is dropped, leadership is handed off, then
`answer_all` answers callers. [`answer_in_halves`](https://github.com/tinyshed/tinystore/blob/76b844cdc8c3410e746a5d5dc3e9aafec83c9962/crates/tinystore/src/sqlite/group.rs#L341)
deliberately propagates synchronous caller answers through a wake tree, not
FIFO wake order. A resumed caller or generic completion callback therefore is
not a commit-order sequencer. The proposed native seam is bounded staging per
savepoint, promotion only for successful writes, then sequence assignment and
bounded dispatch-queue append after successful outer COMMIT while writer order
is still owned. Network draining happens later. Account this extra queue in
global memory admission; it must not become unbounded to preserve ordering.
This is an implementation proposal that needs the native failure/order tests,
not a claim the adapter measures such a hook.

## Owner decisions and product acceptance gates

Agree channel naming/path matching and authorization, event count/byte limits,
drop/disconnect policy, gap range/count and epoch reset format, scope of ordering,
transaction publication semantics, subscription acknowledgement/snapshot recipe,
and whether SDKs ever expose replay. This round changes none of them.

The native implementation must pass rollback/savepoint/group-commit failure and
ordering tests, empty/no-subscriber cleanup, cancel/disconnect while blocked for
credit, gap after multiple overflows, restart/reset, bounded queue/SDK/kernel
buffers and fair multiplexing. Measure actual delivered throughput and p99 at
100/1000 subscriptions through the real public SDK handle over embedded,
production sidecar and TCP/TLS, with a no-credit consumer and a real bandwidth
limited socket. Those remaining gates are explicit; this adapter is evidence
for algorithm selection, not a claim that the product integration is complete.
