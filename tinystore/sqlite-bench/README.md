# TinyStore SQLite backend prototype

This isolated research harness compares the current TinyStore SQL layer,
direct Go SQLite connections, and rusqlite with statically embedded native
SQLite. Production TinyStore is unchanged. See the preserved
[report](../reports/sqlite-native-2026-10-07.md) for measured results and limits.

Prerequisites: Linux amd64, Go 1.27.1, Rust 1.99.0, Python 3, a C compiler,
`ar`, `taskset`, `lscpu`, and network access for pinned build dependencies.
Keep the disk location and machine otherwise idle while measuring.

The Go module uses `../../source` as its TinyStore checkout. For a fresh
copy of these artifacts, create that sibling checkout at the measured commit:

```sh
cd "<repo>"
git submodule update --init tinystore/source
git -C tinystore/source checkout e307c48a40126aad0e2873b6bf3aaedef8115483
cd tinystore/sqlite-bench
```

Put the required Go/Rust toolchains on PATH. Set CARGO_HOME and RUSTUP_HOME
only if your Rust installation uses custom locations. From
`<repo>/tinystore/sqlite-bench`, run:

```sh
GOWORK=off CGO_ENABLED=0 python3 run.py
GOWORK=off CGO_ENABLED=0 python3 write_followup.py
python3 render_report.py
```

`run.py` builds Go and a custom native SQLite 3.53.4 static archive, builds
Rust against that archive, creates two fixtures, checks connection settings,
compares all result bytes and final logical database digests, and performs
three balanced passes. `--skip-build` reuses binaries; `--verify-only` runs
configuration/result checks alone. Default final timing intervals are at
least 0.15 seconds. `write_followup.py` checks the original source/binary
hashes before six longer single-row FULL commit passes (0.25 seconds).

Both runners copy the closed fixture into a separate temporary directory for
every child process. Measured code owns every returned BLOB, plus row
containers for range queries. Fixture copy, connection setup and full digest
hashing are outside the timer. Calibration uses increasing operation indices,
so timed operation counts/key offsets/WAL state can differ between stacks.
This is a warm, single-threaded backend comparison with synthetic SQL, not a
full metrics/records migration or concurrent storage benchmark.

Generated sources, SQLite archives, executables and database copies are
ignored. Report data and measured source/binary hashes live under
`../reports/data/sqlite-native-2026-10-07`. `build_native.py` pins the official
SQLite archive SHA256 and records compiler options/source hashes. The native
archive is linked without `libsqlite3.so`; this build still uses system C
runtime libraries. Further Rust implementation details are in
[rust/README.md](rust/README.md).
