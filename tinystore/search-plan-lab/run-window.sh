#!/bin/sh
# One block format, two executors: block-max MaxScore a candidate at a time
# against a window of documents at a time, on blocks built again here, each
# checked against scoring every posting, then read alternating, A B B A.
set -eu
lab=/target/release/lab
corpus=/src/tinystore/bench/corpus/nomiracl
out=/perf/outw
data=/src/tinystore/reports/data/search-blocks-2026-10-16
cd /src/tinystore/search-plan-lab
cargo build --release --locked --bin lab
mkdir -p "$out"
rm -rf "$out"/*
: > "$data/window-builds.jsonl"
: > "$data/window-checks.jsonl"
: > "$data/window-queries.jsonl"
"$lab" build "$corpus" "$out" mixed-1m auto blocks >> "$data/window-builds.jsonl"
for executor in cursor window; do
  LAB_EXECUTOR=$executor "$lab" check "$out" mixed-1m auto or,pruned-or 0.05 >> "$data/window-checks.jsonl"
done
for order in "1 cursor window" "2 window cursor"; do
  set -- $order
  round=$1
  shift
  for executor in "$@"; do
    LAB_ROUND=$round LAB_EXECUTOR=$executor "$lab" query "$out" mixed-1m auto blocks or,pruned-or 1 0.05 clean exact >> "$data/window-queries.jsonl"
    LAB_ROUND=$round LAB_EXECUTOR=$executor "$lab" query "$out" mixed-1m auto blocks pruned-or 1 0.02 clean exact >> "$data/window-queries.jsonl"
  done
done
mkdir -p "$data/raw"
for file in "$out"/raw/*.jsonl; do
  gzip -c "$file" > "$data/raw/window-$(basename "$file").gz"
done
