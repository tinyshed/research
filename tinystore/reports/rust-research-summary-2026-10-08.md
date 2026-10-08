# TinyStore Rust research synthesis — 2026-10-08

The experiments support continued work on a Rust/native-SQLite prototype. The
largest measured improvements came from native read paths, maintenance, and
removing per-sample exact-arithmetic allocations. Word-based packing also
improved Go substantially, and Rayon helped some reads while slowing other
queries. The evidence does not establish that a production migration is ready.

This synthesis preserves four preliminary local experiments and a subsequent
deep-optimization round. The four earlier harnesses were uncommitted when
measured; source and executable hashes, raw samples and verification evidence
are retained with each report. Publishing those artifacts does not
retrospectively make them formal committed-harness measurement rounds. The final
round measured a committed experimental harness. None of these results is a
product promise. The production baseline is TinyStore commit
`e307c48a40126aad0e2873b6bf3aaedef8115483`; production source was unchanged.

| Experiment | Question answered | Finding | Remaining limitation |
| --- | --- | --- | --- |
| [Selected kernels and SQLite feasibility](rust-kernels-2026-10-07.md) | Can the selected algorithms and a native connection model work? | Rust improved preparation and change-value loops; word packing improved Go's selected decoder by 8.89×. | Isolated algorithms exclude SQLite, complete codec selection and public API work. |
| [SQLite backend comparison](sqlite-native-2026-10-07.md) | How do native SQLite and the existing Go SQL layer compare? | With 4096-byte pages, native reads were 2.57–3.17× faster than TinyStore's SQL layer. | Synthetic SQL, warm connections, adaptive traces and noisy durable writes. |
| [Complete synchronous metrics paths](metrics-native-2026-10-08.md) | Do the gains survive real metrics storage, codecs and exact summaries? | Ingest improved 1.35–2.18×, reads 2.12–3.42× and sealing 2.50×. | The Rust prototype omits production runtime and concurrency contracts. |
| [First serial optimizations and Rayon](metrics-optimization-2026-10-08.md) | Which additional Rust changes pay off, and where does parallelism help? | Cut sum/avg improved 2.39–2.45× over baseline Rust; Rayon 2 improved Read16 another 1.30× over optimized serial Rust. | Several Stream/aggregate cases became slower; concurrent server load was not measured. |
| [Deep optimizations, compiler variants and profiles](metrics-deep-2026-10-08.md) | Do word readers, buffer reuse, fused processing and compiler choices add further gains? | Read16 improved 1.19× with the new serial code, 1.42× with PGO and 1.76× with PGO + Rayon 2 over the remeasured preceding optimized Rust. | Durable scrape showed no further benefit; RSS was materially unchanged and PGO generalization is unproven. |

## Algorithms: implementation choices matter in both languages

The kernel experiment used 54 deterministic fixtures and five paired Go/Rust
passes. Ordered 4096-arrival preparation improved from 23.161 to 7.654 µs/op,
or 3.03×; shuffled preparation improved from 949.738 to 248.892 µs/op, or
3.82×. Go's typed-sort variant took 634.261 µs/op against Rust's 251.482,
reducing the ratio to 2.52×. The selected change-value decoder improved from
4.655 to 1.034 µs/op, or 4.50×.

The literal 17-bit residual decoder did not demonstrate a Rust gain: Go took
4.064 µs/op and Rust 4.264, with overlapping pass ranges. A word-reservoir
algorithm reduced Go to 0.457 µs/op, an 8.89× improvement. Optimized Rust took
0.222 µs/op, another 2.06× relative to optimized Go. Both algorithms preserved
the packed bytes and validation. This is evidence for changing the algorithm
inside Go as well as for continuing the Rust experiment.

These loops do not predict whole-engine speed. The report's illustrative
3× kernel gain yields only 1.07× total speed when the kernel accounts for
10% of execution, 1.20× at 25%, or 1.50× at 50%. Kernel and SQL ratios cannot
be multiplied together to manufacture an engine ratio.

