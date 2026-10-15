# Scaling, round 2: cascade, recovery and fan-out — 2026-10-15

The prototype relays SQLite page packets through US → EU → RU → embedded,
including a database owned by EU. On this fixture, a 7 × 20 tree reduces
the root's response bytes by exactly 20×, increases total response bytes by
5%, and takes 11–15% longer than a star on loopback. A batch of 32 reduces
100 catch-up requests to 4 while preserving all 100 captured states. It
reduces this local catch-up's elapsed time by 8–13%, not 25×.

Snapshot scanning is too expensive to adopt as the capture mechanism without
another round. On a 1,130,496-byte SQL file, recording a captured state after
every write raises the median write-plus-capture time from 205–372 µs to
3,090–3,373 µs. These are container timings and provisional research results.

| Question                                    | Measured answer                                                                        | What follows                                                                            |
| ------------------------------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Does the tree unload its root?              | 140 root requests become 7; 2,023,000 response bytes become 101,150.                   | Relays distribute root egress; every child still needs the data.                        |
| Does it reduce total bytes?                 | No: 2,023,000 becomes 2,124,150, including the seven relay copies.                     | A tree trades extra hops and copies for lower root fan-out.                             |
| Do pull batches matter?                     | 100 retained states need 100 requests with a cap of 1, or 4 with a cap of 32.          | Bound batches by bytes and entries; distinguish fewer round trips from less apply work. |
| Can a copy keep reading during an outage?   | The TCP demo reads its existing copy while its upstream listener is unavailable.       | Keep applied pages and durable positions independently of the connection.               |
| Does async imply no data loss?              | A test leaves two accepted checkpoints unshipped; neither is present after promotion.  | Zero RPO needs acknowledgement from an eligible durable standby.                        |
| Is snapshot capture cheap enough?           | Total write-plus-capture time for 100 writes is 7.85–16.91× the paired baseline.       | Measure WAL capture and incremental integrity before adopting the implementation.       |

## Environment and reproduction

Same managed Linux/KVM container as [round 1](scaling-pages-2026-10-15.md):
kernel 6.18.44, x86-64, AMD EPYC 9V74 reported by the hypervisor, 3 CPUs
visible, CPU quota equivalent to 2 CPUs, 8 GiB memory limit. Stores for this
round were under an explicitly selected temporary directory on overlayfs,
outside the source tree. WAL, `synchronous=FULL`; records retain their normal
`os` durability, as do metrics. Rust 1.99.0, rusqlite 0.40.2, SQLite 3.53.2,
zstd level 3.
All geographic names refer to directories and loopback endpoints on this
one container. No WAN, separate failure domains or deployment latency was
measured. No other builds, tests or lints ran in either timed interval.

TinyStore source: `76b844cdc8c3410e746a5d5dc3e9aafec83c9962`, unchanged.
Research base: `569c1b0d661fa39c0dcc642f9feb12c7da4150d8`. The existing
source submodule stays pinned; the spike uses a separate source worktree.
Measurements preceded committing the harness. Code and raw outputs are now
recorded in research commit `fa41d7ef2565315b9eb1018775633ef871b22468`;
the two measured working-tree revisions are preserved separately:

- The first run's [environment and hashes](data/scaling-cascade-2026-10-15/environment.json)
  and [source archive](data/scaling-cascade-2026-10-15/harness.tar.gz).
  Aggregate source hash: `388bdf225ae7428d510c9ccd19320fdeeea9c7d1d9c4115a89194af8c578315d`.
- The batching follow-up's [environment and hashes](data/scaling-cascade-2026-10-15/batch/environment.json)
  and [source archive](data/scaling-cascade-2026-10-15/batch/harness.tar.gz).
  It adds the batch runner; the page receiver and transport are unchanged.

These are interim findings. The archives preserve the exact inputs from
before the harness commit; results remain provisional pending independent
reproduction. Environment records retain their original working-tree status
as measurement provenance.

```sh
cd <repo>/tinystore/replication-spike
sh bootstrap.sh
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
mkdir -p <temporary-storage>
export TMPDIR=<temporary-storage>
python3 environment.py > environment.json
python3 run_measurement.py resources.json > bench.jsonl
python3 run_measurement.py batch-resources.json batch > batch.jsonl
./target/release/tinystore-replication-spike demo
```

For an exact historical run, extract the corresponding source archive over
the spike before building. The dependency commit and build flags are in the
archive. The current tree includes both runners.

