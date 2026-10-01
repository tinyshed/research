#!/bin/sh
# Saved source archives at /base and /candidate, named-volume data at /data.
set -eu
out="${OUT:?}"
mkdir -p "$out" /data/summary-tmp /data/summary-density/base /data/summary-density/candidate
export TMPDIR=/data/summary-tmp
cp /candidate/metrics/shortcut_bench_test.go /base/metrics/shortcut_bench_test.go
go test -C /base -c -o /tmp/summary-base ./metrics
go test -C /candidate -c -o /tmp/summary-candidate ./metrics
{
  date -u +%Y-%m-%dT%H:%M:%SZ
  go version
  uname -r
  nproc
  grep -m1 'model name' /proc/cpuinfo
  printf 'base %s\ncandidate %s\nharness %s\n' "$BASE_COMMIT" "$CANDIDATE_COMMIT" "$HARNESS_COMMIT"
} > "$out/environment.txt"
for round in $(seq 1 "${REPEATS:-8}"); do
  order="base candidate"
  [ "$((round % 2))" = 0 ] && order="candidate base"
  for revision in $order; do
    /tmp/summary-$revision -test.run '^$' \
      -test.bench 'BenchmarkWholeBlockAggregate|BenchmarkCutBlockAggregate' \
      -test.benchtime=1s -test.count=1 > "$out/$revision-$round.txt" 2>&1
  done
done
for revision in base candidate; do
  TINYSTORE_JSONL=/corpus/tsbs-packed/series.jsonl \
    TINYSTORE_CORPUS_DB=/data/summary-density/$revision/metrics.db \
    /tmp/summary-$revision -test.run '^TestCorpusThroughPublicStore$' -test.v \
    > "$out/density-$revision.txt" 2>&1
done
printf '%s DONE\n' "$(date -u +%H:%M:%S)" > "$out/done.txt"
