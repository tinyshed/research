#!/bin/sh
# Every trace, five seeds, eight at a time: one JSON line a ceiling to stdout.
for seed in 1 2 3 4 5; do
  for job in "zipf 0.8" "zipf 1.0" "crawler 0.8" "crawler 1.0" "shift 0.8" "lecture 0"; do
    echo "$job $seed"
  done
done | xargs -P 8 -L 1 sh -c 'bun sim.ts "$0" "$1" "$2"'
