# Records kernels and a compatible native SQLite slice — 8 October 2026

This extends the [Rust kernel round](rust-kernels-2026-10-07.md) and
[native metrics research](metrics-native-2026-10-08.md) into records. It is
an exploratory synchronous slice, not a production Rust port or a prediction
of concurrent application throughput. The complete Go reader keeps important
advantages: selective reads, small pages and cached Follow are faster than
this native slice, and retaining the native slice's owned strings uses more
process memory.

The [9 October query and ownership round](records-optimization-2026-10-09.md)
implements selective materialization, bounded page selection, shared owning
backing and distinct raw/decoded Follow caches, and remeasures Go, the old
native code and the new variants in the same session. The regressions below
describe this original prototype; they are not language-performance limits.
Its original measurements remain unchanged.

| Question | Answer in this round | What follows |
|---|---|---|
| Does a different bit-reader algorithm help both languages? | A word reader improves 64-bit residual decode 4.14× in Go and 3.89× in Rust, but makes 1-bit and Rice decode slower. | Consider a width-dependent Go reader first; do not replace every reader with the word reader. |
| Can a native slice keep actual records contracts and files? | Go and Rust cross-read v1 head rows; both read identical native-produced v1 sealed rows, including typed JSON spellings, repeated keys, absent fields, trace/span, ordering, pages, budgets and Follow. | Compatibility is demonstrated for the implemented encodings; arbitrary production sealed columns remain unsupported. |
| Is native faster for every records operation? | Full reads favor native here; selective reads, small pages and Follow favor the production Go engine. Append is nearly equal. | Preserve query pruning, result ownership and Follow caching when evaluating a complete native engine. |
| Is the simpler native sealed writer equivalent? | It is faster on these fixtures, but its block payload is 1.47–1.66× the production encoder's. | Treat it as an encoding tradeoff, not an equivalent-algorithm language result. |
| Does native automatically reduce memory? | No: 64 retained 4096-record results use about 336 MiB current native RSS, versus 269–286 MiB for Go. | String/container representation needs its own study. SQLite memory and Rust allocation counters are separate categories. |

## Environment and reproduction

Production baseline: TinyStore
`e307c48a40126aad0e2873b6bf3aaedef8115483`, checked out in the research
submodule. Go uses the actual public `records.Append`, `Maintain`, `Scan`
and `Follow` API, including its runtime and SQLite adapter. The code, tests
and runners are retained in [records-native-bench](../records-native-bench/).
The Apache-2.0 source attribution is retained in its `NOTICE` and `LICENSE`.
No production code, schema, protocol or format version changed.

The user explicitly waived committing the harness before measurement. This
is **exploratory uncommitted collection**. Exact measured file hashes,
binary hashes, compiler versions, SQLite build flags and corpus hashes are
in [environment.json](data/records-native-2026-10-08/environment.json).
The supplemental kernel runner verifies every original main-run source hash
and both timing-binary hashes before and after its run.
The later-added `kernel_recheck.py` has its own hash in the supplemental
environment file; it is absent from the earlier main-run source inventory.
The report and `retained-content.json` were assembled afterward from retained
data and fixture content and are not additional performance runs.

- AMD Ryzen 7 7700, 8 cores/16 threads, Docker Desktop's WSL2 Linux
  `6.18.33.2-microsoft-standard-WSL2`, x86-64, glibc 2.41. CPU affinity 0;
  no cgroup CPU or memory cap. No research builds/tests or competing agent
  timings ran during the granted windows. Background desktop/VM activity was
  not independently controlled; the ratios remain exploratory.
- Go 1.27.1, `CGO_ENABLED=0`, `GOMAXPROCS=1`, default GOGC 100; Rust 1.99.0
  (`b940084d7`, LLVM 23.1.1), ordinary portable release build. A separate
  Rust build counts allocator calls. No unsafe custom SQLite adapter,
  wrapper fork, LTO, host-target tuning or PGO was used.
