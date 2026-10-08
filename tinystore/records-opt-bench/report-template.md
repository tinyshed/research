# Records: fairer native algorithms and owned backing — 9 October 2026

This follows [the first records native round](records-native-2026-10-08.md).
The earlier Rust slice eagerly rebuilt every candidate, sorted all matches,
decoded entire segments for Follow and duplicated small strings. This round
implements the missing query, page, Follow and ownership work and measures
**public Go, the previous native implementation and the improved native
implementation in the same session**. It does not promote a full Rust port.

| Question | Answer | What follows |
|---|---|---|
| Were the previous Rust records losses just inherent native costs? | No. The new implementation improves typed-head full/filter reads 2.92×/2.89× and identical-sealed full/page reads 2.54×/5.55× versus previous Rust. | Algorithm and ownership choices must accompany backend comparisons. |
| Does it now win every query? | No. Production Go still wins sealed exact filters, single-trace reads and no-match context queries. | Raw-attribute reconstruction and remaining selective-read work still matter. |
| Is warm Follow's large win a general backend win? | No. A decoded cache and shared results make warm repeated cursors very cheap. Raw-only cache is close to Go; cold handles and advancing walks are reported separately. | Quote the cache temperature and representation with every Follow number. |
| Does sharing improve retained memory? | Yes for this complete owning-result fixture: about 141.5 MiB current RSS versus previous 335 MiB and Go 272–289 MiB. Detaching the new result into String/Vec returns to 291 MiB. | Owned backing is a measured representation improvement, not automatic native-memory economics. |
| Does a bounded heap always improve CPU? | No. It bounds retained row selection, but disabling it makes the full fixture scan about 12% faster; small-page CPU changes about 1%. | Keep its resource bound without disguising this CPU tradeoff. |

## Environment and reproduction

TinyStore stays pinned at `e307c48a40126aad0e2873b6bf3aaedef8115483`.
All production files and the first round's measured sources/binaries are
unchanged. The code is in [records-opt-bench](../records-opt-bench/README.md),
including build, correctness, mutant-proof and collection scripts. The copied
previous Rust engine is byte-identical to the first round's engine.

This is exploratory uncommitted collection under the user's earlier explicit
waiver of the commit-before-measurement rule. Exact source/binary/fixture
hashes, CPU/toolchain/SQLite settings and calibration are retained in
[capped/environment.json](data/records-optimization-2026-10-09/capped/environment.json).
The machine is the same Ryzen 7 7700 / Docker Desktop WSL2 Linux host, Go 1.27.1,
Rust 1.99.0, stock rusqlite 0.40.1 and the matched static SQLite 3.53.4 archive.
Rust uses its ordinary portable release build; allocations use a separate
counting binary. The static backend still uses the original native Unix VFS
configuration. No fdatasync build from the parallel KV investigation is used.
FULL WAL synchronous writes, 1024-byte pages and 1 MiB SQLite connection caches
remain. Read-only query/Follow optimization is the subject; append and seal
algorithms are unchanged controls and were not rebenchmarked.

Both actual Go-head and fixed compatible native-sealed databases are reused
byte-for-byte from the first round. Each contains 4096 deterministic records
with equal-time pairs, Unicode, optional fields, trace/span and ordered/repeated
JSON fields. The public Apache body fixture is also unchanged; no private
corpus is used. Stores, binaries and build targets are on the Linux volume.
No competing research build/test/timing ran during granted windows; desktop
and VM background activity was not independently controlled. These sequential
VM observations are not bare-Linux latency or concurrent-throughput promises.

The initial fastest-candidate calibration projected 37.38 minutes because the
decoded Follow cache was far faster than old native. The parent authorized
stopping that plan. Docker/VM had stopped by the resumed turn; its 224 complete
raw rows, pilots, environment and exact runner are preserved without alteration
under [partial-fastest-counts](data/records-optimization-2026-10-09/partial-fastest-counts/status.json).
They are **partial**, and supply no headline ratios.

