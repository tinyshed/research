#!/bin/sh
# the static musl binary of the tree mounted at /src, its size, and whether it links a library at run time
set -eu
target=x86_64-unknown-linux-musl
rustup target add "$target" >/dev/null 2>&1
cargo build --release --target "$target" -p dashbin-cli 2>&1 | grep -E "^(error|warning: unused)" -A 6 || true
binary="$CARGO_TARGET_DIR/$target/release/dashbin"
needed=$(readelf -d "$binary" | grep NEEDED || true)
echo "bytes=$(wc -c <"$binary") needed=[${needed}]"
