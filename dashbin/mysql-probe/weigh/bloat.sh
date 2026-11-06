#!/bin/sh
# which crates make the static binary of the tree mounted at /src, its symbols kept for cargo-bloat to read
set -eu
target=x86_64-unknown-linux-musl
rustup target add "$target" >/dev/null 2>&1
command -v cargo-bloat >/dev/null || cargo install -q --locked cargo-bloat --version 0.12.1 --root /target/tools
export PATH="/target/tools/bin:$PATH"
CARGO_PROFILE_RELEASE_STRIP=false cargo bloat --release --target "$target" -p dashbin-cli --crates -n 45 2>&1 | tail -50
