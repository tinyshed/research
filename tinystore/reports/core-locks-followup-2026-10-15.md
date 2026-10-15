# Locks with the real grouped writer — 15 October 2026

Both passes of the actual Core admission matrix are complete (24 fresh
processes). Full per-scenario counters, CPU, RSS and tails are in the
[measurement tables](index-foundations-measurements-2026-10-15.md).

This extends [the standalone lease round](locks-2026-10-15.md). The new harness
uses TinyStore's public KV entry/version CAS on the actual Core at
`76b844cdc8c3410e746a5d5dc3e9aafec83c9962`, including its real grouped commits.
It does not copy or substitute the private SQLite group adapter. Product code
and public API are unchanged. Source is
[`index-foundations/src/integration.rs`](../index-foundations/src/integration.rs);
raw measurements are in `data/index-foundations-2026-10-15/locks.jsonl` and
`lease-correctness.jsonl`.

## What is and is not being compared

Both candidates retain a research lease row `[owner, fence, deadline]` in a KV
bucket, read its version, and acquire/release through conditional writes.
Acquisition increments a persistent fence; release clears the owner but retains
the counter. The row is **never TTL-deleted**. This avoids reusing fencing tokens
after expiry, release or restart. TTL is 10 seconds in the throughput workload;
each granted owner holds the resource for 100 microseconds plus scheduler delay.
Every successful acquire and release goes through real Core durability.

Polling retries after 1 ms. FIFO adds admission per resource before the same
Core CAS and releases admission after the lease is released. The deliberately
simple FIFO gate uses a condition variable and `notify_all`, so its wakeup cost
is part of the measurement. It is not an optimized one-waiter notification
queue. At the deadline, queued requests leave without acquiring. Successful
wait samples exclude those canceled requests; cancellations and grants per
client are present in raw data. Jain fairness describes completed grants over
this short run, not a mathematical proof of starvation freedom. With 1000
clients and fewer than 1000 total grants, even a perfect first-in-first-out
first round cannot have Jain fairness 1. Throughput divides by wall time through
all waiter joins, including cancellation drain after the nominal 3-second
window; queue-drain overhead can materially extend that duration.

There are 10, 100 and 1000 clients, a single hot resource or one resource per
client, and two forward/reverse passes. Each client is a synchronous thread with
a 256 KiB stack; this measures a valid Rust Core caller model, not Bun wire
client overhead. Independent resources still share the KV file's writer and
can share a grouped commit. The FIFO gate must therefore be per resource,
never a store-global mutex.

This is a performance candidate for admission, **not a complete correct lease
implementation**. The delayed-writer check below deliberately demonstrates why
public read/CAS calls alone are insufficient to implement short leases.

## Measured admission result

Values are pass 1; pass 2, not averages. With 1000 clients on one hot resource:

| Admission | Completed grant/s including drain | Attempts/grant | Completed wait p99 ms | Jain | Process CPU s | HWM MiB |
|---|---|---|---|---|---|---|
| Polling | 85.87; 88.76 | 592.09; 599.60 | 3085.74; 3045.78 | 0.173; 0.196 | 5.80; 5.75 | 31.09; 31.16 |
| FIFO notify-all | 46.46; 48.42 | 1; 1 | 2977.92; 2963.38 | 0.298; 0.309 | 5.97; 5.97 | 19.81; 20.09 |

Polling completes 269/275 grants in 3.133/3.098 s. FIFO completes 298/309,
but draining canceled waiters extends the run to 6.414/6.382 s. FIFO grants
more owners within the nominal three-second admission window, but total
completion throughput is worse. Its 1000 deadline cancellations must not be
omitted from a fairness/latency claim. A completed-only p99 of about three
seconds is not acceptable evidence of short-wait service.

At ten clients, hot polling is 891/883 grant/s versus FIFO 779/678; FIFO
improves completed wait p99 from 320/335 ms to 31/36 ms. At 1000 independent
resources both need one attempt per grant: polling 16,652/15,511 grant/s and
FIFO 15,853/15,580. Independent resources can share Core commit groups;
serializing them behind a global admission gate would discard that benefit.

These results support removing failed-attempt storms and making admission
per resource. They **do not** prove the hypothesis that a targeted FIFO queue
will be faster: this particular notify-all FIFO is not the implementation to
ship. The next product candidate needs per-waiter notification, bounded queue
removal and a longer run that can serve many full rounds. No extra lock-free
algorithm is justified by these measurements.

## Correctness findings

| Probe | Pass 1; pass 2 | What it establishes |
|---|---|---|
| 3200 concurrent public KV writes | 118; 119 WAL commit markers | The integration really uses grouped commits, about 27 writes per observed commit in this diagnostic |
| Unrelated KV transaction holds writer for 100 ms; acquire requests a 10 ms TTL | Acquire returned after 103.753; 101.503 ms, already expired | Computing the lease deadline before writer admission is incorrect for short leases |
| Jobs transaction held for 100 ms in jobs.db | KV acquire 0.981; 0.664 ms | File-local writers do not wait for that other file's transaction in this probe; not a general I/O-isolation guarantee |
| KV and Jobs handles in one store transaction | Rejected and KV write rolled back, both passes | Separate files cannot be treated as one atomic lease/job transaction |
| Owner process SIGKILL and restart | Fence 1 retained, takeover gets 2, both passes | Lease state/counter survive process death and stale release cannot clear a new lease |
| Successful acquire response deliberately ignored | Owner can be reread; blind retry is not idempotent | Outcome reconciliation needs an owner/request identity; returning a different fence on a blind retry is not safe retry semantics |
| Renew via version CAS | Owner and fence retained, competitor refused | Basic CAS ownership protection, not a proof of renewal after TTL or under all commit delays |

