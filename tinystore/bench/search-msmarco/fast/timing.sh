#!/bin/sh
# search-fast's timing: the compact blocks, the fast reader under its
# switches, the fast2 layout, Tantivy, PISA's block_max_maxscore and Lucene,
# warm, one query at a time, the sample's 1,097 queries; rounds in one order
# and then the other, ROUNDS of them (4 by default). One command from the
# repository's root, after round.sh has built every index and
# `msmarco fast-build` and `msmarco fast2-build` have written fast.db and
# fast2.db beside compact.db.
#
#   sh tinystore/bench/search-msmarco/fast/timing.sh
#
# Passes are timing/pass.sh's, each container 2 CPUs and 6 GiB; raw lines go
# to tinystore/reports/data/search-fast-2026-11-01/timing/<stamp>/ and
# bootstrap.py summarises them.
set -eu
export MSYS_NO_PATHCONV=1
repo=$(pwd -W 2>/dev/null || pwd)
stamp=${STAMP:-$(date -u +%Y%m%dT%H%M%SZ)}
rounds=${ROUNDS:-4}
label=${LABEL:-measurement}
run=/perf/msmarco/fast-timing/$stamp
data=tinystore/reports/data/search-fast-2026-11-01/timing/$stamp
mkdir -p "$data"
log=$data/passes.jsonl
here=/src/tinystore/bench/search-msmarco/timing
limits="--cpus 2 --memory 6g"
docker run --rm -v search-msmarco-perf:/perf alpine:3 sh -c "set -e; mkdir -p $run; \
  cp /perf/msmarco/q/sample.tsv $run/all.tsv; cp /perf/msmarco/q/sample.pisa $run/all.pisa; wc -l $run/all.tsv" > "$data/sets.txt"
printf '{"label":"%s","stamp":"%s","rounds":%s,"started":"%s","host":"%s"}\n' \
  "$label" "$stamp" "$rounds" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -s)" > "$log"
docker info --format '{"docker":"{{.ServerVersion}}","kernel":"{{.KernelVersion}}","cpus":{{.NCPU}},"memory":{{.MemTotal}}}' >> "$log"

pass() {
  arm=$1 round=$2
  case "$arm" in
  compact | fast* | tantivy)
    docker run --rm $limits -v "$repo:/src:ro" -v search-msmarco-target:/target -v search-msmarco-perf:/perf \
      rust:1.99 sh $here/pass.sh "$arm" warm "$round" all "$run" >> "$log"
    ;;
  pisa-*)
    docker run --rm $limits -v "$repo:/src:ro" -v search-msmarco-pisa:/pisa -v search-msmarco-perf:/perf \
      gcc:13 sh $here/pass.sh "$arm" warm "$round" all "$run" >> "$log"
    ;;
  lucene)
    docker run --rm $limits -v "$repo:/src:ro" -v search-msmarco-perf:/perf \
      eclipse-temurin:21-jdk sh $here/pass.sh "$arm" warm "$round" all "$run" >> "$log"
    ;;
  esac
}

forward=${ARMS:-"compact fast-flat fast-sql fast-blob fast2-notable fast2 tantivy pisa-block_max_maxscore lucene"}
backward=""
for arm in $forward; do backward="$arm $backward"; done
round=0
while [ "$round" -lt "$rounds" ]; do
  round=$((round + 1))
  if [ $((round % 2)) -eq 1 ]; then order=$forward; else order=$backward; fi
  for arm in $order; do
    pass "$arm" "$round"
  done
done

docker run --rm -v search-msmarco-perf:/perf -v "$repo:/src" alpine:3 sh -c \
  "cd $run && for f in *-*.jsonl; do gzip -9 -c \$f > /src/$data/\$f.gz; done"
printf '{"ended":"%s"}\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$log"
python -I tinystore/bench/search-msmarco/bootstrap.py "$data" "$label" > "$data/summary.md" 2>&1 \
  || python3 -I tinystore/bench/search-msmarco/bootstrap.py "$data" "$label" > "$data/summary.md"
cat "$data/summary.md"
