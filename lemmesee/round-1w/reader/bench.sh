#!/bin/sh
# Round 1w: round 1s's reader with `1s`, round 1t's with `nine-mended` and
# this round's with `eleven--wide`, timed in the Linux container in one run
# on the same schemas; round 1t's bench.sh with a third reader. Run from the
# repository's root on the host:
#
#   MSYS_NO_PATHCONV=1 docker run --rm --cpuset-cpus 3 -v <repo>:/src \
#     -v lemmesee-cargo:/usr/local/cargo/registry -w /src/lemmesee \
#     rust:1.99.1 sh round-1w/reader/bench.sh <1s models> <1s threshold> \
#     <1t models> <1t threshold> <1w models> <1w threshold> <bench dir>
#
# Each <models> holds its keys.bin. <bench dir> holds external.jsonl (round
# 1p's databases, as round 1s timed them) and memory-ids.txt; the results go
# there as bench-*.json, memory-*.json and environment.txt.
set -eu
dir=$7
for r in 1s 1t 1w; do
  cargo build --release --locked --manifest-path round-$r/reader/Cargo.toml --target-dir /tmp/$r >/dev/null 2>&1
done
{
  echo "rustc: $(rustc --version)"
  echo "kernel: $(uname -sr)"
  echo "cpu: $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //')"
  echo "cpus seen: $(nproc)"
  for r in 1s 1t 1w; do
    echo "binary bytes, round $r: $(stat -c %s /tmp/$r/release/reader)"
  done
  echo "started: $(date -u +%FT%TZ)"
} > "$dir/environment.txt"
# Round 1s's settings: three warm runs and the median of 21, a schema slower
# than a second (CTU's Bench) of 5 after one; the three readers take turns
for k in 1 2; do
  /tmp/1s/release/reader bench "$1/keys.bin" "$2" "$dir/external.jsonl" "$dir/bench-1s-$k.json" 21 3 1.0 5
  /tmp/1t/release/reader bench "$3/keys.bin" "$4" "$dir/external.jsonl" "$dir/bench-1t-$k.json" 21 3 1.0 5
  /tmp/1w/release/reader bench "$5/keys.bin" "$6" "$dir/external.jsonl" "$dir/bench-1w-$k.json" 21 3 1.0 5
done
memory() {
  /tmp/$1/release/reader memory "$2/keys.bin" "$3" "$dir/external.jsonl" - > "$dir/memory-$1-model.json"
  for id in $(cat "$dir/memory-ids.txt"); do
    name=$(echo "$id" | tr '/' '-')
    /tmp/$1/release/reader memory "$2/keys.bin" "$3" "$dir/external.jsonl" "$id" > "$dir/memory-$1-$name.json"
  done
}
memory 1s "$1" "$2"
memory 1t "$3" "$4"
memory 1w "$5" "$6"
echo "ended: $(date -u +%FT%TZ)" >> "$dir/environment.txt"