A separately identified [run_capped.py](../records-opt-bench/run_capped.py)
uses one identical fixed count per fixture/case across every compared candidate:
the smaller of a 150 ms-fastest target and a 1 s-slowest pilot cap, at least 8 and
at most 32768 operations. Counts stay fixed across all six passes. All original
pilot rows and cap reasons are retained. Six permutations balance Go/previous/
optimized order; switch ablations reverse order every other pass. A separate
opt/raw-cache/cache-off group uses its own shared counts, with 100 ms-fastest
target and the same 1 s-slowest cap. It is not matched to old/Go operation counts.
Cold cases use eight operations per sample. Achieved durations, including the
short cached samples, are reported below rather than presented as 150 ms runs.

After all main/ablation/cache timings completed, the allocation diagnostic
failed with exit 126: the copied counting binary had mode 0644. Only execute
permission changed to 0755; its content SHA remained
`870d5e7d19fb176a5a4602518fc74da9177b11702b6569f42df1593a293b97bc`.
The separate [finish_diagnostics.py](../records-opt-bench/finish_diagnostics.py)
completed allocation/RSS and assembled summaries without rerunning successful
timings. Its hash and source/fixture preservation are recorded in
[completion.json](data/records-optimization-2026-10-09/capped/completion.json).
The completion inventory contains 972 timing rows, 54 summary groups and six
samples for every label/group. The later diagnostic/report helpers have their
own files; they do not change the measured runtime/runner source snapshot.

```sh
export PATH=/work/cargo/bin:$PATH
python3 /src/tinystore/records-opt-bench/build.py
python3 /src/tinystore/records-opt-bench/prove_guard.py
python3 /src/tinystore/records-opt-bench/build.py
python3 /src/tinystore/records-opt-bench/verify.py
# Ensure the already-built counting binary is executable:
chmod 755 /work/records-opt/rust-alloc-records
# In a quiet research window, retain the original pilot plan separately,
# then run the capped plan as documented by run_capped.py:
python3 /src/tinystore/records-opt-bench/run_capped.py --measure
```

The capped reproduction uses retained original pilots under
`partial-fastest-counts/calibration.json`; `run.py` regenerates those pilots
and is retained as the original, intentionally uncapped plan. Starting its
full collection is unnecessary when reproducing the capped results.
The complete capped timing/diagnostic window is 22:50:27–22:56:49 UTC on
8 October (9 October local time). Original controls, runtime sources and
fixture hashes were checked before and after; the partial plan remains
separately labelled rather than merged into the final six-pass results.

## Implemented algorithms and ownership contract

The new reader checks envelope CRC, block directory, names/shapes, times and
stored index metadata before query selection. Supported query columns decode
before matching, and only selected rows need full materialization. Go heads
still require decompression and parsing of every sequential row, but string
and field spans point into an **owned** decompressed backing buffer rather
than allocating two Strings per field. Sealed text columns similarly own a
buffer with compact ranges; immutable schema names/contexts are shared.

A limit-sized heap retains at most the requested page's row references. It
tracks the best discarded timestamp, so pages do not split equal-time groups;
oldest/newest ordering and continuation remain exact. Unfiltered candidate
coverage can stop fetching before the remainder of the range, matching the
shape of production's page strategy. A small result can retain its fetched
chunk's backing; sharing is a deliberate owning tradeoff, not per-record
detachment or a zero-copy SQLite pointer.

Every returned row calls `ready` and has all its fields decoded/validated
**inside the operation**. Canonicalization after timing does not hide field
decode work. Safe Mutex-protected backing makes returned Row Send + Sync;
no unsafe Send/Sync implementation is used. Results survive sibling, cache,
engine and both connection destruction, including a move to another thread.

Follow lists the unmerged holder's blocks, fetches only those intersecting
the requested rows, and decodes those blocks. Cache modes are none, raw bytes
(closest to Go) and parsed schema/decoded chunk. The 4 MiB limit is a **charged
capacity bound**: backing bytes, vector/string capacities, dependent schemas
and conservative map/allocation overhead, not exact allocator RAM or RSS.
Results may keep evicted entries alive independently of the persistent cache.
Keys assume immutable nonrepeating unmerged block IDs and holder first-block
IDs. An external rewrite under an already cached ID is unsupported; a test
records that cached immutable data remains and clearing cache detects CRC
damage. Current index metadata is validated even on a decoded cache hit.