The grouped-commit diagnostic is separate from throughput timing. It pins a
read snapshot to prevent WAL recycling and counts physical WAL commit markers;
that changes checkpoint behavior and is not used to claim ordinary peak WAL.
SIGKILL recovery includes waiting for the old 100 ms TTL; it does not mean an
owner is declared dead before its deadline. These probes use the default system
clock, not arbitrary custom TinyStore clocks or simulated power failure.

The delay probe holds an earlier writer transaction: it measures writer-admission
delay, not an injected slow fsync inside the acquire COMMIT. Even with the
clock evaluated inside the writer, COMMIT or response delay may consume the
remaining TTL. Return the deadline and specify an already-expired/unknown
outcome; do not promise a full TTL starting at receipt. Fault-injected fsync
and delayed-response histories remain explicit native acceptance tests.

Version CAS protects the row from concurrent replacement, but does not evaluate
`expires_at > now` on the writer at execution time. A request can read a free
row, compute a deadline, then wait behind another transaction and commit an
already-expired grant. The product must evaluate lease predicates and deadlines
inside the writer operation, return the actual deadline, and define the outcome
if commit/response latency consumes the lease's usable lifetime. Increasing a
benchmark TTL hides the bug; it does not fix it.

The stale-release check is not proof that a protected external resource rejects
stale writes. Fencing only protects that resource when it atomically compares
the token with its last accepted token. A local lease cannot elect a replicated
primary; distributed locks and failover fencing are outside this round.

## What to reuse from Jobs

Reuse the common writer, clock, deadline indexing, error vocabulary and the
notification abstraction. Inspect Jobs' ownership checks and queue occupancy
for useful SQL patterns. Do not share its lifecycle unchanged: startup cleanup
of process-owned job leases, job attempts/retries and a client's renewable
resource lease are different contracts. Job lease renewal also needs a deliberate
live-TTL predicate before it can be reused for this API.

`store.bucket` and `store.queue` use kv.db and jobs.db respectively. If an
application requires lease state, application rows and a job enqueue in one
transaction, co-locate their handles in the same application SQL database or
design an explicit outbox. Merely using the same Store does not make files
atomic together. Cross-file contention shares CPU/disk even when writer locks
are independent.

A concrete source boundary: Jobs [`EXTEND_LEASE`](https://github.com/tinyshed/tinystore/blob/76b844cdc8c3410e746a5d5dc3e9aafec83c9962/crates/tinystore/src/jobs/claim.rs#L52)
checks identity/attempt, without `until > now`. That can be a valid Jobs policy
until another claim takes ownership; it is not the proposed resource-lock
contract that forbids reviving an expired lease. Jobs also computes its claim
deadline before submitting the grouped write. Reuse infrastructure and SQL
patterns, not those TTL semantics without a deliberate decision.

## Recommendation for Opus

Keep SQLite authoritative, with one atomic writer operation per acquire, renew
or release. Retain a resource fence counter independently of ephemeral owner
rows. Check ownership, live TTL, capacity and fence under that writer operation;
renew must not resurrect an expired lease. Use bounded per-resource FIFO
admission as a policy candidate and a one-waiter wake mechanism, with a periodic
deadline/recheck fallback. Do not implement the prototype's notify-all gate as
the default without its own contention gate.

Cancellation needs a state transition: queued requests can be removed or
tombstoned; once grant COMMIT succeeds, cancellation/lost response is an unknown
outcome until the request is reconciled or the exact owner/fence is released.
Registering a waiter and rechecking the row must close the missed-wakeup race.
An expiry timer is only a hint; the SQL predicate remains authoritative. A
restart rebuilds waiter notifications from current demand, not a persisted
in-memory queue.

Persistent per-resource fences retain metadata for every resource name ever
used. They cannot be TTL-collected and silently restart at zero. An alternative
is one durable monotonic sequence per SQLite file, each grant taking the next
token. That bounds counter metadata while preserving increase per resource. Its
extra hot-page/write cost is unmeasured; it is a candidate, not the measured row
layout. Either design needs overflow/restore epoch semantics and protection
against ordinary bucket clear or user SQL resetting counters.

Fences must survive SDK encoding exactly. Rust `u64` does not justify a Bun/JS
`number`: values above 2^53 need bigint or an opaque lossless representation.
This Core-only row prototype does not test the future public lease wire codec.
Include 2^53 boundaries and counter exhaustion in that codec acceptance gate.

Semaphores should share the resource ownership/fencing/deadline machinery, but
weighted permits and FIFO head-of-line versus bypass behavior require an explicit
contract. This round does not claim a real-Core semaphore implementation or
throughput measurement; the earlier prototype's capacity checks remain the
available evidence. Short-lease correctness takes priority over maximum grant/s.

## Owner decisions and acceptance gates

Agree whether waiting is FIFO, best-effort or configurable; cancellation after
grant; maximum TTL and clock semantics; renew-after-expiry refusal; persistent
fence type/overflow; idempotent acquire request identity and its retention;
whether a lost response can be reconciled by API; semaphore weights/fairness;
and which database owns a resource that must participate in application/Jobs
transactions. No public API is changed here.

The product must pass randomized mutual-exclusion histories, cancel at every
admission/commit/response boundary, TTL and custom-clock jumps, renew at/beyond
expiry, delayed and failed commits, response loss/retry, SIGKILL/restart,
fence enforcement at a protected resource, waiter cleanup and bounded RAM.
Repeat hot/spread tests under actual grouped commits and active Jobs writers,
through supported SDK transports and OSes. A fair queue that wakes a thousand
threads for one grant can be correct and still be an unacceptable implementation.
