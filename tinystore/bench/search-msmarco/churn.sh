#!/bin/sh
# H2.3's harness, for a quiet machine: readers beside one writer on a copy of
# each engine's index, from the repository's root, after round.sh has built
# compact, fts5 and tantivy-keyed.
#
#   sh tinystore/bench/search-msmarco/churn.sh
#
# For each engine, the compact blocks, FTS5 and Tantivy, a fresh copy of its
# index, then nine cells in this order on that copy: 10, 100 and 1,000 sets a
# second, each with 1, 4 and 16 readers, SECONDS_EACH (60) seconds each after five of
# readers alone. A cell continues from the state the one before it left, the
# head and the gone included, and says so in its line. Each container has as
# many CPUs as readers and writer, at most 16, and 12 GiB.
#
# SECONDS_EACH, RATES and READERS override the cells; LABEL names the run. Each
# cell's raw latencies go to tinystore/reports/data/search-msmarco-2026-11-01/
# churn/<stamp>/, its summary to cells.jsonl beside them.
set -eu
export MSYS_NO_PATHCONV=1
repo=$(pwd -W 2>/dev/null || pwd)
stamp=${STAMP:-$(date -u +%Y%m%dT%H%M%SZ)}
seconds=${SECONDS_EACH:-60}
rates=${RATES:-10 100 1000}
readers_list=${READERS:-1 4 16}
label=${LABEL:-measurement}
out=/perf/msmarco
work=$out/churn/$stamp
data=tinystore/reports/data/search-msmarco-2026-11-01/churn/$stamp
mkdir -p "$data"
log=$data/cells.jsonl
printf '{"label":"%s","stamp":"%s","seconds":%s,"rates":"%s","readers":"%s","started":"%s"}\n' \
  "$label" "$stamp" "$seconds" "$rates" "$readers_list" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$log"
seed=20261101
for engine in compact fts5 tantivy; do
  case "$engine" in
  compact) source=$out/compact.db index=$work/compact.db ;;
  fts5) source=$out/fts5.db index=$work/fts5.db ;;
  tantivy) source=$out/tantivy-keyed index=$work/tantivy ;;
  esac
  docker run --rm -v search-msmarco-perf:/perf alpine:3 sh -c "mkdir -p $work && rm -rf $index && cp -r $source $index"
  for rate in $rates; do
    for readers in $readers_list; do
      cpus=$((readers + 1))
      [ $cpus -le 16 ] || cpus=16
      seed=$((seed + 1))
      docker run --rm --cpus $cpus --memory 12g -v search-msmarco-target:/target -v search-msmarco-perf:/perf \
        rust:1.99 /target/release/msmarco churn "$engine" $out "$index" "$readers" "$rate" "$seconds" "$seed" \
        "$work/$engine-$rate-$readers.json" >> "$log"
    done
  done
  docker run --rm -v search-msmarco-perf:/perf alpine:3 rm -rf "$index"
done
docker run --rm -v search-msmarco-perf:/perf -v "$repo:/src" alpine:3 sh -c \
  "cd $work && for f in *.json; do gzip -9 -c \$f > /src/$data/\$f.gz; done"
printf '{"ended":"%s"}\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$log"
cat "$log"