## SQLite: a backend comparison with controlled versions

The SQLite experiment compared the real TinyStore SQL adapter, direct Go
connections to the same translated SQLite backend, and rusqlite/native SQLite.
All reported SQLite 3.53.4 with the same source ID. With 4096-byte pages:

| Operation | TinyStore Go, µs | Direct Go, µs | Rust/native, µs | TinyStore / native |
| --- | ---: | ---: | ---: | ---: |
| Point lookup, 128 B | 4.825 | 2.937 | 1.855 | 2.60× |
| Point lookup in a read transaction | 7.866 | 4.096 | 2.482 | 3.17× |
| Range, 240 owned rows | 107.811 | 71.103 | 41.864 | 2.58× |
| COUNT + SUM, 240 rows | 43.469 | 42.008 | 16.944 | 2.57× |
| One update + FULL commit | 188.470 | 175.221 | 152.464 | 1.24× |
| 64 updates/savepoints + FULL commit | 1039.198 | 1045.415 | 723.889 | 1.44× |

Native reads were also 1.58–2.48× faster than direct Go. Direct Go bypasses
admission and adapter checks as well as database/sql, so the gap between the two
Go variants is not a pure measure of database/sql overhead. These are backend
and wrapper comparisons, not an isolated Rust-language effect.

WAL/FULL durability, caches, statement ownership and read-back PRAGMAs were
retained. The main run collected 108 timings; a longer six-pass write followup
collected 36 more after an initial 9073 µs/op outlier. Durable-write latency
remained visibly noisy. Adaptive calibration also carried mutation state
forward, so stacks could time different operation counts, key offsets and WAL
phases. The following engine comparison addressed this with identical fixed
timed traces.

## Complete synchronous metrics paths

The next prototype performed actual registration, postings, ingest preparation,
packed-head reuse/replacement, sealing, shared clocks, retention, snapshot reads,
label matching, codec decoding and exact aggregate folding. Go called the real
public metrics API. The Rust implementation used the same schema and compatible
current formats; production Go reopened and read Rust-written files.

Across five paired passes, ingest improved 1.35–2.18×, reads 2.12–3.42×,
sealing 2.50× and full expiry 1.89×. Whole-block aggregates improved
1.75–2.56×, including grouping. Cut-block sum/avg improved only 1.14–1.15×:
their overlapping pass ranges made this modest advantage inconclusive as a
general performance promise.

| Public operation | Go, ms | Rust/native, ms | Go / native |
| --- | ---: | ---: | ---: |
| `ingest_scrape100` | 4.9427 | 2.2666 | 2.18× |
| `maintain_ready` | 84.7723 | 33.9451 | 2.50× |
| `read_head_point` | 0.0559 | 0.0164 | 3.42× |
| `read_sealed_full16` | 4.7545 | 2.2435 | 2.12× |
| `stream_sealed_full8` | 2.6487 | 0.9684 | 2.74× |
| `aggregate_cut_sum` | 2.2531 | 1.9813 | 1.14× |

The round checked 49 scenarios, 23 selection/resource plans, errors and atomic
rollback; recorded 320 timings and 36 memory processes; and passed 15 Rust tests.
Rust decoded 152 actual Go heads and 344 sealed blocks. A separate probe rebuilt
all 24 edge blocks/directories with Rust and Go recovered the original 5,772
sample bits/hash. Compatibility here is logical and format compatibility;
compressed bytes and database files were not asserted identical.

The main 132 KiB sealed fixture fit the 1 MiB SQLite cache. Synthetic decimal
values and common clocks were unusually compact. These are warm CPU/backend
measurements, with one owned reader and writer, rather than cold-disk,
high-cardinality or concurrent server workloads. After sealing, both files used
135,168 bytes and every table/index owned the same number of pages: this round
established no file-size saving despite different directory/payload bytes.

