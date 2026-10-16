#!/bin/sh
# Blocks of our own over SQLite's pages against FTS5 and Tantivy, a million
# documents each engine owns, built again here so that all three are read in
# the same state: a check of every answer against scoring every posting,
# two rounds of queries in opposite orders, two rounds of sets, then one
# round of queries over what the sets left.
set -eu
lab=/target/release/lab
corpus=/src/tinystore/bench/corpus/nomiracl
out=/perf/outb
data=/src/tinystore/reports/data/search-blocks-2026-10-16
cd /src/tinystore/search-plan-lab
cargo build --release --locked --bin lab
(cd "$corpus" && sha256sum -c --quiet /src/tinystore/search-plan-lab/corpus.sha256)
(cd "$corpus/../miracl" && sha256sum -c --quiet /src/tinystore/search-plan-lab/miracl.sha256)
mkdir -p "$data" "$out"
rm -rf "$out"/*
{
  echo "{\"uname\": \"$(uname -sr)\", \"nproc\": $(nproc), \"cpu\": \"$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //')\","
  echo " \"memory_kib\": $(grep MemTotal /proc/meminfo | tr -s ' ' | cut -d' ' -f2), \"rustc\": \"$(rustc --version)\","
  echo " \"cgroup_cpu\": \"$(cat /sys/fs/cgroup/cpu.max 2>/dev/null || echo none)\", \"cgroup_memory\": \"$(cat /sys/fs/cgroup/memory.max 2>/dev/null || echo none)\"}"
} > "$data/environment.json"
for file in builds checks queries writes; do : > "$data/$file.jsonl"; done

for backend in blocks fts tantivy; do
  "$lab" build "$corpus" "$out" mixed-1m auto "$backend" >> "$data/builds.jsonl"
done
"$lab" check "$out" mixed-1m auto or,pruned-or 0.05 >> "$data/checks.jsonl"
"$lab" check "$out" mixed-1m auto pruned-or 0.02 >> "$data/checks.jsonl"

read_all() {
  round=$1
  shift
  for backend in "$@"; do
    LAB_ROUND=$round "$lab" query "$out" mixed-1m auto "$backend" or,pruned-or 1 0.05 clean exact >> "$data/queries.jsonl"
    LAB_ROUND=$round "$lab" query "$out" mixed-1m auto "$backend" pruned-or 1 0.02 clean exact >> "$data/queries.jsonl"
  done
}
read_all 1 blocks fts tantivy
read_all 2 tantivy fts blocks
for order in "1 blocks fts tantivy" "2 tantivy fts blocks"; do
  set -- $order
  round=$1
  shift
  for backend in "$@"; do
    LAB_ROUND=$round "$lab" write "$corpus" "$out" mixed-1m auto "$backend" >> "$data/writes.jsonl"
  done
done
read_all after blocks fts tantivy
"$lab" check "$out" mixed-1m auto or,pruned-or 0.05 | sed 's/"step":"check"/"step":"check","after":true/' >> "$data/checks.jsonl"

mkdir -p "$data/raw"
for file in "$out"/raw/*.jsonl; do
  gzip -c "$file" > "$data/raw/$(basename "$file").gz"
done
du -s --block-size=1 "$out"/*/* > "$data/index-files.txt"
