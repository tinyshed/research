#!/bin/sh
# The compact blocks against the packed ones, Tantivy and FTS5 in one
# session, a million documents each engine owns: every engine built here
# but the packed blocks run-packed.sh left, the compact ones checked against
# scoring every posting by each executor with and without reading ahead,
# two rounds of queries in opposite orders, then a long mix of searches,
# sets and deletes on two copies of the compact index, its head packed
# every thousand writes on one and never on the other.
set -eu
lab=/target/release/lab
corpus=/src/tinystore/bench/corpus/nomiracl
out=/perf/outc
packed=/perf/outp
data=/src/tinystore/reports/data/search-compact-2026-10-16
cd /src/tinystore/search-plan-lab
cargo build --release --locked --bin lab
(cd "$corpus" && sha256sum -c --quiet /src/tinystore/search-plan-lab/corpus.sha256)
(cd "$corpus/../miracl" && sha256sum -c --quiet /src/tinystore/search-plan-lab/miracl.sha256)
rm -rf /perf/outb /perf/out1m /perf/outw /perf/out "$out" /perf/churn-*
mkdir -p "$data" "$out"
{
  echo "{\"uname\": \"$(uname -sr)\", \"nproc\": $(nproc), \"cpu\": \"$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //')\","
  echo " \"memory_kib\": $(grep MemTotal /proc/meminfo | tr -s ' ' | cut -d' ' -f2), \"rustc\": \"$(rustc --version)\","
  echo " \"cgroup_cpu\": \"$(cat /sys/fs/cgroup/cpu.max 2>/dev/null || echo none)\", \"cgroup_memory\": \"$(cat /sys/fs/cgroup/memory.max 2>/dev/null || echo none)\"}"
} > "$data/environment.json"
for file in builds checks queries churn; do : > "$data/$file.jsonl"; done

LAB_POSITIONS=1 "$lab" build "$corpus" "$out" mixed-1m auto compact >> "$data/builds.jsonl"
"$lab" build "$corpus" "$out" mixed-1m auto tantivy >> "$data/builds.jsonl"
"$lab" build "$corpus" "$out" mixed-1m auto fts >> "$data/builds.jsonl"
for executor in cursor window; do
  for ahead in 1 8; do
    LAB_EXECUTOR=$executor LAB_AHEAD=$ahead "$lab" check "$out" mixed-1m auto or,pruned-or 0.05 compact >> "$data/checks.jsonl"
  done
done

# One arm: a label, the index's directory, its backend, and its settings.
read_arm() {
  round=$1 label=$2 dir=$3 backend=$4
  shift 4
  for cut in 0.05 0.02; do
    plans=pruned-or
    [ "$cut" = 0.05 ] && plans=or,pruned-or
    env LAB_ROUND="$round" "$@" "$lab" query "$dir" mixed-1m auto "$backend" "$plans" 1 "$cut" clean exact > /perf/lines.jsonl
    sed "s|\"cohort\":\"mixed-1m\"|\"cohort\":\"mixed-1m\",\"arm\":\"$label\"|" /perf/lines.jsonl >> "$data/queries.jsonl"
  done
}
for round in 1 2; do
  arms="compact compact-ahead packed tantivy fts"
  [ "$round" = 2 ] && arms="fts tantivy packed compact-ahead compact"
  for arm in $arms; do
    case $arm in
      compact) read_arm "$round" compact "$out" compact LAB_EXECUTOR=window LAB_AHEAD=1 ;;
      compact-ahead) read_arm "$round" compact-ahead "$out" compact LAB_EXECUTOR=window LAB_AHEAD=8 ;;
      packed) read_arm "$round" packed "$packed" blocks LAB_EXECUTOR=window ;;
      tantivy) read_arm "$round" tantivy "$out" tantivy ;;
      fts) read_arm "$round" fts "$out" fts ;;
    esac
  done
done

for every in 1000 0; do
  mkdir -p /perf/churn-$every
  cp -r "$out/mixed-1m-auto-compact" /perf/churn-$every/
  LAB_EXECUTOR=window "$lab" churn "$corpus" /perf/churn-$every mixed-1m auto 50000 "$every" 25 11 >> "$data/churn.jsonl"
done

du -s --block-size=1 "$out"/*/* /perf/churn-*/*/* > "$data/index-files.txt"
