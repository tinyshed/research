# Additional research handoff — 15 October 2026

**Publication in progress.** Static index comparisons, the 108-process transport matrix and the 24-process
actual Core admission matrix are complete. Concurrent rebuild and additional
resource/recovery diagnostics are still being completed. This
document will identify their final evidence before calling the round complete.
Product Core and public API are unchanged. The pinned Rust source is
`76b844cdc8c3410e746a5d5dc3e9aafec83c9962`.

## Decision matrix

| Area | Recommended implementation direction | Why / evidence boundary | Cost and unresolved gate |
|---|---|---|---|
| SQL secondary indexes | Existing SQLite B-tree, partial/expression indexes | Matched range/equality workload beats relational postings; no separate engine needed | Small: document recipes and verify supported SQLite features |
| Cross-engine Index Engine | Do not create before v0.1.0 | KV codecs/expiry, Records packed identities/retention and Metrics label postings have different contracts | New universal API is unjustified; obtain concrete query requirements first |
| SQL full text | Optional same-file FTS5 projection; choose positions/detail from query semantics | Compact FTS versus Tantivy Basic has similar term-set query latency and lower measured space/RSS | Moderate: ownership/normalization, generation lifecycle, budgeted maintenance and crash gates |
| SQL geo | Evaluate same-file RTree with exact f64 semantics; selective queries and broad results need different plans | Outward float32 candidates need refinement; clustered broad results expose expensive source lookups | Moderate: validated coordinates, rectangle/radius contract, adaptive/refined query and lifecycle gates |
| Rust text index | Defer default adoption; keep explicit external-outbox candidate | Independent durable commit cannot silently inherit SQLite snapshot semantics | Large: watermark/barrier, merge/disk budget, replacement/cleanup, filesystem recovery |
| rstar / Roaring | Optional bounded rebuildable caches when a workload pays for RAM | Fast in-memory result sets do not establish durable update/source atomicity | Moderate to large: identity mapping, cache coherence, eviction/startup and concurrent writes |
| Online rebuild | Immutable generations with same-file atomic maintenance; choose measured keyset policy | Frontier, eager seen IDs and dirty-ID repair are compared rather than assumed equivalent | Moderate to large: total writer holds, final tail, old readers and space reclamation |
| Channels | Standard broadcast as initial candidate; shared payload, byte limits and credit-aware fair drain | A bounded log does not bound SDK/connection/kernel memory; real delivered events are the useful metric | Moderate: native commit hook, runtime-neutral notifications and real public SDK gates |
| Locks | Atomic file-local writer predicate plus bounded per-resource admission | Actual Core CAS probe can return an already-expired lease if deadline precedes writer wait | Moderate: execution-time clock, live renew, idempotency and cancellation boundary; no distributed lock |

Implementation costs are qualitative engineering estimates, not measured effort
or a promised schedule. Do not copy prototype control loops into product code.

## Evidence levels

**Measured:** two passes at 100k/1m for B-tree/postings/bitmap, three text
distributions and two spatial distributions. Full result sets and matched
checksums, build/query tails, I/U/D WAL, index objects and process RSS are
retained. The compact term-set control prevents crediting FTS positions as an
engine disadvantage. [Tables](index-foundations-measurements-2026-10-15.md)
retain compatible scenario boundaries and each pass.

**Correctness only:** same-file Core SQL trigger/rollback/co-located handles,
frontier key moves/holes/extreme IDs, empty sources, definition supersession,
generation/prepared-name behavior, process-crash stages and disk-full recovery;
Russian stemming/float32/NULL counterexamples; actual grouped commits, delayed
lease commit, ignored acquire response and persisted fences. These probes do
not establish a throughput or production availability SLO.

**Architecture hypotheses / remaining product work:** cleanup with pinned old
readers, hard dirty-page/time limits, portable global memory admission,
arbitrary tokenizer/DDL replacement, compact external-content rebuild/OLD-token integrity, exact public SDK reset/gap semantics,
targeted FIFO wakeup, protected-resource fence enforcement and cross-platform
power-failure behavior. No distributed Channels/Locks, ANN or replication
engine is implemented. WAL measurements estimate derived-index replication
input; they do not measure a sender or replication lag.

## Owner decisions before public API work

1. Whether managed SQL index handles are needed, and whether KV/Records have
   demonstrated projection/search requirements. Keep raw SQL user-owned or
   define logical generation-resolving handles and retirement explicitly.
2. Text token/phrase/ranking semantics and Russian normalization/morphology;
   geo rectangle/radius, coordinate validation/precision and SRID.
3. Availability target and maximum acceptable write p99, disk/WAL/RAM reserve,
   rebuild cancel/redefinition, progress visibility and cleanup policy.
4. Channel names/path selectors, authorization, payload/connection limits,
   overflow versus disconnect, gap/reset epoch, subscribe acknowledgement,
   ordering scope and transaction staging semantics. Durable replay is a
   separate feature decision.
5. Lease clock/TTL, renew-at-expiry, waiting/fairness, cancellation after grant,
   request identity/reconciliation, fence overflow/restore epoch, semaphore
   weights and database ownership for application/Jobs atomicity.

## Acceptance before implementation is called complete

Use the per-module gates in [Indexes](index-foundations-2026-10-15.md),
[Channels](channels-wire-followup-2026-10-15.md) and
[Locks](core-locks-followup-2026-10-15.md). Require randomized lifecycle histories,
actual native writer/group-commit integration, bounded queues and native memory,
failed commits, long readers, cancel/credit loss, restart, disk pressure and
supported SDK/OS tests. Prototype success is evidence for a design, not a waiver
of these gates. Reproduction and raw provenance are in the
[evidence ledger](data/index-foundations-2026-10-15/README.md).