- Stock rusqlite 0.40.1 and external static SQLite 3.53.4, built with cc
  14.2.0 and the previous round's matched SQLite flags. The official
  amalgamation archive SHA-256 is
  `1e71ddf93849c6a6ecf58b827c0692073d2dd7ee40196158068f7b29f422e87d`;
  `sqlite3.c` is
  `b1dd5d74ec7f29055a6684fa06fb3c2f6821c87dd38f9a458dfd2e8a1db28189`.
  Native intentional differences remain THREADSAFE=1, Unix VFS and automatic
  initialization. The Go dependency pins wasm SQLite 3.53.4 as well.
- Native zstd is 1.5.7 via zstd 0.13.3; production Go uses
  klauspost/compress 1.20.1. Compression stream/implementation differences
  are part of the engine and writer comparisons.
- Both paths retain FULL synchronous WAL durability, 1024-byte records
  pages, 1 MiB cache per connection, foreign keys, 5-second busy timeout,
  fullfsync and checkpoint_fullfsync. Native uses one writer and one reader;
  production has its normal two-reader pool. Native starts from an empty
  database created by Go, retaining the production migration history.
- Stores, fixtures and build targets live on the Linux volume at `/work`,
  never on the Windows bind mount. One process runs each case. These
  sequential wall-time observations describe this VM session, not
  bare-Linux request latency or concurrent throughput.

The deterministic fixture has 4096 records across two streams, deliberately
unordered event times and equal-time pairs. It includes absent/present
level/body, Unicode text, trace/span IDs, two context fields and five
attributes, including a repeated key and nested JSON with exact spelling.
The second fixture replaces present bodies with the public Loghub Apache
2k sample, cycling it into the same 4096-record trace. Its SHA-256 is
`c7efa3eb686e3a96bd2f8f4457b2a7887e9cf2f3649327f1b4e87af841363ce8`,
pinned in [the corpus manifest](../bench/loghub-sha256.txt). No private
corpus was used. GH Archive was available and hashed in the environment,
but was not used by this records fixture.

Main collection ran **17:39:52–17:40:53 UTC**, six passes with alternating
Go/Rust order. Each engine read ran 32 iterations; append ran 16 × 256
records after four warm operations, and seal ran once on a fresh 4096-record
head. The initial 2048-operation kernels produced some 2.7–7 ms samples.
They remain in the raw output, but are not the kernel headline.

The separate kernel collection ran **17:42:34–17:43:35 UTC**, with unchanged
timing binaries, six balanced pairs and one fixed count per case shared by
Go and Rust. Counts target 150 ms using the faster initial median, capped
at 131072; actual samples span **137.97–1056.20 ms**. The calibration,
script hash and binary hashes are retained in
[kernel-recheck-environment.json](data/records-native-2026-10-08/kernel-recheck-environment.json).

From a container mounting `<repo>` at `/src`, with the public Apache sample
prepared separately as described by the prototype README:

```sh
export PATH=/work/cargo/bin:$PATH
python3 /src/tinystore/sqlite-bench/build_native.py
python3 /src/tinystore/records-native-bench/prove_guard.py
python3 /src/tinystore/records-native-bench/build.py
python3 /src/tinystore/records-native-bench/run.py --prepare
# Stop all other builds, tests and measurements before either command:
python3 /src/tinystore/records-native-bench/run.py --measure --passes 6
python3 /src/tinystore/records-native-bench/kernel_recheck.py
```

## What compatibility covers

The native slice implements atomic append and head publication with the
production schema and v1 head format, retention/skew checks on append,
JSON validation, record/batch limits, stream/late routing, zstd bounds and
CRC. It seals one bounded segment per `(stream, late)` head, preserving
equal-time arrival order; the transaction writes the segment, blocks, keys
and trace blooms and removes its heads together. Reads fetch owning bytes
from one snapshot and decode after that transaction has ended.

The segment writer chooses valid existing v1 fallback representations:
raw-attribute shapes, raw-length text and direct-width integers. This keeps
all field spellings and IDs, but does not reproduce production's typed
compression choices. Rust deliberately rejects arbitrary Go-sealed typed,
dictionary, radix, Rice, FSE and stamped columns. Its strings also restrict
the slice to valid UTF-8; arbitrary byte strings supported by Go are outside
this round. The schema and format were not silently replaced with an easier
private format.