The first run was 08:40:21–08:40:30 UTC, 9.600 s wall time and 42,832 KiB
maximum resident memory for the entire harness, including setup. The batch
follow-up was 08:45:15–08:45:17 UTC, 1.876 s and 9,216 KiB. See
[resources.json](data/scaling-cascade-2026-10-15/resources.json) and the
[follow-up resources](data/scaling-cascade-2026-10-15/batch/resources.json).
RSS is process-wide peak memory, not bytes per replica. An initial attempt
to use `/usr/bin/time` failed because the tool was absent; no benchmark ran
in that attempt. The successful runs use Python `getrusage`.

## A relay is an ordinary node with a per-file role

An owned stream points to its local TinyStore file. An imported stream
points to `replicas/<stream>.db` and has no engine writers, migrations or
background work. The relay stores imported packets unchanged and can serve
them to another follower. Its own local database has a distinct stream ID,
so descendants receive both. Catalog polling also discovers databases added
after a follower starts. There is one owner per stream, not one global role
for the entire node.

Each position is `(epoch, sequence, canonical checksum)`. Delta packets name
their predecessor checksum. A receiver refuses a gap, a conflicting duplicate
and an older epoch. A pruned history produces a full current snapshot within
the caller's byte limit. Snapshots and files have a 16 MiB experimental cap.
The snapshot is not yet chunked. Sequences count captured snapshots, not
every intervening SQLite commit.

## The root's bandwidth is reduced, the network's is not

Fixture: 32 rows, a 256-byte body made from deterministic SHA-256 strings in
each row; first snapshot plus 4 captured updates. Each of 140 new children
replays and verifies all 5 states. There are 7 concurrent clients in either
topology. Node/journal creation is outside the timed interval. The tree's
interval includes catching up its 7 relays and starting their servers.

Runs alternate star, tree, star, tree, star, tree. These are complete
bootstrap timings, not steady-state replication throughput.

| Measurement                    | Star, passes 1; 2; 3               | 7 × 20 tree, passes 1; 2; 3         |
| ------------------------------ | ---------------------------------- | ----------------------------------- |
| Elapsed seconds                | 0.688875; 0.686798; 0.697410       | 0.791051; 0.787334; 0.771560        |
| Root data requests             | 140; 140; 140                      | 7; 7; 7                             |
| Root response bytes            | 2,023,000; 2,023,000; 2,023,000    | 101,150; 101,150; 101,150           |
| All servers' response bytes    | 2,023,000; 2,023,000; 2,023,000    | 2,124,150; 2,124,150; 2,124,150     |
| Children verified at target    | 140; 140; 140                      | 140; 140; 140                       |

Bytes include the application response's framing, not request bytes,
TCP/IP headers or TLS. Each full response is 14,450 bytes in this fixture.
The median completion time is 0.688875 s for the star and 0.787334 s for the
tree. Extra hops are a cost even when no intercontinental latency is present.
The repeatable 20× reduction in root egress is the useful result.

## Batched transport preserves the individual states

Follow-up fixture: one snapshot and 99 captured updates to the same
32-row fixture. It creates a fresh child for each pass. Source encoding
and journal preparation are outside the interval. The byte cap is 256 KiB;
the entry cap is either 1 or 32. All 100 states are individually applied and
checked, so this does not measure coalescing commits or grouped destination
fsyncs. Runs alternate cap 1 and cap 32 three times.

| Measurement        | Cap 1, passes 1; 2; 3        | Cap 32, passes 1; 2; 3        |
| ------------------ | ---------------------------- | ----------------------------- |
| Requests           | 100; 100; 100                | 4; 4; 4                       |
| Response bytes     | 229,061; 229,061; 229,061    | 228,677; 228,677; 228,677     |
| Elapsed seconds    | 0.316154; 0.293450; 0.300130 | 0.290955; 0.261391; 0.261480  |
| States applied     | 100; 100; 100                | 100; 100; 100                 |

The request count falls 25×; response bytes fall by only 384, because the
same packets still travel. Median catch-up time falls from 0.300130 s to
0.261480 s. With a link round-trip time R and this request/response driver,
the unoverlapped round-trip term is 100R versus 4R; that is a model, not a
WAN measurement. The same backlog on 140 children would require 14,000
versus 560 pulls if neither the byte cap nor retention intervened. That
140-child backlog projection was not separately timed.

## Wire amplification is per engine and per page

Ten sequential operations per engine, one capture after each. KV and jobs
receive a 256-byte deterministic string; SQL inserts that string in a row;
records append an event with that string and an integer request field;
metrics ingest one float64 sample on a single labelled gauge. Each engine
opens and has its schema captured first. The first metrics sample also
creates its series. Background work is disabled, so packing and maintenance
costs are outside this fixture.

