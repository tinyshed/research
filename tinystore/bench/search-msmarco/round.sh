#!/bin/sh
# The timing-free half of the round, step by step, from Git Bash or any POSIX
# shell with Docker: `sh round.sh <step>`, run from the repository's root.
# Every index lives on the search-msmarco-perf volume at /perf/msmarco; the
# corpus is the bind mount's tinystore/bench/corpus/msmarco, fetched by
# fetch.sh. Raw output lands in tinystore/reports/data/search-msmarco-2026-11-01.
#
#   sh tinystore/bench/search-msmarco/round.sh tools      the Rust binary, PISA, its driver, Lucene's classes
#   sh tinystore/bench/search-msmarco/round.sh prepare    tokens, PISA's binary collection, the query sets
#   sh tinystore/bench/search-msmarco/round.sh build <compact|fts5|tantivy|tantivy-keyed|lucene|pisa>
#   sh tinystore/bench/search-msmarco/round.sh answers <engine> <set>
#   sh tinystore/bench/search-msmarco/round.sh exact <set>
#   sh tinystore/bench/search-msmarco/round.sh sizes
#
# then, on the host, `python -I tinystore/bench/search-msmarco/evaluate.py
# tinystore/reports/data/search-msmarco-2026-11-01 tinystore/bench/corpus/msmarco`.
set -eu
export MSYS_NO_PATHCONV=1
repo=$(pwd -W 2>/dev/null || pwd)
data=tinystore/reports/data/search-msmarco-2026-11-01
proxy=${PROXY:-}
mkdir -p "$data/answers" "$data/logs"
rust() {
  docker run --rm "$@"
}
mounts="-v $repo:/src -v search-msmarco-cargo:/usr/local/cargo/registry -v search-msmarco-target:/target \
  -v search-msmarco-perf:/perf -e CARGO_TARGET_DIR=/target"
bin=/target/release/msmarco
out=/perf/msmarco
lucene_cp=/perf/lucene/classes:/perf/lucene/lucene-core-10.5.2.jar:/perf/lucene/lucene-analysis-common-10.5.2.jar
java_flags="--add-modules jdk.incubator.vector --enable-native-access=ALL-UNNAMED"

step=${1:-}
case "$step" in
tools)
  rust $mounts -e HTTPS_PROXY="$proxy" -w /src/tinystore/bench/search-msmarco rust:1.99 cargo build --release
  docker run --rm -v "$repo/tinystore/bench/search-msmarco/pisa:/scripts:ro" -v search-msmarco-pisa:/pisa \
    -e http_proxy="$proxy" -e https_proxy="$proxy" gcc:13 sh /scripts/build.sh
  docker run --rm -v "$repo/tinystore/bench/search-msmarco/pisa:/driver-src:ro" -v search-msmarco-pisa:/pisa \
    -e http_proxy="$proxy" -e https_proxy="$proxy" gcc:13 sh /driver-src/driver.sh
  docker run --rm -v "$repo:/src:ro" -v search-msmarco-perf:/perf -e https_proxy="$proxy" eclipse-temurin:21-jdk \
    sh /src/tinystore/bench/search-msmarco/lucene/fetch.sh /perf/lucene
  ;;
prepare)
  rust --cpus 6 --memory 10g $mounts rust:1.99 sh -c \
    "$bin prepare /src/tinystore/bench/corpus/msmarco $out 6 && $bin queries /src/tinystore/bench/corpus/msmarco $out" \
    > "$data/logs/prepare.jsonl"
  ;;
build)
  case "$2" in
  compact | fts5)
    rust --cpus 2 --memory 6g $mounts rust:1.99 sh -c "rm -f $out/$2.db*; $bin build $2 $out $out/$2.db" \
      > "$data/logs/build-$2.jsonl"
    ;;
  tantivy | tantivy-keyed)
    rust --cpus 2 --memory 6g $mounts rust:1.99 sh -c "rm -rf $out/$2; $bin build $2 $out $out/$2" \
      > "$data/logs/build-$2.jsonl"
    ;;
  lucene)
    docker run --rm --cpus 2 --memory 6g -v search-msmarco-perf:/perf eclipse-temurin:21-jdk sh -c \
      "rm -rf $out/lucene && java -Xmx4g $java_flags -cp $lucene_cp Index $out/tokens.tsv $out/lucene" \
      > "$data/logs/build-lucene.jsonl"
    ;;
  pisa)
    docker run --rm --cpus 4 --memory 6g -v "$repo:/src:ro" -v search-msmarco-pisa:/pisa -v search-msmarco-perf:/perf \
      gcc:13 sh /src/tinystore/bench/search-msmarco/pisa/index.sh $out > "$data/logs/build-pisa.jsonl"
    ;;
  esac
  ;;
