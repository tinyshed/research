#!/bin/sh
# One executor, two formats: the blocks run-window.sh built, a directory row
# a block and keys as text, against the same blocks packed, keys by number
# and a directory row a chunk; each checked, then read alternating, A B B A.
set -eu
lab=/target/release/lab
corpus=/src/tinystore/bench/corpus/nomiracl
rows=/perf/outw
packed=/perf/outp
data=/src/tinystore/reports/data/search-blocks-2026-10-16
cd /src/tinystore/search-plan-lab
cargo build --release --locked --bin lab
mkdir -p "$packed"
rm -rf "$packed"/*
: > "$data/packed-builds.jsonl"
: > "$data/packed-checks.jsonl"
: > "$data/packed-queries.jsonl"
LAB_FORMAT=packed "$lab" build "$corpus" "$packed" mixed-1m auto blocks >> "$data/packed-builds.jsonl"
for executor in cursor window; do
  LAB_EXECUTOR=$executor "$lab" check "$packed" mixed-1m auto or,pruned-or 0.05 >> "$data/packed-checks.jsonl"
done
read_one() {
  LAB_ROUND=$1 LAB_EXECUTOR=window "$lab" query "$2" mixed-1m auto blocks or,pruned-or 1 0.05 clean exact \
    | sed "s|\"executor\":\"window\"|\"executor\":\"window\",\"format\":\"$3\"|" >> "$data/packed-queries.jsonl"
  LAB_ROUND=$1 LAB_EXECUTOR=window "$lab" query "$2" mixed-1m auto blocks pruned-or 1 0.02 clean exact \
    | sed "s|\"executor\":\"window\"|\"executor\":\"window\",\"format\":\"$3\"|" >> "$data/packed-queries.jsonl"
}
read_one 1 "$rows" rows
read_one 1 "$packed" packed
read_one 2 "$packed" packed
read_one 2 "$rows" rows
du -s --block-size=1 "$packed"/*/* > "$data/packed-files.txt"
