#!/bin/sh
# One pass of one arm of search-fields, inside rust:1.99: `pass.sh <arm>
# <mode> <round> <set> <run>`, as timing/pass.sh runs one of search-msmarco's.
# Each query's nanoseconds and ids go to <run>/<arm>-<mode>-<round>-<set>.jsonl;
# one line about the pass goes to standard output.
set -eu
arm=$1
mode=$2
round=$3
set=$4
run=$5
out=/perf/msmarco
results=$run/$arm-$mode-$round-$set.jsonl
bin=/target/release/msmarco
started=$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)
case "$arm" in
lab) line=$(env LAB_EXECUTOR=window LAB_AHEAD=8 $bin fields-time lab $out/fields-lab.db "$run/$set.tsv" "$results" "$mode") ;;
native) line=$($bin fields-time native $out/fields-native.db "$run/$set.tsv" "$results" "$mode") ;;
native-nocache) line=$(env FIELDS_CACHE=0 $bin fields-time native $out/fields-native.db "$run/$set.tsv" "$results" "$mode") ;;
tantivy) line=$($bin fields-time tantivy $out/fields-tantivy "$run/$set.tsv" "$results" "$mode") ;;
tantivy-equal) line=$(env FIELDS_EQUAL=1 $bin fields-time tantivy $out/fields-tantivy "$run/$set.tsv" "$results" "$mode") ;;
native-equal) line=$(env FIELDS_EQUAL=1 $bin fields-time native $out/fields-native.db "$run/$set.tsv" "$results" "$mode") ;;
*) echo "no arm $arm" >&2; exit 2 ;;
esac
printf '{"arm":"%s","mode":"%s","round":%s,"set":"%s","started":"%s","ended":"%s","pass":%s}\n' \
  "$arm" "$mode" "$round" "$set" "$started" "$(date -u +%Y-%m-%dT%H:%M:%S.%NZ)" "$line"
