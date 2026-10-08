#!/bin/sh
set -eu
export PATH=/work/cargo/bin:$PATH
export SQLITE3_STATIC=1
export SQLITE3_LIB_DIR=/src/tinystore/sqlite-bench/generated/lib
export SQLITE3_INCLUDE_DIR=/src/tinystore/sqlite-bench/generated/sqlite-amalgamation-3530400
export CARGO_TARGET_DIR=/work/target
mkdir -p /work/bin /work/stores
cargo build --release --locked --manifest-path /work/baseline/Cargo.toml
cp /work/target/release/tinystore-metrics-native-bench /work/bin/rust-baseline
cargo fmt --manifest-path /src/tinystore/metrics-max-bench/rust/Cargo.toml
cargo build --release --locked --manifest-path /src/tinystore/metrics-max-bench/rust/Cargo.toml
cp /work/target/release/tinystore-metrics-native-bench /work/bin/rust-adapter
cargo build --release --locked --features telemetry --manifest-path /src/tinystore/metrics-max-bench/rust/Cargo.toml
cp /work/target/release/tinystore-metrics-native-bench /work/bin/rust-adapter-telemetry
for kind in sealed ready head scrape empty edge wide; do
    mkdir -p /work/stores/$kind
    if [ ! -f /work/stores/$kind/metrics.db ]; then
        /work/bin/go-reference --db /work/stores/$kind/metrics.db --mode fixture --fixture $kind
    fi
done
TINYSTORE_CODEC_FIXTURES=/work/stores cargo test --release --locked --features telemetry --manifest-path /src/tinystore/metrics-max-bench/rust/Cargo.toml
