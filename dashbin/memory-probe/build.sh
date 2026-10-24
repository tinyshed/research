#!/usr/bin/env bash
# runs inside rssprobe-musl: one load client, then hub, qjs and agent for each allocator
set -euo pipefail
export LIBSQLITE3_FLAGS="SQLITE_DQS=0 -USQLITE_ENABLE_MEMORY_MANAGEMENT SQLITE_ENABLE_DBPAGE_VTAB -DHAVE_FDATASYNC=1"
export CC_x86_64_unknown_linux_musl=musl-gcc
mkdir -p /target/out/tools
cargo build -q --release --features load --bin load
cp /target/release/load /target/out/tools/

build() {
  local variant=$1 target=$2 extra=$3
  mkdir -p "/target/out/$variant"
  for bin in hub qjs agent; do
    echo "building $variant/$bin"
    cargo build -q --release --target "$target" --features "$bin${extra:+,$extra}" --bin "$bin"
    cp "/target/$target/release/$bin" "/target/out/$variant/"
  done
}
build glibc x86_64-unknown-linux-gnu ""
build musl x86_64-unknown-linux-musl ""
build mimalloc x86_64-unknown-linux-musl mimalloc
build jemalloc x86_64-unknown-linux-musl jemalloc

for v in glibc musl mimalloc jemalloc; do
  for bin in hub qjs agent; do
    f="/target/out/$v/$bin"
    printf '%-9s %-6s %6.2f MiB  %s\n' "$v" "$bin" "$(stat -c %s "$f" | awk '{print $1/1048576}')" "$(file -b "$f" | cut -d, -f1-2 | sed 's/ELF 64-bit LSB //')"
  done
done