The compatibility controls are therefore distinct:

| Label | Physical artifact both readers receive | What the comparison includes |
|---|---|---|
| `go-head` | The same production-Go-created head database | Go public reader versus the synchronous native head reader |
| `native-sealed` | The same native-produced fallback v1 segment/block database | Both readers on identical schema, payload, compression and SQLite pages |
| append | Copies of the same Go-created empty database and the same records | Different complete append implementations, including adapters and compression |
| seal | Copies of the same production-Go head database | Different sealed encoding strategies; sizes must accompany time |

The native reader materializes owned strings and sorts matching rows. The
Go engine has adaptive candidate pagination, column pruning, a bounded
selection heap, detached page values and a Follow cache. Those are real
implementation differences, so these engine ratios do not isolate language,
native SQLite or safe rusqlite wrapper overhead.
Its decompression expansion limit is per blob. It does not carry production's
cumulative schema/block expansion accounting across columns; malformed files
with recomputed checksums are therefore outside the safe-reader claim.

Not implemented in Rust: runtime lifecycle/lock/migration ownership,
background handler and Lines flushing, cancellation, shared memory admission,
concurrent/grouped writes, production's adaptive candidate walks and selection
algorithm, Follow caching and general Follow budget scheduling, merged
holders/expired-cursor accounting, retention deletion, Damage/Drop, or the
full sealed codec. Follow operates on the small unmerged fixtures within
the default byte budget. No concurrency or production readiness claim follows.

## Algorithms: wide reads improve, narrow and Rice reads do not

Each operation owns its output. Width and Rice kernels contain 1024 values;
both languages use identical fixtures and produce identical bytes. The word
reader keeps a 64-bit accumulator instead of extracting byte fragments. The
sort kernels order **indices** by event time: stable ordering is compared
with an explicit `(time, arrival index)` key. They do not measure moving
complete production Record structures. Head serialization encodes 1024
records before compression or SQL.

Calibrated medians, microseconds per operation; ratio is Go/Rust, so a ratio
above one favors Rust. Every case's six passes are retained in
[kernel-recheck-summary.json](data/records-native-2026-10-08/kernel-recheck-summary.json).

| Kernel | Go µs | Rust µs | Go/Rust |
|---|---:|---:|---:|
| width 1 encode | 1.88 | 1.37 | 1.37 |
| width 1 decode | 2.78 | 1.47 | 1.90 |
| width 1 word decode | 3.10 | 2.61 | 1.19 |
| width 7 encode | 3.00 | 1.49 | 2.01 |
| width 7 decode | 4.13 | 2.54 | 1.62 |
| width 7 word decode | 3.31 | 2.74 | 1.21 |
| width 17 encode | 4.38 | 1.77 | 2.47 |
| width 17 decode | 6.48 | 4.28 | 1.52 |
| width 17 word decode | 3.86 | 2.97 | 1.30 |
| width 64 encode | 5.28 | 1.68 | 3.15 |
| width 64 decode | 15.89 | 11.20 | 1.42 |
| width 64 word decode | 3.84 | 2.88 | 1.33 |
| Rice small encode | 6.95 | 3.10 | 2.24 |
| Rice small decode | 9.71 | 4.46 | 2.18 |
| Rice small word decode | 17.04 | 15.16 | 1.12 |
| Rice escapes encode | 17.25 | 4.53 | 3.81 |
| Rice escapes decode | 40.76 | 22.20 | 1.84 |
| Rice escapes word decode | 70.13 | 67.24 | 1.04 |
| Head record serialization | 139.29 | 56.70 | 2.46 |
| Stable index sort | 88.01 | 12.43 | 7.08 |
| Explicit arrival-key index sort | 44.22 | 11.37 | 3.89 |

The algorithm change matters independently of language. Word extraction
improves width 64 by 4.14× in Go and 3.89× in Rust; width 17 by 1.68× and
1.44×. It makes width 1 1.12×/1.78× slower, and Rice escapes 1.72×/3.03×
slower. Explicit arrival keys improve the Go index-sort kernel 1.99× and
the Rust one 1.09×. These are candidates for complete-path experiments,
not evidence that production should always choose these algorithms.

