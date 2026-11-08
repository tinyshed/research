#!/bin/sh
# Round 1t: round 1t's reader with version 5's cues against round 1s's, timed
# in the Linux container in one run on the same schemas. Run from the
# repository's root on the host:
#
#   MSYS_NO_PATHCONV=1 docker run --rm --cpuset-cpus 3 -v <repo>:/src \
#     -v lemmesee-cargo:/usr/local/cargo/registry -w /src/lemmesee \
#     rust:1.99.1 sh round-1t/reader/bench.sh <1s models> <1s threshold> \
#     <1t models> <1t threshold> <bench dir>
#
# <bench dir> holds external.jsonl (round 1p's databases, as round 1s timed
# them) and memory-ids.txt; the results go there as bench-*.json,
# memory-*.json and environment.txt.
set -eu
old=$1
old_t=$2
new=$3
new_t=$4
dir=$5
cargo build --release --locked --manifest-path round-1s/reader/Cargo.toml --target-dir /tmp/old >/dev/null 2>&1
cargo build --release --locked --manifest-path round-1t/reader/Cargo.toml --target-dir /tmp/new >/dev/null 2>&1
{
  echo "rustc: $(rustc --version)"
  echo "kernel: $(uname -sr)"
  echo "cpu: $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //')"
  echo "cpus seen: $(nproc)"
  echo "binary bytes, round 1s: $(stat -c %s /tmp/old/release/reader)"
  echo "binary bytes, round 1t: $(stat -c %s /tmp/new/release/reader)"
  echo "started: $(date -u +%FT%TZ)"
} > "$dir/environment.txt"
# Round 1s's settings: three warm runs and the median of 21, a schema slower
# than a second (CTU's Bench) of 5 after one; the two readers alternate
for k in 1 2; do
  /tmp/old/release/reader bench "$old/keys.bin" "$old_t" "$dir/external.jsonl" "$dir/bench-1s-$k.json" 21 3 1.0 5
  /tmp/new/release/reader bench "$new/keys.bin" "$new_t" "$dir/external.jsonl" "$dir/bench-1t-$k.json" 21 3 1.0 5
done
for which in old new; do
  if [ $which = old ]; then bin=/tmp/old/release/reader; m=$old; t=$old_t; tag=1s; else bin=/tmp/new/release/reader; m=$new; t=$new_t; tag=1t; fi
  $bin memory "$m/keys.bin" "$t" "$dir/external.jsonl" - > "$dir/memory-$tag-model.json"
  for id in $(cat "$dir/memory-ids.txt"); do
    name=$(echo "$id" | tr '/' '-')
    $bin memory "$m/keys.bin" "$t" "$dir/external.jsonl" "$id" > "$dir/memory-$tag-$name.json"
  done
done
echo "ended: $(date -u +%FT%TZ)" >> "$dir/environment.txt"
