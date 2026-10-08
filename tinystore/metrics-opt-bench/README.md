# TinyStore Rust metrics: allocation optimizations and Rayon

The preserved preliminary experiment used a local, uncommitted harness to
compare production Go, the
preceding baseline Rust metrics port, optimized serial Rust, and bounded
Rayon on one/two threads. Production TinyStore is unchanged.

Publishing these artifacts does not reclassify the original run as a formal
measurement round. The [report](../reports/metrics-optimization-2026-10-08.md) includes all 39
workloads, ablations, limitations, and RSS. Raw measurements, source/binary
SHA-256, fixture hashes, SQLite settings, and environment are under
`../reports/data/metrics-optimization-2026-10-08`.

`rust-baseline-src` preserves the previous Rust algorithms byte-for-byte;
only its CLI adds the same wide workloads as the new CLI. `rust` adds a
280-byte exact sum accumulator, fewer codec/output allocations, and ordered
per-series Rayon after the SQLite snapshot closes. `--fast-exact 0|1` and
`--fast-codec 0|1` control serial ablations. `--threads 0|1|2` controls Rayon;
`--force-parallel 1` exercises worker paths even for tiny verification inputs.
One global pool is initialized before timing. At most 8 series are decoded
per parallel batch. Writer transactions are serial.

Prerequisites: Linux amd64, Go 1.27.1, Rust 1.99.0, Python 3, GNU time,
taskset/lscpu/readelf, a C compiler/ar, network for pinned dependencies.
Matplotlib is needed only for report figures. Put the required Go/Rust/GNU-time
executables on PATH. Set CARGO_HOME and RUSTUP_HOME only if your Rust
installation uses custom locations.

Initialize the repository submodule and select the measured production commit:

```sh
cd "<repo>"
git submodule update --init tinystore/source
git -C tinystore/source checkout e307c48a40126aad0e2873b6bf3aaedef8115483
cd tinystore/metrics-opt-bench
python3 ../sqlite-bench/build_native.py
export SQLITE3_STATIC=1
export SQLITE3_LIB_DIR="$PWD/../sqlite-bench/generated/lib"
export SQLITE3_INCLUDE_DIR="$PWD/../sqlite-bench/generated/sqlite-amalgamation-3530400"
export GOWORK=off CGO_ENABLED=0 GOMAXPROCS=1
mkdir -p bin stores
go -C go build -trimpath -o ../bin/go-metrics-bench .
cargo build --manifest-path rust-baseline-src/Cargo.toml --release --locked
cp rust-baseline-src/target/release/tinystore-metrics-native-bench bin/rust-baseline
cargo build --manifest-path rust/Cargo.toml --release --locked
cp rust/target/release/tinystore-metrics-native-bench bin/rust-optimized
for kind in sealed ready head scrape empty edge wide; do
  mkdir -p "stores/$kind"
  if ! test -e "stores/$kind/metrics.db"; then
    bin/go-metrics-bench --db "$PWD/stores/$kind/metrics.db" --mode fixture --fixture "$kind"
  fi
done
python3 opt_run.py
python3 render_opt_report.py
```

Default measurement: 56 correctness traces × 9 configurations (forced
parallel), plans/guards, 39 timing workloads × 5 passes plus selected
ablations, and 75 separate memory processes. Every run copies a closed
Go-created fixture into its own temporary directory. Writes execute
identical warm/timed operation indices and are cross-read by production Go.
Fixture/binary files are generated locally and excluded from the archive.
The filename must be **metrics.db**, because the public Go engine owns it.

`python3 opt_run.py --verify-only` stops after correctness validation;
`--skip-verify` is only for already verified unchanged sources/binaries.
Do not compile or run other CPU workloads during timing. The harness pins
serial variants to the first allowed CPU and picks a second CPU with a
different reported L1 cache for primary Rayon 2. It also saves a selected
ablation on the shared-cache CPU pair; this prototype assumes the original
three-vCPU VM topology for that extra variant. Report the actual VM/quota
and affinity; do not infer physical-core scaling from these virtual CPUs.

Validation:

```sh
go -C go vet ./...
cargo fmt --manifest-path rust/Cargo.toml --check
cargo test --manifest-path rust/Cargo.toml --release --locked
```

SQLite 3.53.4 is built from the pinned official amalgamation with the same
custom native flags as the previous experiment. SQLite/zstd are statically
embedded; system C/runtime libraries remain dynamic. This synchronous port
still omits production reader pools/admission/shared memory reservations,
caller cancellation/instruments/runtime directory locks and background jobs.
Native Huffman uses private symbols from the pinned zstd build. Measurements
are cache-resident single-request latency, not concurrent server throughput.
No commits/pushes or production rollout are part of this experiment.
