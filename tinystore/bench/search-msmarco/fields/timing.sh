#!/bin/sh
# search-fields' timing: the lab's layout, the native layout with and without
# its directories kept, and Tantivy, over two fields, warm, one query at a
# time; the sample's 1,097 queries (`fields`) and those whose last word is a
# prefix (`prefix`); rounds in one order and then the other, ROUNDS of them.
# One command from the repository's root, after the fields-build steps of
# fields/README.md:
#
#   sh tinystore/bench/search-msmarco/fields/timing.sh
#
# Raw lines go to tinystore/reports/data/search-fields-2026-11-01/timing/<stamp>/
# and bootstrap.py summarises them, each arm against `tantivy`.
set -eu
export MSYS_NO_PATHCONV=1
repo=$(pwd -W 2>/dev/null || pwd)
stamp=${STAMP:-$(date -u +%Y%m%dT%H%M%SZ)}
rounds=${ROUNDS:-4}
label=${LABEL:-measurement}
run=/perf/msmarco/fields-timing/$stamp
data=tinystore/reports/data/search-fields-2026-11-01/timing/$stamp
mkdir -p "$data"
log=$data/passes.jsonl
docker run --rm -v search-msmarco-perf:/perf alpine:3 sh -c "set -e; mkdir -p $run; \
  cp /perf/msmarco/q/fields-sample.tsv $run/fields.tsv; cp /perf/msmarco/q/prefix-sample.tsv $run/prefix.tsv; \
  wc -l $run/fields.tsv $run/prefix.tsv" > "$data/sets.txt"
printf '{"label":"%s","stamp":"%s","rounds":%s,"started":"%s","host":"%s"}\n' \
  "$label" "$stamp" "$rounds" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -s)" > "$log"
docker info --format '{"docker":"{{.ServerVersion}}","kernel":"{{.KernelVersion}}","cpus":{{.NCPU}},"memory":{{.MemTotal}}}' >> "$log"

pass() {
  docker run --rm --cpus 2 --memory 6g -v "$repo:/src:ro" -v search-msmarco-target:/target -v search-msmarco-perf:/perf \
    rust:1.99 sh /src/tinystore/bench/search-msmarco/fields/pass.sh "$1" warm "$2" "$3" "$run" >> "$log"
}

forward=${ARMS:-"lab native native-nocache tantivy"}
backward=""
for arm in $forward; do backward="$arm $backward"; done
round=0
while [ "$round" -lt "$rounds" ]; do
  round=$((round + 1))
  if [ $((round % 2)) -eq 1 ]; then order=$forward; else order=$backward; fi
  for arm in $order; do
    pass "$arm" "$round" fields
    pass "$arm" "$round" prefix
  done
done

docker run --rm -v search-msmarco-perf:/perf -v "$repo:/src" alpine:3 sh -c \
  "cd $run && for f in *-*.jsonl; do gzip -9 -c \$f > /src/$data/\$f.gz; done"
printf '{"ended":"%s"}\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$log"
python -I tinystore/bench/search-msmarco/bootstrap.py "$data" "$label" > "$data/summary.md" 2>&1 \
  || python3 -I tinystore/bench/search-msmarco/bootstrap.py "$data" "$label" > "$data/summary.md"
cat "$data/summary.md"
