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
# search-fast's arms: the fast reader under its switches, and the fast2 layout
fast-flat) line=$(env FAST_BOUNDS=loose FAST_SEED=0 FAST_NORMS=flat FAST_READ=sql FAST_AHEAD=8   /target/release/msmarco time fast $out/compact.db "$run/$set.tsv" "$results" "$mode") ;;
fast-sql) line=$(env FAST_BOUNDS=front FAST_SEED=1 FAST_NORMS=flat FAST_READ=sql FAST_AHEAD=8   /target/release/msmarco time fast $out/compact.db "$run/$set.tsv" "$results" "$mode") ;;
fast-blob) line=$(env FAST_BOUNDS=front FAST_SEED=1 FAST_NORMS=flat FAST_READ=blob   /target/release/msmarco time fast $out/compact.db "$run/$set.tsv" "$results" "$mode") ;;
fast2) line=$(env FAST2_TABLE=1 /target/release/msmarco time fast2 $out/compact.db "$run/$set.tsv" "$results" "$mode") ;;
fast2-notable) line=$(env FAST2_TABLE=0 /target/release/msmarco time fast2 $out/compact.db "$run/$set.tsv" "$results" "$mode") ;;
fast2-words) line=$(env FAST2_TABLE=0 FAST2_WORDS=1 /target/release/msmarco time fast2 $out/compact.db "$run/$set.tsv" "$results" "$mode") ;;
fast2-walk) line=$(env FAST2_TABLE=0 FAST2_WALK=1 /target/release/msmarco time fast2 $out/compact.db "$run/$set.tsv" "$results" "$mode") ;;
fast2-direct) line=$(env FAST2_TABLE=0 FAST2_DIRECT=1 /target/release/msmarco time fast2 $out/compact.db "$run/$set.tsv" "$results" "$mode") ;;
fast3) line=$(env FAST2_TABLE=0 FAST2_WORDS=1 FAST2_WALK=1 FAST2_DIRECT=1 /target/release/msmarco time fast2 $out/compact.db "$run/$set.tsv" "$results" "$mode") ;;
fast4) line=$(env FAST2_TABLE=0 FAST2_WORDS=1 FAST2_WALK=1 FAST2_DIRECT=1 FAST2_WINDOWS=1 /target/release/msmarco time fast2 $out/compact.db "$run/$set.tsv" "$results" "$mode") ;;
fast4-cache) line=$(env FAST2_TABLE=0 FAST2_WORDS=1 FAST2_WALK=1 FAST2_DIRECT=1 FAST2_WINDOWS=1 FAST2_CACHE=1 /target/release/msmarco time fast2 $out/compact.db "$run/$set.tsv" "$results" "$mode") ;;
tantivy) line=$(/target/release/msmarco time tantivy $out/tantivy "$run/$set.tsv" "$results" "$mode") ;;
tinystore) line=$(/target/release/msmarco time tinystore $out/tinystore "$run/$set.tsv" "$results" "$mode") ;;
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
