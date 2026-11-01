#!/bin/sh
# search-tinystore: TinyStore's own search, the main tinystore/source holds,
# on MS MARCO beside the prototypes and the other engines. One step a call,
# from the repository's root, after round.sh built every other index and
# `msmarco fast2-build` wrote fast2.db:
#
#   sh tinystore/bench/search-msmarco/tiny/round.sh tools     # the msmarco binary, with TinyStore
#   sh tinystore/bench/search-msmarco/tiny/round.sh build     # TinyStore's index of the token stream
#   sh tinystore/bench/search-msmarco/tiny/round.sh answers   # its ten for the sample, beside compact's
#   sh tinystore/bench/search-msmarco/tiny/round.sh timing    # the arms, ROUNDS rounds (4)
#
# Passes are timing/pass.sh's, each container 2 CPUs and 6 GiB, warm, one
# query at a time, the arms in one order and then the other; raw lines go
# to tinystore/reports/data/search-tinystore-2026-11-02/.
set -eu
export MSYS_NO_PATHCONV=1
repo=$(pwd -W 2>/dev/null || pwd)
data=tinystore/reports/data/search-tinystore-2026-11-02
mkdir -p "$data/logs" "$data/answers"
mounts="-v $repo:/src -v search-msmarco-cargo:/usr/local/cargo/registry -v search-msmarco-target:/target \
  -v search-msmarco-perf:/perf -e CARGO_TARGET_DIR=/target"
bin=/target/release/msmarco
out=/perf/msmarco
limits="--cpus 2 --memory 6g"

case "${1:-}" in
tools)
  docker run --rm $mounts -w /src/tinystore/bench/search-msmarco rust:1.99 cargo build --release
  ;;
build)
  docker run --rm $limits $mounts rust:1.99 sh -c "rm -rf $out/tinystore; $bin build tinystore $out $out/tinystore" \
    > "$data/logs/build-tinystore.jsonl"
  docker run --rm -v search-msmarco-perf:/perf alpine:3 ls -la $out/tinystore > "$data/logs/files.txt"
  cat "$data/logs/build-tinystore.jsonl"
  ;;
answers)
  for engine in tinystore compact; do
    docker run --rm $limits $mounts rust:1.99 sh -c \
      "$bin answer $engine $([ $engine = tinystore ] && echo $out/tinystore || echo $out/compact.db) \
        $out/q/sample.tsv $out/tiny-$engine-sample.jsonl" >> "$data/logs/answers.jsonl"
    docker run --rm -v search-msmarco-perf:/perf -v "$repo:/src" alpine:3 sh -c \
      "gzip -9 -c $out/tiny-$engine-sample.jsonl > /src/$data/answers/$engine-sample.jsonl.gz"
  done
  python -I tinystore/bench/search-msmarco/tiny/agree.py "$data/answers" > "$data/answers/agree.md" 2>&1 \
    || python3 -I tinystore/bench/search-msmarco/tiny/agree.py "$data/answers" > "$data/answers/agree.md"
  cat "$data/answers/agree.md"
  ;;
timing)
  stamp=${STAMP:-$(date -u +%Y%m%dT%H%M%SZ)}
  rounds=${ROUNDS:-4}
  label=${LABEL:-measurement}
  run=$out/tiny-timing/$stamp
  timing=$data/timing/$stamp
  mkdir -p "$timing"
  log=$timing/passes.jsonl
  here=/src/tinystore/bench/search-msmarco/timing
  docker run --rm -v search-msmarco-perf:/perf alpine:3 sh -c "set -e; mkdir -p $run; \
    cp $out/q/sample.tsv $run/all.tsv; cp $out/q/sample.pisa $run/all.pisa; wc -l $run/all.tsv" > "$timing/sets.txt"
  printf '{"label":"%s","stamp":"%s","rounds":%s,"started":"%s","host":"%s"}\n' \
    "$label" "$stamp" "$rounds" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -s)" > "$log"
  docker info --format '{"docker":"{{.ServerVersion}}","kernel":"{{.KernelVersion}}","cpus":{{.NCPU}},"memory":{{.MemTotal}}}' >> "$log"
  pass() {
    case "$1" in
    compact | fast* | tantivy | tinystore)
      docker run --rm $limits -v "$repo:/src:ro" -v search-msmarco-target:/target -v search-msmarco-perf:/perf \
        rust:1.99 sh $here/pass.sh "$1" warm "$2" all "$run" >> "$log"
      ;;
    pisa-*)
      docker run --rm $limits -v "$repo:/src:ro" -v search-msmarco-pisa:/pisa -v search-msmarco-perf:/perf \
        gcc:13 sh $here/pass.sh "$1" warm "$2" all "$run" >> "$log"
      ;;
    lucene)
      docker run --rm $limits -v "$repo:/src:ro" -v search-msmarco-perf:/perf \
        eclipse-temurin:21-jdk sh $here/pass.sh "$1" warm "$2" all "$run" >> "$log"
      ;;
    esac
  }
  forward=${ARMS:-"compact fast4-cache tinystore tantivy pisa-block_max_maxscore lucene"}
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
    "cd $run && for f in *-*.jsonl; do gzip -9 -c \$f > /src/$timing/\$f.gz; done"
  printf '{"ended":"%s"}\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$log"
  python -I tinystore/bench/search-msmarco/bootstrap.py "$timing" "$label" > "$timing/summary.md" 2>&1 \
    || python3 -I tinystore/bench/search-msmarco/bootstrap.py "$timing" "$label" > "$timing/summary.md"
  cat "$timing/summary.md"
  ;;
*)
  echo "round.sh tools|build|answers|timing" >&2
  exit 2
  ;;
esac
