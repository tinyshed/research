# Records algorithms and ownership, second native round

This continues `records-native-bench` without changing any of its measured
sources or data. Go still calls the actual public TinyStore records API at
`e307c48a40126aad0e2873b6bf3aaedef8115483`. Rust keeps stock rusqlite 0.40.1
and matched external static SQLite 3.53.4, FULL WAL writes and the production
1024-byte records pages. Source translations retain Apache-2.0 attribution
in `NOTICE`/`LICENSE`.

The prior Rust implementation remains as `engine.rs`/`codec.rs` and the
`previous` variant. The old round's archived binary is also sampled in the
new session for common cases. New code is in `codec/optimized.rs`,
`optimized.rs` and the extended common driver. Go's driver adds advancing
Follow walks, explicit cold-handle measurement and ownership checks, without
changing production API implementations.

## Variants and controlled work

- `opt`: owning string/field spans into immutable backing buffers; shared
  immutable schema dictionaries; query columns decoded before matching;
  bounded heap selection; only needed Follow blocks; charged 4 MiB
  raw+decoded Follow cache.
- `prune_off`: eagerly decode all fetched columns and disable the candidate
  page-coverage early stop, retaining the new backing representation.
- `heap_off`: collect/sort all matching row references instead of keeping
  at most the page limit; all other choices remain enabled.
- `sharing_off`: detach selected results into the previous String/Vec Record
  representation before returning, rather than keeping shared owning spans.
- `cache_raw`: only compressed schema/block bytes cached, closer to Go's
  production Follow cache; directory/decompression/validation repeat.
- `cache_off`: no Follow cache; still fetch/decode only intersecting blocks.
- `strict`: eagerly validate all fetched supported columns, including those
  the query rejects. This makes the old reader's stronger unused-column
  checking visible alongside the default consumed-column policy.

Every returned row has all its fields decoded and validated **before** the
operation returns. Canonicalization is verification only. Shared outputs own
their backing; no SQLite pointer escapes. Safe Mutex-protected state makes
Row Send+Sync; tests move a retained row to a thread after source data drops.
Driver tests drop siblings, engine and both connections before inspecting
the surviving result.

The decoded cache is an explicit optimization beyond Go's raw-byte cache.
Cold per-call measurements create a fresh engine/connection pair before each
measured operation, with opening/destruction excluded on both paths. OS
filesystem cache is uncontrolled. Warm repeated `(0,0)` reads and advancing
whole-fixture walks are separate cases; every walk restarts at the same
cursor and returns the same complete record sequence.

Cache keys rely on immutable, nonrepeating unmerged block IDs and schema
holder first-block IDs. External rewriting of an already cached payload
under the same ID is outside this contract; a probe retains that behavior
explicitly and shows clearing the cache detects CRC corruption. Current
index metadata is checked even with a decoded hit. Merged holders, retention,
Drop and external mutation are not implemented. The 4 MiB cache is a
**charged capacity bound**, counting owned backing bytes, vector/string
capacities, dependent schemas and conservative map/allocation overhead;
it is not an exact allocator-RAM or process-RSS bound. Live result owners
can keep an evicted entry alive independently of the persistent cache.

## Compatibility and validation scope

The restricted existing v1 codec and valid-UTF8 scope remain. Both runtimes
read the same actual Go-head and compatible native-sealed fixtures from the
first round; no schema, compression strategy or payload is changed for the
comparison. Arbitrary Go typed/FSE/Rice/radix/dictionary/stamped sealed
columns still reject explicitly. The new decoder bounds cumulative decoded
text across a block/schema and charges before decompression. Envelope CRC,
directory/count/index checks always run on fetched bytes. Supported query
columns and all fields of returned records are validated. A malformed unused
column with a recomputed checksum can remain unconsumed, matching production's
selective-read policy; `strict` and `previous` expose the stronger policy.

This remains a synchronous research slice: runtime lifecycle/locking,
cancellation, shared admission, concurrent/grouped writes, full sealed codec,
general retention/merges and corruption recovery are excluded. It is not a
production-ready ownership/API port.

## Reproduce

Use the previous round's Linux container, SQLite/toolchain cache and public
Apache fixture. Stores and targets are on the Linux `/work` volume. Original
binaries/fixtures remain in `/work/records-native`; new targets and binaries
use `/work/records-opt-target` and `/work/records-opt`.

```sh
export PATH=/work/cargo/bin:$PATH
python3 /src/tinystore/records-opt-bench/build.py
python3 /src/tinystore/records-opt-bench/prove_guard.py
python3 /src/tinystore/records-opt-bench/build.py
python3 /src/tinystore/records-opt-bench/verify.py
# After all competing research builds/tests/timings stop:
python3 /src/tinystore/records-opt-bench/run.py --measure
```

The mutation proof disables returned-field readiness, requires its negative
test to fail, restores exact source bytes and verifies the test passes.
Rebuild after the proof. Verification retains page/Follow/cursor hashes,
archive preservation, cold CRC/index and consumed/unused-column probes,
cache/lifetime results. Six balanced permutations compare Go, previous Rust
and optimized Rust in one session, with six-pass switch ablations. Operation
counts calibrate once per fixture/case from the fastest candidate to roughly
150 ms and are identical across all variants, keeping fixture, history and
logical output identical; counts and pilots are retained. Cold calls use a
fixed eight operations. Rust allocations use a separate counting build;
native SQLite counters and inclusive retained RSS remain separate metrics.
Collection is exploratory uncommitted work under the user's prior explicit
waiver, pinned by source/binary/fixture hashes. No result is a bare-Linux
latency or concurrent-throughput promise.