`prune_off`, `heap_off`, `sharing_off`, `cache_raw`, `cache_off` and `strict`
are separate switches. `strict` parses all supported fetched columns, while
the default follows consumed-column validation: a malformed unused column
with recomputed CRC need not fail a query that returns no rows. Envelopes,
directories and indexes always validate; malformed consumed/returned fields
fail. This difference from old native's eager validation is explicitly probed.
The new decoder also charges cumulative decoded text across each block/schema
before decompression, closing the first slice's per-blob expansion gap.

The restricted v 1 format/valid-UTF 8 scope remains. Arbitrary production typed,
dictionary, FSE, Rice, radix and stamped sealed columns are not a completed
port. Lifecycle/locking, cancellation, shared admission, concurrent/grouped
writes, general merged/expired holders, retention deletion and Damage/Drop
are excluded. No production-ready engine or universal language-only win is
claimed.

## Same-session reads: substantial improvement, remaining Go wins

Six-pass medians in milliseconds; every row uses identical fixture bytes and
the same iteration count for Go, previous Rust and optimized Rust. Full reads
return 4096 records; exact combined filters 274; requested 127-page results 126
because equal timestamps remain together; trace returnsone. Search matches
3510 typed records and zero Apache records. `scan_none` is an absent attribute
value; `scan_no_context` an absent context value. Both return zero.

{{MAIN}}

Go still wins the sealed exact filter, single trace and absent-context query.
The fallback representation's raw attributes and the new column work are not
production's complete typed/pruned paths. Conversely, actual Go-head filtered
and full reads improve 2.89×/2.92× over previous native, and sealed page/full
reads 5.55×/2.54×. The old Rust implementation is a same-session control,
not a number copied from yesterday's report.
Source inspection shows remaining differences: the native raw-attribute
slot parses field spans for its entire column before testing exact values,
and any retained row validates all fields of its chunk. Schema context values
are copied into shared dictionaries, and safe Mutex locking protects lazy
state. These are candidates for the remaining filter/trace cost, not isolated
causal findings or evidence of a language penalty; this round has no profile
or matched switch separating each of them individually.

{{PASSES}}

## Follow: separate cold, raw-cache and decoded-cache behavior

Warm repeated cursor means the same `(0,0)` 127-record result after warmup.
Warm advancing walk restarts at zero each operation and follows all 4096
records through the same 127-record cursor sequence. Cold cases create a new
engine/connection pair before **each** operation, excluding opening and
destruction symmetrically; caches of that handle are cold, while OS file
cache temperature is uncontrolled. Eight cold operations are accumulated.

{{FOLLOW}}

The approximately 323× warm typed repeated-Follow previous/optimized ratio
is chiefly a decoded-cache/representation comparison. It must not headline
a native SQLite advantage. Cold repeated-Follow improves 4.08× versus previous
and 1.38× versus Go; cold complete walk improves 31.14× versus previous and 7.86×
versus Go. Even a cold advancing walk can fill its per-operation cache while
advancing through adjacent batches; it is not cache-disabled.

The independently counted fast-only supplemental group gives the following
microsecond medians. It excludes old/Go from its count plan and retains the
same count within opt/raw/off. Its fastest samples remain short because the
1 s-slowest cap limits the large decoded/raw spread; raw ranges are below.

{{FAST_CACHE}}

Raw-only Follow is close to Go, while decoded cached chunks avoid redoing
column parsing and owned-record reconstruction on repeated blocks. Cache off
still fetches/decode only needed blocks and therefore differs from previous
Rust's full-segment path. No single warm-cache result stands in for a realistic
whole-file follower.

## Attribution switches include negative outcomes

Relative-to-opt values aboveone mean that variant is slower. These are six
balanced switch passes, not causal claims inferred solely from the first
Go/Rust ratio.

{{ABLATIONS}}

On the typed sealed page, disabling pruning/coverage changes 0.583 ms to 1.289 ms;
disabling the heap changes 0.583 ms to 0.588 ms. Thus the large page improvement
is not a measured heap-CPU win. Full scan is faster with collection/sort
(`heap_off` 1.124 ms vs 1.260 ms), although the heap bounds selection space.
On Go heads, prune/heap switches barely move the page/filter cases because
the sequential head must still be parsed. Sealed filtered eager/strict
validation is about 13% slower, and the absent-attribute query about 39–45%
slower. The consumed-column difference is therefore both semantic and measured.

