# Immutable metrics payload packs

Research only, using TinyStore commit
`e307c48a40126aad0e2873b6bf3aaedef8115483` and the current production
32-slot, 240-sample, version-4 exact-summary directories. No production files
or code are changed. The baseline is a fresh rebuild of the actual Go-created
fixture with its original DDL and identical table/body/directory/clock bytes.
The Go-created file is also reported separately. Registry, postings, state and
mutable heads are copied unchanged into every variant.

Experimental directory envelope `0x50/1` contains the original v4 directory
plus a compact list of unique pack rowids and, for each external slot, its
pack index, byte offset and expected body CRC. The envelope CRC binds the
series and temporal bounds. Keeping the original payload-id bytes makes this
a conservative added-metadata comparison. A second experimental format0x51
replaces obsolete payload IDs inside one recompressed directory stream while
copying scalar/exact-summary tokens and inline values unchanged. Same-format
`compact-unpacked` and `compact-count1` controls isolate representation cost;
`rowid-` variants retain and charge the UNIQUE group index. All new bytes are charged to the
groups b-tree; there is no SQL mapping row per block. This prototype rejects
directories above the existing 8 KiB directory bound.

`payload_packs(id INTEGER PRIMARY KEY, refs, live_bytes, body)` is a ROWID
table. Its immutable bodies contain a 24-byte checked version/id/count/length
header and the original sealed bodies, including each original per-block
checksum. The retained expected CRC rejects address swaps; the original
decoder validates the selected body's time/first-value/clock/body checksum.
Each pack is at most 16 KiB and 32 bodies; each body still decodes at most 240
samples within the original codec bound. Byte targets are soft when one
individual body exceeds the target. Packs follow original series/group/slot
order. Crossing a group/series boundary is allowed and charged through
retention dead-space results.

Whole-BLOB reads batch pack ids through the same `json_each` join shape as
the one-body-per-row baseline. Range reads use stock safe rusqlite `Blob`:
open, check header, read selected range, reopen for another row, close. All
native handle operations are timed. Fetches happen in one snapshot; every
handle closes and the snapshot ends before decoding or exact aggregate
folding. Request/copied-byte and decode ceilings remain explicit. The extra
copy from a materialized pack into each selected body is separately counted.

Queries exercise sealed bodies: a point in one block, 16 sparse blocks, eight
adjacent blocks, the selected series' complete sealed range (capped at the
100,000-point decode ceiling), and whole-block exact sum/count folding.
Registry matching, mutable heads, engine admission and production concurrency
are outside these timings. They remain in whole-file density. These are
synchronous backend operation times in a shared WSL2 VM; they do not establish
production request latency.

`warm` means eight warm queries on a 1 MiB SQLite cache. `cold` clears the
SQLite page cache with `PRAGMA shrink_memory` before each timed operation,
outside its timer. It does not evict Linux's page cache. Timings contain no
db-status/VFS/strace instrumentation. Cache-hit/miss counters run in separate
diagnostic processes; strace includes startup/selection/warmup and therefore
is not labeled isolated query I/O. Page-cache misses and read syscalls are
reported separately from application byte copies. No claim of cold physical
disk I/O follows from constant range-copy bytes.

Prepare in the existing Linux measurement container, with `<repo>` mounted at
`/src` and a Linux volume at `/work`. Build SQLite first with the existing
`../sqlite-bench/build_native.py`; its pinned stock archive remains unchanged.

```sh
export CARGO_HOME=/work/cargo RUSTUP_HOME=/work/rustup
export PATH=/work/cargo/bin:$PATH CARGO_TARGET_DIR=/work/metrics-payload-target
export SQLITE3_LIB_DIR=/src/tinystore/sqlite-bench/generated/lib
export SQLITE3_INCLUDE_DIR=/src/tinystore/sqlite-bench/generated/sqlite-amalgamation-3530400
export SQLITE3_STATIC=1
cargo test -- --skip decode_go_production_fixture_stores
cargo build --release --locked
python3 run.py density --datasets tsbs,alibaba,regular,irregular,nonsparse,edge
python3 run.py density --datasets tsbs,alibaba,regular,irregular,nonsparse,edge --variants compact-unpacked,compact-count1,compact-count8,compact-count32,rowid-baseline,rowid-compact-unpacked,rowid-compact-count8,rowid-compact-count32
python3 run.py verify --datasets tsbs,alibaba --variants baseline,compact-count8,rowid-compact-unpacked,rowid-compact-count32
python3 run.py pilot --datasets tsbs,alibaba --variants baseline,compact-count8,rowid-compact-unpacked,rowid-compact-count32
# Inspect plan.json/projected run length before the six retained passes.
sh finish_study.sh
python3 render_report.py
```

The recorded0x50 runtime source is archived under
`../reports/data/metrics-payload-2026-10-09/format50-source`; its source hashes
are checked against the original environment. Restore those files at this
crate's path in an isolated checkout to reproduce that exact executable. The
original orchestration hash is retained; later runner revisions are not
asserted byte-identical. The final checked-in runner also supports the original
count/byte-target recipes. No old executable is committed.

The common production-Go fixture generator is in
`../metrics-layout-bench/go/`. Its manifest hashes sorted LE timestamp/value
bits returned by the real Go API independently of the reused Rust decoder.
`verify` checks those hashes, every sample's bits, exact summary bytes and all
unchanged tables. The inherited fixture test requiring five historical
metrics-native fixture folders is excluded; the newly exported real Go files
are verified explicitly instead. Tests include malformed metadata, offsets,
lengths, header identity, expected-CRC address swaps, selected-body corruption,
immutable-address directory publication, and a WAL reader retaining its
snapshot while a writer deletes the pack. Owned copies decode after that
snapshot ends.

The lifecycle trace expires complete prefix blocks at 25/50/75/100% of the
sealed time span, clips overlapping blocks in reads, updates pack references
and live bytes atomically, and deletes a pack only with its last live reference.
It reports retained dead bodies separately from file freelist pages and WAL
bytes. It never repacks or VACUUMs after expiry. Maintenance timings are
single-trace diagnostics unless repeated separately; they are not throughput
claims. Power-failure recovery, concurrent maintenance/publication, production
pooling/cancellation and real cold-disk performance remain production gates.