answers)
  engine=$2
  set=$3
  results=$out/answers/$engine-$set.jsonl
  case "$engine" in
  compact*) index=$out/compact.db ;;
  fts5) index=$out/fts5.db ;;
  tantivy*) index=$out/tantivy ;;
  esac
  case "$engine" in
  lucene | lucene-exhaustive)
    mode=default
    [ "$engine" = lucene ] || mode=exhaustive
    docker run --rm --cpus 1 --memory 4g -v search-msmarco-perf:/perf eclipse-temurin:21-jdk sh -c \
      "mkdir -p $out/answers && java -Xmx2g $java_flags -cp $lucene_cp Search $out/lucene $out/q/$set.tsv $results $mode answer"
    ;;
  pisa-*)
    docker run --rm --cpus 1 --memory 4g -v search-msmarco-pisa:/pisa -v search-msmarco-perf:/perf gcc:13 sh -c \
      "mkdir -p $out/answers && /pisa/driver/pass $out/pisa.block_simdbp.idx $out/pisa.bm25.bmw $out/q/$set.pisa \
       $results ${engine#pisa-} answer"
    ;;
  *)
    rust --cpus 1 --memory 4g $mounts -e LAB_EXECUTOR=window rust:1.99 sh -c \
      "mkdir -p $out/answers && $bin answer $engine $index $out/q/$set.tsv $results"
    ;;
  esac >> "$data/logs/answers.jsonl"
  docker run --rm -v search-msmarco-perf:/perf -v "$repo:/src" alpine:3 sh -c \
    "gzip -9 -c $results > /src/$data/answers/$engine-$set.jsonl.gz"
  ;;
answers-parallel)
  # The same answers as `answers`, the query file cut into <parts> runs at once:
  # for FTS5, which scores every match and takes seconds a query.
  engine=$2
  set=$3
  parts=$4
  case "$engine" in
  compact*) index=$out/compact.db ;;
  fts5) index=$out/fts5.db ;;
  tantivy*) index=$out/tantivy ;;
  esac
  rust --cpus "$parts" --memory 6g $mounts rust:1.99 sh -c "mkdir -p $out/answers && cd $out/q && \
    split -n l/$parts -d $set.tsv $set.part. && for part in $set.part.*; do \
      $bin answer $engine $index $out/q/\$part $out/answers/$engine-\$part.jsonl & done; wait; \
    cat $out/answers/$engine-$set.part.* > $out/answers/$engine-$set.jsonl && \
    rm $out/answers/$engine-$set.part.* $out/q/$set.part.*" >> "$data/logs/answers.jsonl"
  docker run --rm -v search-msmarco-perf:/perf -v "$repo:/src" alpine:3 sh -c \
    "gzip -9 -c $out/answers/$engine-$set.jsonl > /src/$data/answers/$engine-$set.jsonl.gz"
  ;;
dev-rest)
  # The dev queries the sample does not hold, for FTS5, whose sample answers
  # already hold the other 1,000; merge.py joins the two into fts5-dev.
  docker run --rm -v search-msmarco-perf:/perf alpine:3 sh -c \
    "cd $out/q && awk -F '\t' 'NR == FNR { held[\$1]; next } !(\$1 in held)' sample.tsv dev.tsv > dev-rest.tsv \
     && wc -l dev-rest.tsv"
  ;;
exact)
  set=$2
  rust --cpus 4 --memory 8g $mounts rust:1.99 sh -c \
    "mkdir -p $out/answers && $bin exact $out $out/q/$set.tsv $out/answers/exact-$set.jsonl 4" \
    >> "$data/logs/exact.jsonl"
  docker run --rm -v search-msmarco-perf:/perf -v "$repo:/src" alpine:3 sh -c \
    "gzip -9 -c $out/answers/exact-$set.jsonl > /src/$data/answers/exact-$set.jsonl.gz"
  ;;
sizes)
  rust $mounts rust:1.99 sh -c "$bin sizes $out/tantivy; $bin sizes $out/tantivy-keyed; $bin sizes $out/lucene" \
    > "$data/logs/sizes.jsonl"
  docker run --rm -v "$repo:/src:ro" -v search-msmarco-perf:/perf golang:1.27 \
    python3 -I /src/tinystore/bench/search-msmarco/columns.py $out/compact.db > "$data/logs/compact-columns.jsonl"
  docker run --rm -v search-msmarco-perf:/perf alpine:3 ls -la $out $out/tantivy $out/lucene > "$data/logs/files.txt"
  ;;
*)
  echo "round.sh tools|prepare|build <engine>|answers <engine> <set>|exact <set>|sizes" >&2
  exit 2
  ;;
esac