## First optimizations: exact arithmetic before broader parallelism

The first optimization round remeasured Go and baseline Rust in the same session
as its candidates. Its comparisons therefore use that round's baselines, not
the preceding report's medians. It checked 56 scenarios × 9 variants and 21 Rust
tests, with 1135 timings over 39 workloads and 75 separate RSS processes.

A 35-word, 280-byte exact accumulator removed per-decoded-sample BigInt creation
for sum/avg while preserving exact merges and final rounding. Regular clocks
filled timestamps directly; uncompressed payloads were borrowed and Read
reserved bounded output capacity. These serial changes reduced cut sum/avg
latency by 2.39–2.45× and wide cut-aggregate latency by 2.27–2.43× relative to
baseline Rust.

| Workload | Baseline Rust, ms | Optimized serial Rust, ms | Rayon 2, ms | Baseline / serial | Serial / Rayon 2 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `aggregate_cut_sum` | 2.0202 | 0.8238 | 0.8931 | 2.45× | 0.92× |
| `aggregate_cut_avg` | 2.0188 | 0.8435 | 0.7987 | 2.39× | 1.06× |
| `aggregate_wide_cut_sum` | 5.0345 | 2.1257 | 2.4112 | 2.37× | 0.88× |
| `read_sealed_full16` | 2.5330 | 2.3641 | 1.8139 | 1.07× | 1.30× |
| `stream_wide64` | 2.6537 | 2.4170 | 2.8076 | 1.10× | 0.86× |

The ablation supports exact arithmetic as the main cut-sum contribution:
refactoring with optimizations disabled took 2.150 ms and the exact accumulator
alone took 0.877 ms; codec changes alone had little effect on that aggregate.
For Read16, codec/reservation changes reduced 2.569 to 2.188 ms. Small changes
elsewhere require caution on this shared VM.

Rayon processed independent series after the SQLite snapshot closed, with
ordered results/callbacks and bounded batches. Two threads improved Read16 by
another 1.30× over optimized serial Rust but increased wide Stream latency and
several aggregate latencies. Queries using only summaries remained serial. The
writer was not parallelized, and write-path differences cannot be attributed to
Rayon. The cgroup quota was two CPUs; virtual CPU/cache topology and a separate
CPU-pair comparison limit physical-core scaling conclusions. This is evidence
for selecting parallelism by actual decode work and query type, not for enabling
it everywhere or claiming linear scaling on 8–16 cores.

## Linking and the no-Cgo constraint

The existing Go builds use `CGO_ENABLED=0`. Their SQLite is translated ahead of
time through wasm2go, with a Go VFS; no WASM interpreter runs in these measured
loops. Replacing that backend with Cgo would change the Go build constraint and
is not the candidate evaluated here. The Rust prototype can call native C
libraries without adding Cgo to the unchanged Go production engine.

The initial rusqlite 0.40.1 smoke used its `bundled` feature to establish
connection/configuration and linking feasibility. It embedded SQLite 3.53.2,
whereas the baseline used 3.53.4. The later comparisons therefore disabled
defaults and linked a pinned, externally built static SQLite 3.53.4 archive from
the official amalgamation, with recorded source/compiler hashes and options.
This controlled version/source ID and mirrored SQL/planner options more closely
than the initial bundled smoke. The external archive is a build input; the
resulting executable still embeds SQLite rather than requiring libsqlite3.so.

Rusqlite's synchronous API matched the prototype's owned reader/writer model.
SQLx's SQLite worker threads/channels would add a different execution model;
raw libsqlite3-sys remains available for specific API gaps. The feasibility
report also records compatibility gaps that prevented treating Turso as a
drop-in replacement for these contracts.

