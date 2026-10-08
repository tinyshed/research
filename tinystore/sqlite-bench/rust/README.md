# Native SQLite Rust benchmark

This synchronous rusqlite 0.40.1 runner links a static SQLite 3.53.4 library
created by `../build_native.py`. It refuses to open the supplied fixture if
the linked SQLite version differs. Cargo disables rusqlite defaults and uses
only cache, limits and modern_sqlite; modern_sqlite supplies pregenerated
bindings for the compatible SQLite 3.53 ABI. SQLite itself is not supplied by
rusqlite's bundled feature.

From the parent sqlite-bench directory, with Rust, a C compiler and ar present:

```sh
python3 build_native.py --cargo
rust/target/release/tinystore-sqlite-native-bench --db fixture-copy.db --case point --mode verify --iterations 16
rust/target/release/tinystore-sqlite-native-bench --db fixture-copy.db --case range240 --mode bench --seconds 0.15
rust/target/release/tinystore-sqlite-native-bench --db fixture-copy.db --mode inspect
```

The build downloads the official 3.53.4 amalgamation through the configured
HTTPS proxy, checks its pinned archive SHA256, and records the source/header
hashes, compiler and exact options in generated/native-build.json. It compiles
sqlite3.c with O2 and links libsqlite3.a using SQLITE3_LIB_DIR,
SQLITE3_INCLUDE_DIR and SQLITE3_STATIC. Generated source, archives, libraries
and database copies stay outside the recorded prototype files.

The six cases are point, point_txn, range240, aggregate240, update1 and
update64. The root harness supplies an identical Go-created fixture copy
for each process, with page size 1024 or 4096. Every read returns ordinary
owned blob bytes, and a range owns its row vector and each row's body. Results
retain scalar aggregate/update values. Input update bodies own 128-byte
vectors. Verification alone serializes those results for rolling FNV1a64;
timing consumes the owned result through black_box and a constant-size sink.

All connections apply TinyStore's WAL/FULL synchronization, fullfsync flags,
five-second busy timeout, foreign key enforcement, 1 MiB page cache and length
limit, and 32-entry statement caches. Reader handles open read/write without
creation and apply query_only. Writer transactions begin IMMEDIATE; the
point_txn reader uses DEFERRED. Update64 applies 64 cached
SAVEPOINT/UPDATE/RELEASE scopes inside one transaction. BEGIN and COMMIT use
uncached execution, as TinyStore's driver does. Connection setup and compiling
the six SQL programs are outside timing.

Each timing process performs 32 warm operations, then tests iteration counts
1, 2, 4 and onward until the final interval reaches --seconds. Its operation
index increases across every warmup and calibration interval, so later rounds
continue changing the data. Verification begins at zero without executing a
write warmup and compares every result byte plus the final complete data
digest. Inspection performs 1000 point reads, then reports both connections'
settings, SQLite compile options, process RSS, SQLite memory_used and per-
connection DBSTATUS_CACHE_USED.

The compiler options mirror the Go build's planner and SQL semantics,
including DQS=0, STAT4, TRUSTED_SCHEMA=0, LIKE_DOESNT_MATCH_BLOBS and STRICT_SUBTYPE.
The native library deliberately retains THREADSAFE=1, the Unix VFS and
automatic initialization. Go's translated SQLite uses THREADSAFE=0 in isolated
instances, a custom SQLITE_OS_OTHER VFS and explicit initialization. These
are backend comparisons even when versions, inputs and SQL settings match.
The program is not a complete TinyStore migration: admission, reader pools,
cancellation, connection recovery, grouped-write scheduling and concurrent
loads are not implemented. Fixed update batches are not the production
group-commit scheduler. Native C allocations require SQLite status or process
memory measurements and do not pass through a Rust allocator counter.

The release executable embeds SQLite without libsqlite3.so. Its system C,
math, compiler-runtime and loader dependencies may remain dynamic; embedding
SQLite does not establish a fully static executable or cross-platform
portability of this measured build.