## Complete paths: retaining production pruning and caching matters

Six-pass medians, milliseconds per synchronous operation. `full` returns
4096 records. `filtered` combines an exact attribute, context and minimum
level and returns 274 records. `page`/`newest` request 127 and return 126
because timestamps are not split. `trace` returns one record. Search is
case-insensitive `COMPLETED`: it returns 3510 typed-fixture records and no
Apache records. Follow returns the first 127 records in publication order.

| Fixture/artifact | Operation | Go ms | Native ms | Go/native |
|---|---|---:|---:|---:|
| Typed / Go head | full | 7.155 | 3.994 | 1.79 |
| Typed / Go head | filtered | 2.598 | 4.430 | 0.59 |
| Typed / Go head | search | 7.192 | 4.648 | 1.55 |
| Typed / Go head | trace | 2.236 | 2.948 | 0.76 |
| Typed / Go head | page | 2.407 | 3.598 | 0.67 |
| Typed / Go head | newest | 2.477 | 3.761 | 0.66 |
| Typed / native sealed | full | 8.457 | 3.242 | 2.61 |
| Typed / native sealed | filtered | 1.324 | 3.881 | 0.34 |
| Typed / native sealed | search | 8.738 | 4.341 | 2.01 |
| Typed / native sealed | trace | 0.282 | 0.823 | 0.34 |
| Typed / native sealed | page | 1.465 | 3.353 | 0.44 |
| Typed / native sealed | newest | 2.060 | 3.590 | 0.57 |
| Typed / native sealed | Follow 127 | 0.305 | 1.493 | 0.20 |
| Apache / Go head | full | 7.446 | 4.023 | 1.85 |
| Apache / Go head | filtered | 2.594 | 4.249 | 0.61 |
| Apache / Go head | search | 2.979 | 3.759 | 0.79 |
| Apache / Go head | trace | 2.258 | 3.453 | 0.65 |
| Apache / Go head | page | 2.629 | 3.760 | 0.70 |
| Apache / Go head | newest | 2.604 | 3.824 | 0.68 |
| Apache / native sealed | full | 8.486 | 3.261 | 2.60 |
| Apache / native sealed | filtered | 1.452 | 4.140 | 0.35 |
| Apache / native sealed | search | 3.148 | 3.430 | 0.92 |
| Apache / native sealed | trace | 0.295 | 0.856 | 0.34 |
| Apache / native sealed | page | 1.505 | 3.320 | 0.45 |
| Apache / native sealed | newest | 2.099 | 3.376 | 0.62 |
| Apache / native sealed | Follow 127 | 0.333 | 1.559 | 0.21 |

The full-read advantage is not a universal engine advantage. The slice's
eager owned-row approach is faster on these full scans, but pays for rows
the complete Go query path can avoid rebuilding. Go's warm Follow cache is
especially valuable. These mechanisms are implementation facts; separating
their CPU contribution from adapter/native/backend effects requires further
matched variants rather than an explanation attached to one ratio.

All six pass values are in [summary.json](data/records-native-2026-10-08/summary.json)
and [timings.jsonl](data/records-native-2026-10-08/timings.jsonl). The pass
table below records representative complete paths rather than hiding spread
behind their medians.

| Typed complete path | Go ms, six passes | Native ms, six passes |
|---|---|---|
| scan_full on native-sealed | 8.153; 8.792; 7.914; 9.120; 8.466; 8.447 | 3.590; 3.186; 3.504; 3.240; 3.245; 3.231 |
| scan_filtered on native-sealed | 1.288; 1.416; 1.303; 1.483; 1.346; 1.258 | 3.877; 3.870; 3.999; 4.062; 3.884; 3.877 |
| scan_page on native-sealed | 1.488; 1.591; 1.430; 1.701; 1.432; 1.442 | 3.384; 3.234; 3.341; 3.365; 3.648; 3.218 |
| follow on native-sealed | 0.406; 0.325; 0.270; 0.576; 0.284; 0.285 | 1.448; 1.517; 1.492; 1.495; 1.509; 1.471 |

