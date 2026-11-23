#!/bin/sh
# each program built static for musl as dashbin ships, and its size in bytes
set -eu
target=x86_64-unknown-linux-musl
rustup target add "$target" >/dev/null 2>&1
cargo build -q --release --target "$target" --bin rows
cargo build -q --release --target "$target" --bin rows_and_files --features files
for bin in rows rows_and_files; do
    echo "$bin $(wc -c <"${CARGO_TARGET_DIR:-target}/$target/release/$bin")"
done