Native C compilation, rusqlite, Unix VFS, allocation ownership and native
compression remain combined implementation differences. In timed Read/aggregate
loops, Go retains its sink output while Rust consumes and destroys each owning
result; both retain the last Stream callback result. The Go/native latency
ratios include these consumer-lifetime differences, as well as the backend and
engine differences. The separate RSS scenarios retain 16 owning results on both
stacks. Normalize consumer lifetimes in a future application-load comparison.
Native SQLite retains
THREADSAFE=1 and independently owned NO_MUTEX connections; the Go backend uses
isolated translated THREADSAFE=0 instances and a custom VFS. Copying the Go
setting into a shared native library would not preserve that isolation.

SQLite and, in the engine prototype, zstd are embedded. System libm, libgcc_s,
libc and the loader remain dynamic. A C compiler is needed at build time, and a
fully static musl artifact was not measured. The prototype Huffman code also
uses private symbols from its pinned zstd build, leaving ABI/build assumptions
for production review.

## Memory: observed RSS and owned capacity, not total-RAM control

The kernel experiment retained approximately 64 MiB of identical logical output.
Go preparation held 80 MiB of output capacity against Rust's 64 MiB, while decoded
buffers had equal capacity. GC, allocator/page-release policy and harness
metadata contributed to RSS differences; the experiment did not show that an
equivalent retained footprint was impossible in Go.

In the full synchronous engine round, retaining 16 Read16 results retained
1,229,056 points, or 18.75 MiB of timestamp/value content. Median current RSS was
48.97 MiB for Go and 23.39 MiB for Rust/native. In the first optimization round,
Read wide64 peak RSS fell from 31.32 MiB for baseline Rust to 27.51 MiB with
serial reservations; Rayon 2 used 27.77 MiB. Stream wide64 peak RSS grew from
4.39 to 4.79 MiB when two threads were enabled.

Those are explicit caller-retention experiments, not arbitrary application
memory bounds. Rust makes buffer ownership and capacity visible, but SQLite C
and native compression allocations bypass Rust's global allocator. Go's mapped
SQLite memory likewise bypasses HeapAlloc. RSS includes code, allocator pages,
stacks, libraries and consumer-owned results. Bounded parallel queues do not
provide a production shared reservation or a total concurrent memory budget;
those contracts were not ported. SQLite status counters, allocation counters,
VmRSS and maximum RSS describe different parts of the process and must not be
collapsed into an exact total-RAM guarantee.

## Final deep optimizations: smaller buffers, faster words and compiler variants

The [final report](metrics-deep-2026-10-08.md) measured the preceding optimized
Rust again (`rust_previous`) alongside the new source with its deep switches
disabled (`rust_control`) and enabled (`rust_new`). Its committed experimental
harness was `16df44bff8bc7f18313d0806492a7c8304fbc9b6`. Five passes produced
2070 complete-operation timings, 90 separate RSS processes and 20 isolated
kernel processes. Collection ran from 2026-10-08T15:27:14.519691+00:00 to
2026-10-08T15:32:44.018064+00:00. The
[environment](data/metrics-deep-2026-10-08/environment.json) preserves source,
binary and PGO-profile hashes; [raw summary/pass values](data/metrics-deep-2026-10-08/summary.json)
keep every comparison within this session.

The new switches select word-based residual/Huffman readers, reusable decode
buffers, fused decode/processing and specialized aggregate processing. All
serial variants retain the preceding exact-accumulator and codec options.
`rust_control` uses `--tuning 0`; `rust_new` and compiler variants use
`--tuning 15`. Each individual switch is compared with `rust_control`, while
the total incremental result is compared with `rust_previous`. These baselines
are distinct: turning switches off does not undo every source refactoring.

| Workload | Previous Rust, ms | New serial, ms | LTO, ms | Native CPU + LTO, ms | PGO + LTO, ms | PGO + Rayon 2, ms | Go, ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `read_sealed_full16` | 2.3561 | 1.9749 | 2.0170 | 2.0048 | 1.6565 | 1.3387 | 5.2531 |
| `aggregate_cut_avg` | 0.9232 | 0.7297 | 0.7586 | 0.6770 | 0.5583 | 0.6135 | 2.5398 |
| `aggregate_wide_cut_sum` | 2.1542 | 1.8226 | 1.8676 | 1.7916 | 1.4538 | 1.7874 | 5.2648 |
| `ingest_scrape100` | 2.5106 | 2.6740 | 2.7281 | 2.7639 | 2.5135 | 2.5693 | 6.3312 |

