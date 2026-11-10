#!/bin/sh
# search-prefix-memory: the heap a TinyStore search with `prefix` or
# `prefix_every` holds above the same search without, on indexes generated
# from a seed, each step a container of rust:1.99. From the repository's root:
#
#   sh tinystore/search-prefix-bench/round.sh tools     # the binary, release, with TinyStore's SQLite flags
#   sh tinystore/search-prefix-bench/round.sh build     # each index of INDEXES, on the volume search-prefix-data
#   sh tinystore/search-prefix-bench/round.sh measure   # each index without a budget and with BUDGET's, in turn
#   sh tinystore/search-prefix-bench/round.sh summary   # the tables, from the raw lines
#
# An index is <documents>x<words a document>; the raw lines go to
# tinystore/reports/data/search-prefix-memory-2026-11-10/, or DATA's.
set -eu
export MSYS_NO_PATHCONV=1
repo=$(pwd -W 2>/dev/null || pwd)
data=${DATA:-tinystore/reports/data/search-prefix-memory-2026-11-10}
mkdir -p "$data"
mounts="-v $repo:/src -v search-msmarco-cargo:/usr/local/cargo/registry -v search-prefix-target:/target \
  -v search-prefix-data:/data -e CARGO_TARGET_DIR=/target"
bin=/target/release/search-prefix-bench
limits="--cpus 2 --memory 6g"
seed=${SEED:-20261110}
indexes=${INDEXES:-"250000x50 500000x50 1000000x50 2000000x50 4000000x50 1000000x12 1000000x25 1000000x100"}
budget=${BUDGET:-16777216}

case "${1:-}" in
tools)
  docker run --rm $mounts -w /src/tinystore/search-prefix-bench rust:1.99 \
    sh -c 'rustc --version && cargo build --release 2>&1 | tail -3'
  ;;
build)
  for at in $indexes; do
    docker run --rm $limits $mounts rust:1.99 sh -c \
      "rm -rf /data/$at && $bin build /data/$at ${at%x*} ${at#*x} $seed" >> "$data/builds.jsonl"
    tail -1 "$data/builds.jsonl"
  done
  ;;
measure)
  docker info --format '{"docker":"{{.ServerVersion}}","kernel":"{{.KernelVersion}}","cpus":{{.NCPU}},"memory":{{.MemTotal}}}' \
    >> "$data/environment.jsonl"
  turn=0
  for at in $indexes; do
    turn=$((turn + 1))
    if [ $((turn % 2)) -eq 1 ]; then order="none $budget"; else order="$budget none"; fi
    for memory in $order; do
      printf '{"step":"run","index":"%s","memory":"%s","started":"%s"}\n' "$at" "$memory" \
        "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$data/environment.jsonl"
      docker run --rm $limits $mounts rust:1.99 \
        $bin measure /data/$at $memory ${HEAP_PASSES:-3} ${TIME_PASSES:-9} >> "$data/measure.jsonl"
    done
  done
  printf '{"step":"ended","at":"%s"}\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$data/environment.jsonl"
  ;;
summary)
  python -I tinystore/search-prefix-bench/summary.py "$data" > "$data/summary.md" 2>&1 \
    || python3 -I tinystore/search-prefix-bench/summary.py "$data" > "$data/summary.md"
  cat "$data/summary.md"
  ;;
*)
  echo "round.sh tools|build|measure|summary" >&2
  exit 2
  ;;
esac
