#!/bin/sh
# One pass of one arm, inside the arm's own image: `pass.sh <arm> <mode>
# <round> <set> <run>`. <mode> is warm (every query once untimed, then once
# timed) or cold (once timed; timing.sh has evicted the arm's pages first);
# <set> names <run>/<set>.tsv and <run>/<set>.pisa. Each query's nanoseconds
# and ids go to <run>/<arm>-<mode>-<round>-<set>.jsonl; one line about the
# pass goes to standard output.
set -eu
arm=$1
mode=$2
round=$3
set=$4
run=$5
out=/perf/msmarco
results=$run/$arm-$mode-$round-$set.jsonl
lucene_cp=/perf/lucene/classes:/perf/lucene/lucene-core-10.5.2.jar:/perf/lucene/lucene-analysis-common-10.5.2.jar
started=$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)
case "$arm" in
compact) line=$(/target/release/msmarco time compact $out/compact.db "$run/$set.tsv" "$results" "$mode") ;;
fts5) line=$(/target/release/msmarco time fts5 $out/fts5.db "$run/$set.tsv" "$results" "$mode") ;;
tantivy) line=$(/target/release/msmarco time tantivy $out/tantivy "$run/$set.tsv" "$results" "$mode") ;;
pisa-*)
  line=$(/pisa/driver/pass $out/pisa.block_simdbp.idx $out/pisa.bm25.bmw "$run/$set.pisa" "$results" \
    "${arm#pisa-}" "$mode")
  ;;
lucene)
  line=$(java -Xmx2g --add-modules jdk.incubator.vector --enable-native-access=ALL-UNNAMED -cp "$lucene_cp" \
    Search $out/lucene "$run/$set.tsv" "$results" default "$mode" 2> /dev/null)
  ;;
*) echo "no arm $arm" >&2; exit 2 ;;
esac
printf '{"arm":"%s","mode":"%s","round":%s,"set":"%s","started":"%s","ended":"%s","pass":%s}\n' \
  "$arm" "$mode" "$round" "$set" "$started" "$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)" "$line"
