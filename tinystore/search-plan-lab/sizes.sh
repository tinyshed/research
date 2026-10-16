#!/bin/sh
# What each piece adds to a stripped release executable, on Linux x86-64:
# probe built with one set of features at a time.
set -eu
data=/src/tinystore/reports/data/search-plan-2026-10-16
cd /src/tinystore/search-plan-lab
mkdir -p "$data"
echo "[" > "$data/binary-sizes.json"
first=1
for features in "" analyzer analyzer,typo analyzer,typo,icu analyzer,typo,tantivy; do
  cargo build --release --locked --bin probe --no-default-features ${features:+--features "$features"} 2>/dev/null
  bytes=$(stat -c %s /target/release/probe)
  [ $first = 1 ] || echo "," >> "$data/binary-sizes.json"
  printf '{"features": "%s", "bytes": %s}' "${features:-none}" "$bytes" >> "$data/binary-sizes.json"
  first=0
done
echo "]" >> "$data/binary-sizes.json"
cat "$data/binary-sizes.json"