| Engine     | Page bytes   | Changed pages min / median / max   | Raw packet bytes min / median / max   | Zstd packet bytes min / median / max   | SQLite file bytes min–max   |
| ---------- | ------------ | ---------------------------------- | ------------------------------------- | -------------------------------------- | --------------------------- |
| KV         | 4,096        | 2 / 2 / 2                          | 8,304 / 8,304 / 8,304                 | 369 / 1,038 / 1,672                    | 49,152–49,152               |
| jobs       | 4,096        | 3 / 3 / 3                          | 12,406 / 12,406 / 12,406              | 420 / 1,107 / 1,738                    | 81,920–81,920               |
| SQL        | 4,096        | 1 / 1 / 1                          | 4,205 / 4,205 / 4,205                 | 331 / 994 / 1,628                      | 8,192–8,192                 |
| records    | 1,024        | 6 / 6 / 9                          | 6,277 / 6,277 / 9,361                 | 522 / 997.5 / 1,709                    | 32,768–36,864               |
| metrics    | 4,096        | 3 / 3 / 14                         | 12,409 / 12,409 / 57,509              | 266 / 269 / 534                        | 110,592–110,592             |

The raw packet has a 102-byte fixed header, stream ID bytes, and one
4-byte page number per page. File sizes are logical SQLite page bytes,
excluding WAL, SHM, the replication journal and the outbox. This is a wire
measurement, not a claim of on-disk storage savings. The whole previous page
is resent when it changes; compressed size can grow as prior values fill
that page. These small generated fixtures do not predict production
compression. Blobs' external objects were not replicated or measured.

## Snapshot capture is the expensive provisional part

4,096 rows, 256-byte deterministic bodies, 1,130,496-byte SQL file; each
pass updates 100 rows. Both configurations initialize and capture the same
starting database. The baseline then writes without capturing; the candidate
captures after each write. Three interleaved pairs, fresh directories each.
The timer starts immediately before the real TinyStore write and stops
after capture and journal persistence. Fixture generation is not timed.

| Measurement                  | Baseline, passes 1; 2; 3        | Capture enabled, passes 1; 2; 3      |
| ---------------------------- | ------------------------------- | ------------------------------------ |
| Median per write, µs         | 371.911; 204.889; 210.507       | 3,089.662; 3,373.230; 3,256.844      |
| p95 per write, µs            | 711.534; 365.121; 366.764       | 4,506.980; 7,173.405; 4,905.127      |
| Total for 100 writes, ms     | 41.580; 23.071; 23.184          | 326.507; 390.105; 347.593            |
| Capture-only median, µs      | not enabled                     | 2,885.754; 3,068.671; 2,976.481      |

p95 is nearest rank among 100 observations in a pass. Pass 1's slower
baseline is preserved rather than discarded. The combined cost includes a
whole source snapshot, reading the journal's prior image, SHA-256, page
diffs, zstd and durable journal updates. This run does not attribute the
time among them. WAL capture is the next candidate, not a measured win here.

## Recovery, outbox and limits

The captured packet is made durable before modifying an imported file.
Applying its pages is one SQLite transaction. A post-apply canonical checksum
is checked before advancing the durable position. A pending apply blocks
reads through the node. Restart replays the packet if the process died before
applying pages, or if it died after applying pages but before the cursor was
committed. A subprocess test exits without destructors in both windows.

The independent SDK outbox holds trusted application intents, not pages.
Its enqueue result is pending. The primary commits a request ID and the
effect in the same application database transaction. An injected lost
response leaves the intent queued; replay finds the receipt and does not
repeat the insert. The fixture has no offline overlay or conflict merge.
Delivery is a callback experiment, not a write RPC added to TinyStore.

`cargo test --locked` passed 21 behaviour tests; one subprocess worker is
ignored by the top-level harness and is launched twice by its parent test.
Formatting and clippy with warnings denied passed. The
[test output](data/scaling-cascade-2026-10-15/tests.txt) records the cases.
The demo separately bootstraps 140 directories through TCP and performs the
multi-level relay, retention, outage, outbox and promotion scenarios.

This is a foundation for implementation, not production replication. It has
no commit-time WAL capture, sync write gate, lease, automatic retention,
automatic rejoin, object shipping, production authentication or cross-region
fault test. A stream's promotion does not reopen its inherited TinyStore
engine handles. The archive preserves these limits as well as the results.

## What follows

Keep the pull/relay model and per-file authority. Replace snapshot scanning,
bound retention and snapshot transfer, and integrate capture at every commit
path. Keep offline intents separate and make acknowledgement semantics
explicit. [The implementation handoff](scaling-handoff-2026-10-15.md) maps
those changes to the current Rust code and lists the gates still missing.
