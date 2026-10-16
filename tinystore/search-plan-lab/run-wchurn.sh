#!/bin/sh
# The words' writer held in steps: three copies of the compact index of
# search-compact-2026-10-16, each 50,000 operations of one seed, 70 searches,
# 25 sets and 5 deletes in a hundred, every 25th search checked against
# scoring every posting. The head is packed every thousand writes: whole in
# one commit, as search-compact packed it; 2,048 of its rows a commit, keys
# whole, until it is empty; and so, with the documents gone taken out of the
# blocks that hold them once they are 0.5 % of the documents, in steps of
# 2,048 keys.
set -eu
lab=/target/release/lab
corpus=/src/tinystore/bench/corpus/nomiracl
base=/perf/outc/mixed-1m-auto-compact
data=/src/tinystore/reports/data/search-wchurn-2026-10-16
cd /src/tinystore/search-plan-lab
cargo build --release --locked --bin lab
(cd "$corpus" && sha256sum -c --quiet /src/tinystore/search-plan-lab/corpus.sha256)
mkdir -p "$data"
{
  echo "{\"uname\": \"$(uname -sr)\", \"nproc\": $(nproc), \"cpu\": \"$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //')\","
  echo " \"memory_kib\": $(grep MemTotal /proc/meminfo | tr -s ' ' | cut -d' ' -f2), \"rustc\": \"$(rustc --version)\","
  echo " \"cgroup_cpu\": \"$(cat /sys/fs/cgroup/cpu.max 2>/dev/null || echo none)\", \"cgroup_memory\": \"$(cat /sys/fs/cgroup/memory.max 2>/dev/null || echo none)\"}"
} > "$data/environment.json"
: > "$data/churn.jsonl"

arm() {
  name=$1
  shift
  rm -rf "/perf/wc-$name"
  mkdir -p "/perf/wc-$name"
  cp -r "$base" "/perf/wc-$name/"
  LAB_EXECUTOR=window "$lab" churn "$corpus" "/perf/wc-$name" mixed-1m auto 50000 1000 25 7 "$@" >> "$data/churn.jsonl"
  rm -rf "/perf/wc-$name"
}
arm whole 0 0
arm steps 2048 0
arm purged 2048 0.005
