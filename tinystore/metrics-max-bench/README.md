# TinyStore Rust metrics: deeper optimizations

This research prototype compares production Go, the preceding optimized Rust
port, independently switched deeper optimizations, Rayon, LTO, host CPU
code generation and profile-guided optimization on identical closed fixtures.
It does not change production TinyStore.

`--tuning` is a bit mask: 1 = word-based residual/Huffman kernels,
2 = shared head buffers and bounded decode scratch, 4 = fused decoding and
query folding, 8 = operation-specialized aggregate kernels. Zero preserves the
control algorithms. The same source produces compiler variants; telemetry is
built separately and is never used for latency comparisons.

Prerequisites: Linux amd64, Go 1.27.1, Rust 1.99.0 with `llvm-tools`, Python 3,
a C compiler and ar, GNU time, taskset and lscpu. Put these tools on PATH;
inherit your normal CARGO_HOME/RUSTUP_HOME, if customized. This run used a
2-CPU cgroup quota on a 3-vCPU VM; it cannot establish wider scaling.

From a clone of this repository:

```sh
cd <repo>
git submodule update --init tinystore/source
python3 tinystore/sqlite-bench/build_native.py
cd tinystore/metrics-max-bench
export SQLITE3_STATIC=1
export SQLITE3_LIB_DIR="$PWD/../sqlite-bench/generated/lib"
export SQLITE3_INCLUDE_DIR="$PWD/../sqlite-bench/generated/sqlite-amalgamation-3530400"
export GOWORK=off CGO_ENABLED=0 GOMAXPROCS=1
rustup component add llvm-tools
mkdir -p bin stores
go -C go build -trimpath -o ../bin/go-reference .
cargo build --release --locked --manifest-path reference-rust/Cargo.toml
cp reference-rust/target/release/tinystore-metrics-native-bench bin/rust-reference
for kind in sealed ready head scrape empty edge wide; do
  mkdir -p "stores/$kind"
  bin/go-reference --db "$PWD/stores/$kind/metrics.db" --mode fixture --fixture "$kind"
done
python3 build.py
cargo test --release --locked --features telemetry --manifest-path rust/Cargo.toml
python3 deep_run.py --verify-only
git -C ../.. rev-parse HEAD > measured-commit.txt
python3 deep_run.py --skip-verify
```

Commit the harness before measuring. Keep the machine quiet during the last
command. Raw output is in `../reports/data/metrics-deep-2026-10-08`; the dated
report will link the measured harness commit and complete environment.

The harness verifies 56 traces across 13 configurations, including forced
worker execution, query plans, guards and Go cross-reading closed writes.
It measures 42 workloads in five interleaved passes, with four independent
switch ablations on nine selected workloads, 90 separate peak-RSS processes and
20 scalar/word microbenchmark processes. Fixed operation counts come from a
120-ms pilot of the preceding Rust implementation, subject to documented caps.
Profiles report inclusive instrumented time and Rust allocation requests,
not native SQLite/zstd allocations, total live heap or additive CPU samples.

PGO training holds out wide64, full16, IEEE edge traces, prefix/NoneOf and
grouped queries. Host-specific code generation is a separate variant, not the
portable default. SQLite 3.53.4 and zstd are statically linked; system C/runtime
libraries remain dynamic. No dynamic libsqlite3 is required.

The synchronous port omits production reader pools, admission/shared memory
reservations, caller cancellation, instrumentation, directory locks and
background jobs. These measurements concern cache-resident single-request
latency, not concurrent server throughput or a production migration.