Read16 improved 1.19× with the new serial code over `rust_previous`, 1.42×
with PGO and 1.76× with PGO + Rayon 2. Cut avg improved 1.27× with new serial
code and 1.65× with PGO; wide cut sum improved 1.18× and 1.48× respectively.
Against Go measured in the same run, serial PGO was 3.17× faster for Read16,
4.55× for cut avg and 3.62× for wide cut sum. These are direct ratios of the
final session's medians; none multiplies earlier reports' speedups.

The switch ablation on Read16 compared control 2.5421 ms with bits-only
2.0801 ms, buffers-only 2.1926 ms and fused-only 2.2948 ms: 1.22×, 1.16×
and 1.11× respectively. All switches together took 1.9749 ms, a 1.29×
gain over control. Specialized processing alone reduced wide cut sum from
control 2.1463 to 1.9593 ms, about 1.10×. Individual improvements overlap
and cannot be added or multiplied. Same-series summary operations mostly
showed no switch gain: aggregate sum was 0.1828 ms in control and 0.1857
with all switches. Durable scrape PGO was 2.5135 ms against previous Rust's
2.5106 ms, establishing no additional benefit on that write workload.

Compiler comparisons use matching enabled switches. Portable thin LTO and one
codegen unit did not consistently improve every workload. PGO is compared with
LTO, since its build already includes LTO: the ratios were 1.22× for Read16,
1.36× for cut avg and 1.28× for wide cut sum. Native CPU targeting is likewise
compared with LTO: about 1.01×, 1.12× and 1.04× on those cases. Small changes
are noisy and native CPU targeting restricts portability. Held-out PGO cases
still use the same synthetic workload distribution; this does not prove gains
on independent production corpora. PGO + Rayon helped Read16 but increased
wide cut-sum latency from serial PGO's 1.4538 to 1.7874 ms.

The [isolated kernel processes](data/metrics-deep-2026-10-08/kernels.jsonl)
decoded identical packed input/output hashes outside SQLite. For 239 residuals,
word readers improved medians from 1.30× at width 1 to 17.65× at width 64.
Width 13 changed from 4.0062 to 0.3224 µs/op, or 12.43×. The 4096-symbol
Huffman fixture changed from 52.0849 to 15.7393 µs/op, or 3.31×. These
measurements establish the selected bit-reader gains, not equivalent whole
query gains; the complete-operation table includes the remaining storage,
planning, allocation and folding work.

[Instrumented profiles](data/metrics-deep-2026-10-08/profiles.jsonl) show work
in both snapshot fetch and decode/processing. For the new wide Read, the
profile observed 0.644 ms/op in snapshot, 1.920 in read processing and 1.267
inside block decode, with 2.608 ms/op in the whole instrumented operation.
Block decode is nested within processing: these inclusive wall-time scopes
are not additive CPU samples, and instrumentation changes their costs. They
guide attribution but do not replace the uninstrumented timing comparisons.

The profile's requested Rust allocation bytes for wide Read fell from
3,999,198 per operation in control to 2,511,902 with all switches, about 37%.
Those counters exclude native SQLite/zstd/Huffman C allocations and count
allocation requests rather than live memory. The
[RSS processes](data/metrics-deep-2026-10-08/memory.jsonl) established no
material saving over `rust_previous`: wide Read peak RSS was 27.64 MiB for
previous Rust, 27.76 for new serial and 27.39 for PGO; Read16 was 23.20,
23.40 and 23.23 MiB respectively. Caller-owned output remained a substantial
retained footprint. Lower allocation churn must not be presented as an equal
RSS reduction or an exact total-memory budget.

