# Records: native backend and algorithm research

An exploratory synchronous Rust slice against TinyStore
`e307c48a40126aad0e2873b6bf3aaedef8115483`, plus a runner using the actual Go
public `records` API. No production code changes. Apache-2.0 attribution is
in `NOTICE` and `LICENSE`.

`go/` exports deterministic records, makes production fixtures, scans and
follows through the public API, measures Go allocations, and divides closed
files by b-tree using TinyStore's `internal/dbstat`. `rust/` uses stock
rusqlite 0.40.1 over the matched external static SQLite 3.53.4 archive and
native zstd 1.5.7. Safe rusqlite wrapper work remains in every native engine
operation. Timed binaries do not count Rust allocations; a separately built
`counting-allocator` binary does. SQLite's allocator is outside that Rust
counter and has its own current/peak memory and cache counters.

## Implemented slice

- Atomic synchronous append, JSON validation, time/record/batch bounds,
  stream/late routing, bounded head rows, production v1 head serialization,
  zstd frame and CRC; exact absent/present level/body, trace/span and ordered
  repeated context/attribute keys. Native and Go compression streams differ.
- One bounded segment per `(stream, late)` head, stable event-time ordering,
  atomic publication/deletion, segment keys, trace blooms, production schema,
  blocks and Follow cursor. The native segment writer deliberately uses v1's
  raw-attribute shapes, raw-length text and direct-width integer fallback.
- Snapshot fetch followed by decoding after the transaction, time/level and
  trace candidate checks, exact filters, case-insensitive text search,
  oldest/newest pages without splitting a timestamp, block/byte/decode budget
  edges and a sequential Follow walk over unmerged holders.
- CRC, counts, lengths, integer width, expansion, context/ID references,
  column trailing bytes and stored index/body consistency guards. zstd
  windows are capped at 256 KiB and decoded head/text at 4 MiB.

This is **not a production port**. Rust strings restrict the slice to valid
UTF-8. It cannot read arbitrary Go-sealed typed, dictionary, FSE, Rice,
radix or stamped columns. It accepts the compatible fallback columns it
writes and production Go heads; unsupported encodings fail explicitly.
Its sealed-read comparisons give both readers the *same native-produced
v1 bytes*. Production Go Maintain versus native fallback seal is a comparison
of different encoding strategies, with sizes beside time, not a language or
equivalent-seal speedup.

Not implemented: runtime lifecycle/locking/migration ownership, background
flush and maintenance, the slog handler/Lines interfaces, cancellation,
shared memory admission and concurrency, grouped commits, adaptive candidate
pagination, production query's column pruning/selection heap, Follow cache,
holder merges and expired-cursor accounting, retention deletion and Drop.
The reader handles the fixed small fixtures within default Follow byte
bounds; it does not implement a general Follow budget scheduler. It uses
two connections and the same FULL durability, 1024-byte pages, 1 MiB cache
per connection and other connection PRAGMAs as Go. Native storage is seeded
from an empty public-Go database, preserving the migration history exactly.

## Reproduce

Use the Linux container/toolchain described by the dated report. Mount this
research checkout at `/src`, a Linux volume at `/work`, and put the pinned
public Loghub Apache sample at
`/work/records-native/corpus/Apache/Apache_2k.log`; the parent
`bench/fetch-record-events.py` and `bench/loghub-sha256.txt` retain public
corpus provenance. Corpus bytes are fetched, not committed. The container
needs Go 1.27.1, Rust 1.99.0 at `/work/cargo/bin`, Python 3, cc, GNU time,
taskset and a populated Cargo cache. Prepare the matched archive with
`python3 /src/tinystore/sqlite-bench/build_native.py`.

```sh
export PATH=/work/cargo/bin:$PATH
python3 /src/tinystore/records-native-bench/prove_guard.py
python3 /src/tinystore/records-native-bench/build.py
python3 /src/tinystore/records-native-bench/run.py --prepare
# Only after all builds/tests and other measurements have stopped:
python3 /src/tinystore/records-native-bench/run.py --measure --passes 6
```

`--prepare` cross-checks owning output bytes for 21 kernels, six head/sealed
reader fixture combinations, complete oldest/newest page and Follow walks,
late-head publication, invalid JSON/time/limit input and eight corruption
and index mutations. The guard proof temporarily disables head CRC
validation, requires its negative test to fail, restores the exact source
bytes, and reruns the test. It must run before the final build.

Collection balances Go/Rust order across six passes, starts one process per
case, pins timed work to one CPU, and uses copies/stores/build outputs on the
Linux volume. It records owning outputs and current/peak retained-result RSS,
21 paired kernels, read/filter/page/follow paths on typed and public-text
fixtures, append and differently encoded seal paths, allocation counters,
native SQLite counters, file/payload/dbstat and exact source/binary hashes.
These are Docker Desktop/WSL2 exploratory local ratios; sequential engine
times are not bare-Linux latency or concurrent-throughput claims.
