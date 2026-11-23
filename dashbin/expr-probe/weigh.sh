#!/bin/sh
# each variant built static for musl, as dashbin ships, alone with its own feature, and its size in bytes
set -eu
target=x86_64-unknown-linux-musl
rustup target add "$target" >/dev/null 2>&1
for variant in plain: own: with_cel:cel with_cel:cel-pratt with_cel:cel-default with_evalexpr:evalexpr with_rhai:rhai; do
    bin=${variant%%:*}
    features=${variant#*:}
    cargo build -q --release --target "$target" --bin "$bin" ${features:+--features $features}
    echo "$bin ${features:--} $(wc -c <"${CARGO_TARGET_DIR:-target}/$target/release/$bin")"
done