## Append and seal: close append times, different sealing economics

| Fixture | Operation | Go ms | Native ms | Interpretation |
|---|---|---:|---:|---|
| Typed | append 256 records | 3.736 | 3.562 | 1.05× local ratio; complete paths and compression differ |
| Apache | append 256 records | 3.741 | 3.597 | 1.04× local ratio; complete paths and compression differ |
| Typed | seal 4096 records | 30.415 | 19.722 | Different encoders; the native fallback writes more bytes |
| Apache | seal 4096 records | 36.891 | 20.898 | Different encoders; the native fallback writes more bytes |

Append traces, timestamp distribution, records and result lifetimes are
matched. The seal comparison starts from the same Go head file, but the
production planner/typed codec and native fallback writer are deliberately
different. Do not quote their 1.54×/1.77× time ratios as an equivalent Rust
seal speedup.

Payload alongside real files and b-tree ownership, all bytes; no VACUUM was
run. The native head's zstd bytes can differ while decoding identically.

| Fixture/artifact | Head payload | Segment + block payload | Blocks b-tree | All listed live b-trees | Whole file | File outside live b-trees |
|---|---:|---:|---:|---:|---:|---:|
| Typed / Go head | 78,287 | 0 | 1,024 | 106,496 | 106,496 | 0 |
| Typed / native head | 78,196 | 0 | 1,024 | 106,496 | 106,496 | 0 |
| Typed / Go sealed | 0 | 30,495 | 31,744 | 57,344 | 120,832 | 63,488 |
| Typed / native sealed | 0 | 49,776 | 50,176 | 75,776 | 131,072 | 55,296 |
| Apache / Go head | 92,073 | 0 | 1,024 | 117,760 | 117,760 | 0 |
| Apache / native head | 91,049 | 0 | 1,024 | 116,736 | 116,736 | 0 |
| Apache / Go sealed | 0 | 41,761 | 41,984 | 67,584 | 138,240 | 70,656 |
| Apache / native sealed | 0 | 60,882 | 62,464 | 88,064 | 147,456 | 59,392 |

The fallback adds 18,432 bytes to the typed blocks b-tree and 20,480 bytes
to Apache's; the other listed live b-trees have the same size. Whole files
grow less because free pages left by the deleted head differ. Payload,
live b-trees and the allocated file therefore tell different parts of the
story. [storage.json](data/records-native-2026-10-08/storage.json) retains
every table and index, not just the totals.

## Allocations, native counters and retained owning results

These allocation observations use separate 256-operation kernel and
8-operation engine diagnostic runs. Rust's System allocator counter excludes
C allocations by SQLite and zstd; Go's runtime counter covers Go allocations.
Zeros in the uninstrumented Rust timing rows mean **counter disabled**, not
zero allocations. No allocator-instrumented Rust binary supplies timing
headlines.

| Typed artifact / operation | Go allocations/op | Rust allocations/op | Go allocated bytes/op | Rust allocated bytes/op |
|---|---:|---:|---:|---:|
| Go head / full | 66,520.5 | 81,553 | 9,737,393 | 11,481,478 |
| Go head / filtered | 18,747.8 | 81,562 | 3,605,202 | 8,683,760 |
| Go head / page | 22,044.5 | 81,553 | 3,600,083 | 11,481,478 |
| Native sealed / full | 100,958.4 | 88,819 | 11,078,639 | 12,537,133 |
| Native sealed / filtered | 13,963.3 | 88,828 | 2,492,448 | 9,739,415 |
| Native sealed / page | 27,300.4 | 88,819 | 2,499,213 | 12,537,133 |
| Native sealed / Follow 127 | 1,719.5 | 44,408 | 843,658 | 5,227,660 |

Native SQLite counters are recorded before and after every engine case. For
the warmed typed sealed full read, a representative 32-operation process
records 320 additional cache hits and no additional misses; 18 misses occurred
before the timed interval. SQLite reports 24,400 bytes of combined connection
cache, 14,464 bytes of statements and 192,736 current/205,320 peak bytes
through its own memory API. These figures exclude the returned Rust strings
and zstd memory and are not process memory.

