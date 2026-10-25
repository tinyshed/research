#!/usr/bin/env bash
# runs inside rssprobe-musl: the four size binaries, static musl, fat LTO, stripped, each measured against the frames alone
set -euo pipefail
T=x86_64-unknown-linux-musl
cargo build -q --release --target $T --bin size-base --bin size-own
cargo build -q --release --target $T --features json --bin size-json
cargo build -q --release --target $T --features arrow --bin size-arrow
base=$(stat -c %s /target/$T/release/size-base)
for b in base json arrow own; do
  s=$(stat -c %s /target/$T/release/size-$b)
  printf '%-6s %9d bytes %+9d over base\n' $b $s $((s - base))
done