Detaching complete sealed results (`sharing_off`) raises 1.260 ms to 3.923 ms,
returning much of the former CPU and allocation cost. This direct native
control supports ownership representation as a major contributor.

The archived executable controls remain close to their extended driver
counterparts; they are retained rather than silently replacing the old
baseline after refactoring.

{{ARCHIVE}}

## Allocations and retained results

Rust diagnostic allocations come from the separate counting build and exclude
C SQLite/zstd allocations; Go uses runtime counters. Native SQLite counters
are recorded separately. Counter-disabled Rust timing rows containzero and
must not be read as zero allocation. Diagnostics are not timing headlines.

{{ALLOCATIONS}}

For full sealed reads, previous 88819 allocator calls become 649; detaching new
results raises them to 77898. Allocated bytes remain significant because
backing buffers, directories, ranges and decoded columns still allocate.
The exact-filter native path still allocates more bytes than Go despite far
fewer individual allocation calls. Shared graphs are not allocation-free.

The memory fixture retains 64 owning results ×4096 records:262144 records and
the same 47,121,856 bytes (44.94 MiB) logical content as the first round. Current
RSS is `/proc/self/status`; peak is GNU time. These include runtime, SQLite,
compression, scratch and result graph. This scan-only test fills no persistent
Follow cache (`follow_cache_bytes=0`), so it does not conflate cache capacity
with retained query results.

{{MEMORY}}

The old native current RSS is about 335 MiB, optimized 141.5 MiB and detached-new
291 MiB; Go varies 272–289 MiB. Ownership sharing is a real improvement here,
but it keeps chunk backing alive and is not a complete production API design.
SQLite's current native memory stays around 0.19 MiB and is separate from these
graphs. The cache charge and inclusive RSS must not be added or equated.

## Achieved durations, validation and limits

The capped plan intentionally leaves some fastest cached samples only a few
milliseconds. All six values, operation counts, cap reasons and temperature
labels remain in [capped/summary.json](data/records-optimization-2026-10-09/capped/summary.json)
and its raw JSONL files. Treat very large warm-cache ratios as approximate
local differences with these duration limits, not precise production factors.

{{DURATIONS}}

[build.json](data/records-optimization-2026-10-09/build.json) retains successful
Linux formatting/build logs, two Go tests and fourteen Rust tests.
[verification.json](data/records-optimization-2026-10-09/verification.json)
records eight variants on five fixed artifacts, complete oldest/newest pages
and Follow walks, late publication, seven Follow boundary cursors, invalid
limits/budgets, result-after-engine/sibling-drop checks, cache eviction/index
guards and cold CRC/index/consumed/unused-column probes. Shared Row has compile
time Send + Sync and a thread-handoff test. No production tests or other-platform
runtime claims are added for this research-only change.

The new guard mutation disables returned-field readiness. Its negative test
then accepts a recomputed-CRC malformed selected body and fails, proving the
test guards against moving decode/validation past return. Exact source bytes
are restored and it passes; [guard-proof.json](data/records-optimization-2026-10-09/guard-proof.json)
retains both outcomes and matching hashes. All original-round sources/binaries,
and current measured source/fixture hashes, are preserved.

There is no new storage-format claim: both readers consume the same physical
artifacts and this round writes no alternative format. The previous round's
payload/file/dbstat division remains the relevant storage measurement.
CPU profiles, concurrent throughput, general untrusted-file support and a
complete typed sealed codec remain unresolved. The result is improved and
remeasured native algorithms with honest controls, not a predetermined win.

## What follows

The new ownership and selective paths remove major avoidable costs; they also
show the remaining production Go wins. A next candidate needs the full typed
codec and stronger selective raw-attribute handling, while preserving exact
result ownership, corruption policy, cumulative expansion and bounded
resources. The full-scan heap regression and cache temperature effects should
remain visible in that evaluation rather than optimized away from its report.
