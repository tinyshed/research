#!/bin/sh
# both builds static for musl, as dashbin ships; prints each size and whether SQLite's text is inside
set -eu
rustup target add x86_64-unknown-linux-musl >/dev/null 2>&1
for features in "" "config"; do
    cargo build -q --release --locked --target x86_64-unknown-linux-musl --no-default-features ${features:+--features $features}
    binary=$CARGO_TARGET_DIR/x86_64-unknown-linux-musl/release/config-weight
    bytes=$(wc -c <"$binary")
    sqlite=$(grep -c "SQLite format 3" "$binary" || true)
    echo "features [${features:-typos}]: $bytes bytes, SQLite inside: $sqlite, runs: $($binary | tr '\n' ' ')"
done
