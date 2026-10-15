# Replication Engine: snapshots, bounded history and delivery — 15 October 2026

The owner moved the priority from marginal capture speed to coherent snapshots,
segmented retention and a network sender. This is the interim design; measured
results will follow. Product work stays on `experiment/replication-engine`;
research code and reports land on main. The previous measured modules stay intact.

| Responsibility | Owner | Contract |
| --- | --- | --- |
| Capture | SQLite writer observer | Published native WAL frames become ordered checksummed records before local ACK; FULL remains; feed sync precedes source checkpoint. |
| History | Segmented journal | Bounded bytes/age; metadata publication precedes deletion; slow peers do not pin history indefinitely. |
| Delivery | Persistent TCP sessions | Resume from explicit durable position; missing history requires a snapshot; no network wait under the writer or journal lock. |

## A snapshot without a file-long read transaction

Copy SQLite pages in short read transactions. Take a stream position S0 under
the file writer before copying, then a position S1 under the writer after the
last page read. Replay every captured change in (S0,S1] onto the unpublished
copy, including growth/truncation, and publish a synced manifest plus file.
For each page: if it changed after S0, the last record supplies its state at
S1; otherwise every copied version between S0 and S1 is identical. A page not
present in an intermediate copy can be supplied by later growth records.
The final geometry is taken at S1. A snapshot never becomes visible halfway.

The two cuts use a no-write TinyStore transaction as a writer barrier. Taking
only the capture mutex would be wrong: native COMMIT may already be visible
while its observer is still waiting. Long native read transactions remain a
control, not the proposed large-file snapshot path. sqlite_dbpage supports an
equality lookup on pgno, not a range seek: bounded copies use prepared page
lookups, avoiding repeated full-table scans for each chunk.

A snapshot build can lose required history to retention and must abort. It
cannot hold an unlimited lease or stall Primary to finish. Published snapshots
whose tail is gone cannot bootstrap a current follower; build a fresh one.
The sender closes file handles before network waits, to avoid keeping deleted
segments or snapshots alive as invisible disk usage. Artifact count and size
are separately bounded; full file SHA-256 verifies snapshot transfer.

## Positions and memory

The new experimental history format uses a SHA-256 record chain (stream, epoch,
sequence, predecessor, geometry, cursor and pages), and a streamed SHA-256 of
an immutable snapshot. These are distinct checks. It does not keep a full
per-page Merkle tree: the earlier in-memory tree would itself grow with database
size. Final canonical page digest, SQLite integrity and application counters
will independently verify the resulting files. This is a new versioned research
format, not a reinterpretation of the earlier measured format.

## What a hard budget can and cannot promise

Primary may expire history regardless of an asynchronous peer's ACK. The peer
then resnapshots. This bounds Primary disk independently of disconnected peers;
it does not guarantee every bandwidth-starved peer eventually catches up.
For change rate R and snapshot transfer time T, at least approximately R*T of
history must survive the transfer, plus margin. Steady delta delivery must
exceed R. A finite disk, unbounded outage, lossless retained history and an
always-available Primary cannot all be promised together.

Use real databases around 1–2 GiB in this environment (32 GiB volume), as the
owner clarified. Test actual snapshot/catch-up and outages; simulate weeks of
age with an injected clock and label that simulation. Do not describe it as a
weeks-long endurance run. Smaller fixtures isolate crash points, 140 sessions,
slow transfer and deliberate retention starvation. Measure actual per-object
SQLite bytes, history/artifacts, WAL, process RSS and pending ACK/application
positions, with raw output and exact committed revisions.