## Low-level SQLite extension plan (unmeasured)

Documentation addendum, 2026-10-08. This proposal adds no implementation or
benchmark samples. It describes the next experiments beyond the already
measured native-SQLite backend and Rust optimizations.

The working hypothesis is that a small TinyStore-owned SQLite adapter can
reduce allocation/copying costs on selected paths and give native SQLite a
place in the production memory budget. Further engine-level CPU gains are
plausible where those costs matter. Replacing thin rusqlite calls with raw C
calls alone has no demonstrated speed benefit; a custom allocator is primarily
a memory-admission and failure-control candidate and can add overhead.

### Adapter boundary and supported access

Rusqlite 0.40.1 reexports libsqlite3-sys as `rusqlite::ffi` and exposes the
underlying connection through unsafe `Connection::handle()`. Missing high-level
operations can therefore call SQLite's documented C API. Keep normal operations
on safe rusqlite interfaces and place pointer ownership, callback state,
initialization and error translation inside one audited adapter. Connection,
statement and buffer lifetimes must remain explicit; a raw call must preserve
rusqlite's ownership, statement-cache and threading invariants.

The adapter would own the pinned static SQLite build, startup-only global
configuration, per-connection configuration, statement/transaction lifetime,
memory accounting and admission, diagnostics and cancellation. The engines
would consume safe operations and bounded owned snapshots. Version, source ID,
compile options, page sizes and PRAGMAs remain checked at startup. Native
THREADSAFE=1 and exclusively owned NO_MUTEX connections remain the baseline.
The current build already controls SQLite's flags and embeds the library;
low-level access does not require a dynamic SQLite dependency or a fork of its
internal VDBE/pager code.

Several proposed facilities already have safe wrappers: `Row::get_ref()`
borrows column values, `prepare_cached()` already uses
SQLITE_PREPARE_PERSISTENT, and the enabled hooks expose a progress handler and
an interrupt handle. Raw access is an extension point for missing facilities,
not evidence that those existing operations need to be replaced.

### Candidates and engine implications

| Candidate | Mechanism to separate in an experiment | Plausible benefit | Constraint to preserve |
| --- | --- | --- | --- |
| Native allocation accounting and admission | SQLite status counters first; then SQLITE_CONFIG_MALLOC callbacks that account for rounded allocation sizes and enforce a shared budget | Native memory becomes visible and rejectable; lower fragmentation or allocator cost is a separate hypothesis | Global configuration precedes initialization; realloc failure preserves the old allocation; OOM and transaction recovery remain correct; callbacks cannot unwind into C |
| Lookaside and page-cache buffers | Per-connection lookaside plus separately measured SQLITE_CONFIG_PAGECACHE capacity | Fewer general-purpose allocations during prepare/step, potentially less CPU and variance | Preallocated pools consume reserved RAM; page-cache exhaustion falls back to heap allocation; tuning must include every connection and engine |
| Borrowed rows copied into a bounded shared arena | Read validated BLOB views with get_ref(), copy into one admitted batch arena, and retain ranges after closing the snapshot | Fewer per-payload Rust allocations and less capacity/metadata overhead; especially plausible for scans of many blocks | Each borrowed value is consumed before the next step/reset/finalize; short snapshots, corruption/limit precedence and Rayon ownership remain intact; required byte copies are still present |
| Scoped static input binding | Measure sqlite3_bind_blob64 with SQLITE_STATIC against the current transient binding path for sufficiently large buffers | Potentially removes one parameter-binding copy on head/body writes | Input storage survives until rebinding, clear_bindings or finalize; reset alone does not release the binding; pager/storage copies and FULL commit work remain |
| Statement diagnostics and narrowly chosen raw loops | Measure VM steps, scans, sorts, reprepares, statement/cache memory and wrapper-only costs before changing prepare/bind/step | Identifies avoidable SQL/planner or conversion work on short operations | Existing statement reuse/PERSISTENT is already baseline; direct step is not automatically faster |
| SQLite C compiler variants | Hold version/options fixed and separately compare the current -O2 archive with supported optimization/profile-guided C builds | Possible native VM/pager CPU improvement shared by engines | The previous Rust PGO/LTO experiments did not optimize the precompiled SQLite C archive; build/link compatibility and format/error checks must be repeated |
| Custom VFS or page-cache implementation | Isolate a concrete I/O, instrumentation or cache-policy requirement before replacing an existing implementation | Useful only if a measured bottleneck or required policy justifies it | WAL, locking, syncing, file lifetime and platform behavior need their own fault/concurrency evidence |

