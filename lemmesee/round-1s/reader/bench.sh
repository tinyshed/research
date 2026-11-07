#!/bin/sh
# Round 1s: the reader timed in the Linux container, as round-1s/README.md §4
# says. Run from the repository's root on the host:
#
#   MSYS_NO_PATHCONV=1 docker run --rm --cpuset-cpus 3 -v <repo>:/src \
#     -v lemmesee-cargo:/usr/local/cargo/registry -w /src/lemmesee \
#     rust:1.99.1 sh round-1s/reader/bench.sh <models> <threshold> <bench dir>
#
# <bench dir> holds external.jsonl and test.jsonl, the schemas to time; the
# results go there as bench-*.json, memory-*.json and environment.txt.
set -eu
models=$1
threshold=$2
dir=$3
target=/tmp/target
cargo build --release --locked --manifest-path round-1s/reader/Cargo.toml --target-dir $target >/dev/null 2>&1
bin=$target/release/reader
{
  echo "rustc: $(rustc --version)"
  echo "kernel: $(uname -sr)"
  echo "cpu: $(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //')"
  echo "cpus seen: $(nproc)"
  echo "binary bytes: $(stat -c %s $bin)"
  echo "binary bytes, stripped: $(strip -o /tmp/reader-stripped $bin && stat -c %s /tmp/reader-stripped)"
} > "$dir/environment.txt"
# The external databases as the specification says, three warm runs and the
# median of 21, a schema slower than a second (CTU's Bench) of 5 after one;
# the test split's 4 320 schemas of 5 after one
$bin bench "$models/keys.bin" "$threshold" "$dir/external.jsonl" "$dir/bench-external.json" 21 3 1.0 5
$bin bench "$models/keys.bin" "$threshold" "$dir/test.jsonl" "$dir/bench-test.json" 5 1 1.0 5
# The resident set, each a process of its own
$bin memory "$models/keys.bin" "$threshold" "$dir/external.jsonl" - > "$dir/memory-model.json"
for id in $(cat "$dir/memory-ids.txt"); do
  name=$(echo "$id" | tr '/' '-')
  $bin memory "$models/keys.bin" "$threshold" "$dir/external.jsonl" "$id" > "$dir/memory-$name.json"
done
