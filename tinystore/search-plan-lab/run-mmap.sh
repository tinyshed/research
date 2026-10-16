#!/bin/sh
# The packed blocks read through SQLite's read calls against a 4 GiB map,
# the window executor both times, alternating, A B B A.
set -eu
lab=/target/release/lab
packed=/perf/outp
data=/src/tinystore/reports/data/search-blocks-2026-10-16
cd /src/tinystore/search-plan-lab
cargo build --release --locked --bin lab
: > "$data/mmap-queries.jsonl"
read_one() {
  LAB_ROUND=$1 LAB_EXECUTOR=window LAB_MMAP=$2 "$lab" query "$packed" mixed-1m auto blocks or,pruned-or 1 0.05 clean exact >> "$data/mmap-queries.jsonl"
  LAB_ROUND=$1 LAB_EXECUTOR=window LAB_MMAP=$2 "$lab" query "$packed" mixed-1m auto blocks pruned-or 1 0.02 clean exact >> "$data/mmap-queries.jsonl"
}
read_one 1 0
read_one 1 4294967296
read_one 2 4294967296
read_one 2 0
