#!/bin/sh
# One FST for every script against one a script, alternating, for the
# questions with a typo: run after run.sh, on the indexes it built.
set -eu
lab=/target/release/lab
out=/perf/out
data=/src/tinystore/reports/data/search-plan-2026-10-16
cd /src/tinystore/search-plan-lab
cargo build --release --locked --bin lab
: > "$data/typos-by-script.jsonl"
for round in 1 2; do
  for cohort in ru en mixed; do
    for fst in one script; do
      LAB_ROUND=$round LAB_TYPOS=$fst "$lab" query "$out" "$cohort" auto fts or,pruned-or 1 0.05 typos fuzzy >> "$data/typos-by-script.jsonl"
    done
  done
done
mkdir -p "$data/raw"
for file in "$out"/raw/*-one-r*.jsonl "$out"/raw/*-script-r*.jsonl; do
  gzip -c "$file" > "$data/raw/$(basename "$file").gz"
done
