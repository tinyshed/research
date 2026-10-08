# Rust SQLite linking and configuration smoke

This independent executable checks that `rusqlite = 0.40.1`, with only
`bundled`, `cache` and `limits` enabled, can embed SQLite and express the
connection settings and selected transaction semantics used by TinyStore.
Cargo.lock pins the dependency graph, including libsqlite3-sys 0.38.2.
It is a smoke proof, not an engine migration or a performance benchmark.

The configuration follows TinyStore commit
`e307c48a40126aad0e2873b6bf3aaedef8115483`, especially
[internal/sqlite/file.go](https://github.com/tinyshed/tinystore/blob/e307c48a40126aad0e2873b6bf3aaedef8115483/internal/sqlite/file.go)
and
[internal/sqlite/driver.go](https://github.com/tinyshed/tinystore/blob/e307c48a40126aad0e2873b6bf3aaedef8115483/internal/sqlite/driver.go).
Their SHA256 values are respectively
`7a3e74bedee784d2f831b49185045cb519ce64b463627fd356056b7e8e3534b2` and
`9f4117dd4bd955603d606f4bd4aa9c22767bac50a00e45cf24134c1514899701`.

The smoke opens exactly one writer and one reader on a new temporary database.
The reader uses `SQLITE_OPEN_READ_WRITE` without creation, plus `query_only=1`.
Both connections apply foreign_keys=1, busy_timeout=5000, synchronous=FULL,
fullfsync=1, checkpoint_fullfsync=1, cache_size=-1024, and a 1 MiB
SQLITE_LIMIT_LENGTH. Each statement cache is explicitly set to 32.
It sets the new file's page_size before enabling WAL; `--page-size 4096`
covers the metrics setting and `--page-size 1024` covers records.

The executable checks pragma and sqlite3_db_config readbacks, then checks:

- BEGIN IMMEDIATE writes while a deferred reader keeps its old WAL snapshot;
  a fresh snapshot sees the committed value.
- A query_only reader rejects an UPDATE with SQLITE_READONLY.
- Two savepoint scopes preserve a successful insert while rolling back the
  second scope's update and insert.
- INTEGER, FLOAT, TEXT, BLOB and NULL keep their native value variants;
  timestamp-shaped text is still text, and retained blob bytes are owned.
- Both connection length limits reject a blob above 1 MiB.
- `LIMIT CAST(? AS INTEGER)` reuses a prepared program across four bindings,
  reports zero automatic reprepares, and returns the same program from cache.

Reproduce from `<repo>/tinystore/sqlite-rust-spike` with Rust, a C compiler,
and Linux binutils available. This uncommitted historical smoke is retained as
linkage/configuration evidence; the version-matched benchmark is documented in
[the SQLite report](../reports/sqlite-native-2026-10-07.md):

```sh
cargo fmt --check
cargo build --locked --release
mkdir -p evidence
target/release/tinystore-sqlite-rust-smoke --page-size 4096 > evidence/smoke-4096.txt
target/release/tinystore-sqlite-rust-smoke --page-size 1024 > evidence/smoke-1024.txt
ldd target/release/tinystore-sqlite-rust-smoke > evidence/ldd.txt
readelf -d target/release/tinystore-sqlite-rust-smoke > evidence/readelf-dynamic.txt
```

The executable prints sqlite_version, sqlite_source_id, compile_options, and
per-connection readbacks. The evidence directory records the actual run and
linkage output. A zero exit status means every assertion passed. Scratch
database, WAL and shared-memory files are removed after both connections close.

The selected bundled library reports SQLite 3.53.2. TinyStore's current
generated wasm2go SQLite is 3.53.4; version and compile-option parity are not
established. Native SQLite uses its Unix VFS, while TinyStore's translated
build uses SQLITE_OS_OTHER and a custom VFS. The smoke does not establish
equivalent filesystem, locking, failure or durability behavior across those
backends or platforms. Linux reads back fullfsync flags but does not exercise
macOS F_FULLFSYNC.

The native build deliberately retains THREADSAFE=1. TinyStore's translated
build's THREADSAFE=0 applies to isolated translated instances and must not be
copied blindly into a native library whose globals multiple connections share.
Each smoke connection is individually owned and runs synchronously with
SQLITE_OPEN_NO_MUTEX. DQS_DDL, DQS_DML and trusted_schema are disabled explicitly
per connection to reproduce those Go build defaults; the bundled library's
compile-time defaults are otherwise retained and printed.

Bundled SQLite means deployment does not require libsqlite3.so. It does not
mean the entire executable is static: this Linux build retains system runtime
dependencies, documented by ldd and readelf. Building it requires a C toolchain.
No reader pool, cancellation, writer admission, grouped-write scheduler,
connection recovery, custom VFS, engine schema, migrations, retained production
data or full TinyStore test suite is ported here. Native SQLite allocations
also bypass Rust allocator counters; future memory comparisons need SQLite's
own memory/status measurements as well as process RSS.
