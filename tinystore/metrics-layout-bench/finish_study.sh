#!/bin/sh
set -eu
export PATH=/work/cargo/bin:$PATH
export CARGO_TARGET_DIR=/work/metrics-layout-target
export SQLITE3_STATIC=1
export SQLITE3_LIB_DIR=/src/tinystore/sqlite-bench/generated/lib
export SQLITE3_INCLUDE_DIR=/src/tinystore/sqlite-bench/generated/sqlite-amalgamation-3530400
export GOWORK=off CGO_ENABLED=0 GOMAXPROCS=1
study_root=/src/tinystore/metrics-layout-bench
cargo build --release --locked --manifest-path "$study_root/rust/Cargo.toml"
cp /work/metrics-layout-target/release/tinystore-metrics-layout-bench /work/metrics-layout/layout
variants='exact rowid native_control rowid_native_control cap16 rowid_inline32 rowid_inline64'
datasets='tsbs alibaba irregular'
python3 "$study_root/run.py" provenance
python3 "$study_root/run.py" pilot --datasets $datasets --variants $variants
python3 "$study_root/run.py" timing --datasets $datasets --variants $variants
python3 "$study_root/run.py" publication --datasets $datasets --variants $variants
python3 "$study_root/run.py" retention --datasets $datasets --variants $variants
python3 "$study_root/run.py" rss --datasets $datasets --variants $variants
python3 "$study_root/run.py" crossread --datasets $datasets --variants exact rowid cap16 page1024 native_control
python3 "$study_root/render_report.py"