The short-snapshot boundary is particularly important. Metrics currently
materializes payloads into owned Vec buffers before decode/parallel work.
Production records explicitly decodes only after releasing its snapshot.
Borrow-and-decode while a SQLite row is live would change that policy; it can
be an isolated experiment, but a production candidate must preserve the existing
snapshot contract or establish a separately reviewed replacement. The first
compatible experiment is a bounded owned arena, not references to SQLite memory
handed to Rayon after the statement or connection has released it.

| Engine/workload | Where additional gains are plausible | Current limit on that expectation |
| --- | --- | --- |
| Metrics Read/Stream and decoded cut aggregates | Payload acquisition/ownership, batch arenas, native allocation traffic and SQLite C CPU | Whole-block summary queries do little decode work; output materialization remains; existing native-backend gains are already in the baseline |
| Records block scans and stream/page assembly | Fewer allocations while fetching packed bodies, reused batch storage and efficient column decoding | No complete native records port was measured; decode must stay outside the snapshot and pagination/budget/corruption behavior must match |
| KV and sqldb short point operations | Statement/conversion overhead, planner work and lookaside behavior if diagnostics identify them | Small values offer little copy savings; statements are already cached; the current records/metrics results do not quantify these engines |
| Blobs metadata and inline contents | Metadata allocation costs and bounded owned inline-body acquisition | Large bodies use external files, so SQLite-call changes do not accelerate their bulk file reads; inline readers retain checked bytes and survive replacement/deletion/store close |
| Durable writes in every engine | Large-input binding copies and native prepare/step work | WAL/FULL commit, syncing and checkpoint costs remain; the final scrape experiment found no additional query-switch benefit |

Incremental BLOB I/O is another available API, but it is not a generic shortcut.
SQLite excludes WITHOUT ROWID tables; TinyStore has both rowid body tables and
WITHOUT ROWID key/metadata tables. The blobs reader's current ownership and
integrity contract also prevents treating a retained SQLite BLOB handle as an
equivalent drop-in replacement for an owned inline buffer or open file.

### What memory control would and would not establish

`sqlite3_status64`, `sqlite3_db_status` and allocator callbacks answer different
questions. SQLite's hard heap limit is shared by all connections using that
library instance, not a per-engine or per-query reservation and not a cap on
process RSS. Caller-provided page-cache/lookaside storage must also be counted
in the application's budget; supplied page-cache pools and disabled memory
accounting can prevent the stock heap limit from covering all relevant memory.
A shared native budget needs engine/query admission and capacity for writer
progress as well as accounting of Rust outputs, codec scratch and native zstd.

The measured 16 retained Read16 results already contain 18.75 MiB of timestamp
and value data before containers. An SQLite allocator change cannot remove that
caller-owned content. Lower native allocation traffic, lower peak RSS and a
correct memory limit are separate outcomes to report.

### Next measurement and expected size of the effect

Prioritize native diagnostics, a contract-preserving batch arena, then
lookaside/page-cache ablations. Treat the tracking allocator as a separately
measured budgeting feature before assuming it accelerates requests. Raw
statement loops, static binding, C compiler variants and custom VFS/cache work
follow only where their individual costs or requirements justify the effort.