The RSS diagnostic retains **64 complete 4096-record results**: 262,144
records, **47,121,856 bytes (44.94 MiB) of logical content**. The logical
definition counts timestamp/present level/IDs and UTF-8 field content, and
excludes container headers, length tags and allocation metadata. It is
retained in [retained-content.json](data/records-native-2026-10-08/retained-content.json).
Every returned record continues to own its values while later reads run.

| Process | Current RSS KiB, three passes | GNU time peak RSS KiB, three passes |
|---|---|---|
| Go | 293,240; 287,788; 275,560 | 293,516; 288,268; 276,104 |
| Native Rust | 343,960; 343,972; 343,956 | 343,868; 343,924; 343,944 |

These are inclusive process measurements: runtime, store, SQLite, compression,
scratch and retained results. Current RSS reads `/proc/self/status`; peak
comes from GNU time (`ru_maxrss`). The small native current/peak discrepancy
is retained in the raw data, so their difference is not used as a measurement.
Neither is an incremental result-size measurement. Go's reported `heap_alloc`
in the raw memory rows was captured before retention, so it is not used as
the retained graph's heap size. The native slice's owned String/Vec graph
uses more retained process memory on this fixture; a native backend alone
does not establish a memory advantage.

## Verification and limits

[build.json](data/records-native-2026-10-08/build.json) retains the successful
Linux Go tests, Rust release tests, formatting checks, builds, static-link
inspection and hashes. There are two Go tests and seven Rust tests. No
production repository code changed, so this round did not run the complete
production `task check`, SDK suites or platform/race matrices. Windows and
macOS runtime behavior were not measured.

The retained correctness runner establishes:

- Identical bytes for all 21 owning kernel outputs, including optimized vs
  baseline readers and arrival-order sort candidates; shifts by 64, signed
  wrapping, truncated width/Rice input and Rice trailing-byte rejection.
- Matching canonical record hashes, counts, page boundaries and Follow
  cursors for typed and Apache Go heads, native heads and native sealed
  artifacts; Go production-sealed artifacts reject explicitly in Rust.
- Complete 33-page oldest/newest typed walks of 4096 records, and a 34-batch
  Follow walk of the sealed fixture. Equal-time pairs keep arrival order;
  a requested limit of 127 never splits their timestamp.
- A mixed path where Go appends four records 20 seconds behind waiting
  heads; SQL confirms four records in the late head. Native sealing publishes
  each stream's old-time records in its later segment. Both readers walk all
  4100 records and match publication-order Follow hashes separately from
  event-time Scan hashes. Segment IDs/times are retained in
  [verification.json](data/records-native-2026-10-08/verification.json).
- Future-time, invalid JSON and excessive-limit input rejection; two head
  and two block mutations (CRC and index count), rejected by both readers.
  Native decoders additionally enforce count/length, expansion/window,
  width/reference, ordering and trailing-column checks for the implemented
  encodings.
- An intentional mutation that disables native head CRC verification causes
  the checksum-only negative test to fail. Exact original source bytes are
  restored and the same test passes;
  [guard-proof.json](data/records-native-2026-10-08/guard-proof.json) records
  both outcomes and identical before/after source hashes.

The raw data and complete reproduction code are retained. The reader and
format restrictions above are material: this does not settle arbitrary
production corpus compression, steady-state maintenance, concurrent
throughput, crash/restart durability, cancellation, memory admission or
long-lived retention/merge behavior.

## What follows

The immediately portable result is algorithm-specific: test a word residual
reader on sufficiently wide columns in Go, retaining the narrow/Rice reader,
then measure complete production blocks. An explicit arrival-key index
strategy is another Go candidate, but needs a complete seal experiment that
includes its extra key storage and record movement.

A deeper native records candidate should keep the full typed codec, query
pruning and Follow cache and use an ownership representation that does not
duplicate every small String. This round establishes the compatible head
and restricted sealed slice, finds where its tradeoffs matter, and retains
the measurements needed to compare that next candidate in the same session.
