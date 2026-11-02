#!/bin/sh
# each variant built static for musl, as dashbin ships, and its size
set -eu
rustup target add x86_64-unknown-linux-musl >/dev/null 2>&1
for variant in plain:"" with_utoipa:utoipa with_aide:aide with_utoipa_offline:utoipa; do
    bin=${variant%%:*}
    features=${variant#*:}
    cargo build -q --release --target x86_64-unknown-linux-musl --bin "$bin" ${features:+--features $features}
    echo "$bin $(wc -c <"$CARGO_TARGET_DIR/x86_64-unknown-linux-musl/release/$bin")"
done