The existing inclusive profiles show snapshot and decode/process work, but do
not isolate rusqlite overhead or native malloc cost. They cannot provide a
percentage forecast for this proposal. As arithmetic only, if a separately
measured component occupies 25% of a call and becomes twice as fast, total
speed improves to 1 / (0.75 + 0.25 / 2) = 1.14x. This is an illustration, not
an estimated TinyStore gain. There is no basis here to promise another broad
multi-times speedup from switching to raw calls.

Commit each candidate harness first and compare it with the current optimized
Rust/native baseline in the same quiet session, one change at a time. Keep
SQLite version, SQL, schema, PRAGMAs, compiler settings, affinity, input traces
and result lifetimes equal. Include the fixed-cache fixtures and independent
corpora with larger/irregular blocks and realistic label/payload distributions.
Measure latency distributions, CPU per call, Rust and native allocation traffic,
SQLite/cache counters and current/peak RSS under serial and concurrent load.
Arena/borrow experiments must also record snapshot duration, WAL growth and
checkpoint behavior. For a budgeted allocator, record admission/OOM refusals,
writer progress and recovered connection/transaction state; injected allocation
failures, cancellation, rollback and corruption checks are acceptance gates.
Durable write trials retain WAL/FULL. Any storage-layout candidate reports
payload bytes and per-object file pages separately.

The hypothesis is strongest for allocation/copy-heavy block processing and for
shared native memory admission. Small cached operations and sync-bound writes
may see little benefit. Each engine needs its own public-path evidence before
these facilities become production defaults.

API references: SQLite [global configuration](https://sqlite.org/c3ref/config.html),
[allocator methods](https://sqlite.org/c3ref/mem_methods.html),
[heap limits](https://sqlite.org/c3ref/hard_heap_limit64.html),
[column-value lifetimes](https://sqlite.org/c3ref/column_blob.html),
[binding lifetimes](https://sqlite.org/c3ref/bind_blob.html) and
[incremental BLOB access](https://sqlite.org/c3ref/blob_open.html).
The locked rusqlite/libsqlite3-sys versions and native build flags are retained
in [Cargo.toml](../metrics-max-bench/rust/Cargo.toml),
[Cargo.lock](../metrics-max-bench/rust/Cargo.lock) and the
[native build record](data/metrics-deep-2026-10-08/native-build.json).

## Remaining decision gates and reproduction

The measured paths preserve formats, exact arithmetic and several public error
contracts. Production directory locking/lifecycle, reader pooling/admission,
shared memory reservations, caller cancellation, instruments, background work,
snapshots/backups and server APIs remain outside the Rust comparison. Full
expiry does not establish partial-retention/group-merge/quarantine fault or
concurrency correctness. The complete records engine has not been ported and
measured by these metrics experiments.

A migration decision still needs representative corpora and contention,
cancellation, recovery and cross-platform behavior. First carry forward
useful Go algorithm backports and measured Rust buffer/processing improvements,
then evaluate concurrent load with a
shared bounded pool and explicit memory admission. FULL durability must remain
part of storage comparisons; these runs do not establish power-loss safety or
disk-independent write speedups.

Reproduction starts from `<repo>`, this research checkout, with
`tinystore/source` pinned to the baseline commit above. Each linked report gives
its environment, commands, interval and raw data; the corresponding harness
instructions are [rust-spike](../rust-spike/README.md),
[sqlite-bench](../sqlite-bench/README.md),
[metrics-bench](../metrics-bench/README.md), and
[metrics-opt-bench](../metrics-opt-bench/README.md), followed by the final
[metrics-max-bench](../metrics-max-bench/README.md). Do not treat figures from
different sessions as a new paired comparison. The shared AMD EPYC 9V74 VM,
Go 1.27.1, Rust 1.99.0, two-CPU quota and 8 GiB memory limit define the observed
environment; they do not define a deployment guarantee.
