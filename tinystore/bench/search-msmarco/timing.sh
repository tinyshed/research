#!/bin/sh
# H2.1's timing harness, for a quiet machine: one command from the
# repository's root, after round.sh has built the tools and every index.
#
#   sh tinystore/bench/search-msmarco/timing.sh
#
# Five arms, each a process of its own a pass, one query at a time:
#   A compact   the lab's compact blocks, window executor, read ahead 8
#   B fts5      FTS5's bm25(), every match scored
#   C tantivy   Tantivy's block-max WAND
#   D pisa      PISA's block_max_maxscore, then its block_max_wand
#   E lucene    Lucene's windowed MaxScore, IndexSearcher.search(query, 10)
# Rounds A B C D E then E D C B A, cold first and then warm: four rounds. A
# cold pass evicts its arm's pages from the page cache (timing/evict.sh) and
# times each query once; a warm pass runs every query once untimed and then
# times each once. Every arm but FTS5 answers the `all` set, the 1,097
# queries of q/sample.tsv; every arm answers the `fts` set, its first 200,
# since FTS5 takes seconds a query. Each container has 2 CPUs and 6 GiB, as
# the search rounds before this one.
#
# QUERIES=n cuts both sets to their first n queries, FTS5_QUERIES=m the fts
# set to m; LABEL names the run in its log. The passes' raw lines go to
# tinystore/reports/data/search-msmarco-2026-11-01/timing/<stamp>/, and
# bootstrap.py turns them into p50 and p99 with 95 % intervals.
set -eu
export MSYS_NO_PATHCONV=1
repo=$(pwd -W 2>/dev/null || pwd)
stamp=${STAMP:-$(date -u +%Y%m%dT%H%M%SZ)}
queries=${QUERIES:-0}
fts_queries=${FTS5_QUERIES:-200}
label=${LABEL:-measurement}
out=/perf/msmarco
run=$out/timing/$stamp
data=tinystore/reports/data/search-msmarco-2026-11-01/timing/$stamp
mkdir -p "$data"
log=$data/passes.jsonl
here=/src/tinystore/bench/search-msmarco/timing
limits="--cpus 2 --memory 6g"

docker run --rm -v search-msmarco-perf:/perf alpine:3 sh -c "set -e; mkdir -p $run; cd $out/q; \
  if [ $queries -gt 0 ]; then head -n $queries sample.tsv > $run/all.tsv; head -n $queries sample.pisa > $run/all.pisa; \
  else cp sample.tsv $run/all.tsv; cp sample.pisa $run/all.pisa; fi; \
  head -n $fts_queries $run/all.tsv > $run/fts.tsv; head -n $fts_queries $run/all.pisa > $run/fts.pisa; \
  wc -l $run/all.tsv $run/fts.tsv" > "$data/sets.txt"
printf '{"label":"%s","stamp":"%s","queries":%s,"fts5_queries":%s,"started":"%s","host":"%s"}\n' \
  "$label" "$stamp" "$queries" "$fts_queries" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$(uname -s)" > "$log"
docker info --format '{"docker":"{{.ServerVersion}}","kernel":"{{.KernelVersion}}","cpus":{{.NCPU}},"memory":{{.MemTotal}}}' >> "$log"

pass() {
  arm=$1 mode=$2 round=$3 set=$4
  if [ "$mode" = cold ]; then
    docker run --rm -v "$repo:/src:ro" -v search-msmarco-perf:/perf gcc:13 sh $here/evict.sh "$arm" >> "$log"
  fi
  case "$arm" in
  compact | fts5 | tantivy)
    docker run --rm $limits -v "$repo:/src:ro" -v search-msmarco-target:/target -v search-msmarco-perf:/perf \
      rust:1.99 sh $here/pass.sh "$arm" "$mode" "$round" "$set" "$run" >> "$log"
    ;;
  pisa-*)
    docker run --rm $limits -v "$repo:/src:ro" -v search-msmarco-pisa:/pisa -v search-msmarco-perf:/perf \
      gcc:13 sh $here/pass.sh "$arm" "$mode" "$round" "$set" "$run" >> "$log"
    ;;
  lucene)
    docker run --rm $limits -v "$repo:/src:ro" -v search-msmarco-perf:/perf \
      eclipse-temurin:21-jdk sh $here/pass.sh "$arm" "$mode" "$round" "$set" "$run" >> "$log"
    ;;
  esac
}

slot() {
  arm=$1 mode=$2 round=$3
  case "$arm" in
  fts5) pass fts5 "$mode" "$round" fts ;;
  pisa)
    for algorithm in pisa-block_max_maxscore pisa-block_max_wand; do
      pass $algorithm "$mode" "$round" all
      pass $algorithm "$mode" "$round" fts
    done
    ;;
  *)
    pass "$arm" "$mode" "$round" all
    pass "$arm" "$mode" "$round" fts
    ;;
  esac
}

round=0
for mode in cold warm; do
  for order in "compact fts5 tantivy pisa lucene" "lucene pisa tantivy fts5 compact"; do
    round=$((round + 1))
    for arm in $order; do
      slot "$arm" "$mode" "$round"
    done
  done
done

docker run --rm -v search-msmarco-perf:/perf -v "$repo:/src" alpine:3 sh -c \
  "cd $run && for f in *-*.jsonl; do gzip -9 -c \$f > /src/$data/\$f.gz; done"
printf '{"ended":"%s"}\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$log"
python -I tinystore/bench/search-msmarco/bootstrap.py "$data" "$label" > "$data/summary.md" 2>&1 \
  || python3 -I tinystore/bench/search-msmarco/bootstrap.py "$data" "$label" > "$data/summary.md"
cat "$data/summary.md"
