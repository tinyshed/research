#!/bin/sh
# The reads of run-packed.sh again, on the indexes it and run-window.sh left:
# one executor, the blocks with a directory row a block and keys as text
# against the same blocks packed, alternating, A B B A.
set -eu
lab=/target/release/lab
rows=/perf/outw
packed=/perf/outp
data=/src/tinystore/reports/data/search-blocks-2026-10-16
cd /src/tinystore/search-plan-lab
cargo build --release --locked --bin lab
: > "$data/packed-queries.jsonl"
read_one() {
  for cut in 0.05 0.02; do
    plans=pruned-or
    [ "$cut" = 0.05 ] && plans=or,pruned-or
    LAB_ROUND=$1 LAB_EXECUTOR=window "$lab" query "$2" mixed-1m auto blocks "$plans" 1 "$cut" clean exact > /perf/lines.jsonl
    sed "s|\"executor\":\"window\"|\"executor\":\"window\",\"format\":\"$3\"|" /perf/lines.jsonl >> "$data/packed-queries.jsonl"
  done
}
read_one 1 "$rows" rows
read_one 1 "$packed" packed
read_one 2 "$packed" packed
read_one 2 "$rows" rows
