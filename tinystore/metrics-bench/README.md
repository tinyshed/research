# Complete synchronous metrics paths

Isolated prototype comparing the real public Go metrics API with a Rust port
using native SQLite 3.53.4. Production code is unchanged. The
[report](../reports/metrics-native-2026-10-08.md) includes scope, results and
implementation differences; raw data is under
`../reports/data/metrics-native-2026-10-08`.

Prerequisites: Linux amd64, Go 1.27.1, Rust 1.99.0, Python 3, GNU time,
`taskset`, `lscpu`, a C compiler and `ar`, network access for pinned dependencies.
Matplotlib is used only to regenerate the standalone figure.

The Go module imports TinyStore through `../../source`. Initialize the
repository submodule and select the measured commit before reproducing:

```sh
cd "<repo>"
git submodule update --init tinystore/source
git -C tinystore/source checkout e307c48a40126aad0e2873b6bf3aaedef8115483
cd tinystore/metrics-bench
```

Keep `../sqlite-bench/build_native.py` beside this prototype; it pins and
builds the same custom SQLite archive used by the earlier SQL comparison.
The source archive includes that builder. `run.py` builds it if its static
library is absent and points Cargo at its exact archive/header directory.

Put the required Go/Rust/GNU-time executables on PATH. Set CARGO_HOME and
RUSTUP_HOME only if your Rust installation uses custom locations. From
`<repo>/tinystore/metrics-bench`, reproduce with:

```sh
GOWORK=off CGO_ENABLED=0 python3 run.py
python3 plot_results.py
python3 render_report.py
```

`--skip-build` reuses binaries; `--verify-only` checks results, plans, guards
and Go cross-reader compatibility without timing. Default measurements are
five paired passes over 32 cases. Each process copies a closed immutable
fixture into a private directory with the filename **metrics.db**, which the
public Go engine owns. Other basenames are rejected to prevent reading a
different file by mistake.

The main workload includes six ingest traces, sealing/full expiry, heads,
sealed/head boundaries, 8/16-series reads, exact label/Where matching,
Stream, all eight aggregate operations, cut-block and grouped aggregates.
Seventeen additional edge traces verify floating-point bits and exact sums.
Go-created fixtures are generated locally and never included in the archive.

Pilots choose a fixed number of timed operations. Both stacks then execute
the same 32 warm and fixed timed indices on independent fresh copies; no
divergent write calibration state survives. Maintain/expiry are first-call
one-shot scenarios. After every write timing, production Go reopens both
files and compares logical samples; closed-file pages and payload sizes are
recorded separately. Memory is measured in separate processes, retaining 16
Read/aggregate outputs and consuming Stream results one series at a time.

Validation commands (after fixtures/native archive are built):

```sh
GOWORK=off CGO_ENABLED=0 go -C go vet ./...
cargo fmt --manifest-path rust/Cargo.toml --check
SQLITE3_STATIC=1 \
SQLITE3_LIB_DIR="$PWD/../sqlite-bench/generated/lib" \
SQLITE3_INCLUDE_DIR="$PWD/../sqlite-bench/generated/sqlite-amalgamation-3530400" \
cargo test --manifest-path rust/Cargo.toml --release --locked
```

The Rust code retains Go-compatible formats and exact arithmetic, but uses
native zstd/Huffman candidates and different ownership in some codecs. It is
a synchronous prototype: concurrent pools/admission, shared reservations,
caller cancellation, instruments and runtime lifecycle are outside its scope.
The native Huffman prototype uses private symbols from the pinned zstd build.
SQLite/zstd are statically embedded; system C/runtime libraries remain dynamic.
Sources and executables must stay unchanged while measuring. No production
edits, commits or pushes are part of this experiment.
