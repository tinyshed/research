#!/bin/sh
# A million documents, FTS5 against Tantivy as collections that own their
# documents: one build each, two rounds of queries in opposite orders, then
# two rounds of writes, which change the index and so come last.
set -eu
lab=/target/release/lab
corpus=/src/tinystore/bench/corpus/nomiracl
out=/perf/out1m
data=/src/tinystore/reports/data/search-1m-2026-10-16
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
: > "$data/builds.jsonl"
: > "$data/queries.jsonl"
: > "$data/writes.jsonl"

for backend in fts tantivy; do
  "$lab" build "$corpus" "$out" mixed-1m auto "$backend" >> "$data/builds.jsonl"
done
for order in "1 fts tantivy" "2 tantivy fts"; do
  set -- $order
  round=$1
  shift
  for backend in "$@"; do
    LAB_ROUND=$round "$lab" query "$out" mixed-1m auto "$backend" or,pruned-or 1 0.05 clean exact >> "$data/queries.jsonl"
    LAB_ROUND=$round "$lab" query "$out" mixed-1m auto "$backend" pruned-or 1 0.02 clean exact >> "$data/queries.jsonl"
  done
done
for order in "1 fts tantivy" "2 tantivy fts"; do
  set -- $order
  round=$1
  shift
  for backend in "$@"; do
    LAB_ROUND=$round "$lab" write "$corpus" "$out" mixed-1m auto "$backend" >> "$data/writes.jsonl"
  done
done

mkdir -p "$data/raw"
for file in "$out"/raw/*.jsonl; do
  gzip -c "$file" > "$data/raw/$(basename "$file").gz"
done
du -s --block-size=1 "$out"/*/* > "$data/index-files.txt"
