#!/bin/sh
# Writes into the vectors' lists. An index built from the first 696,059
# rows of mixed-1m, every passage but the Chinese ones of MIRACL's shard,
# so that its clusters were trained without them; then a hundred batches of
# a thousand of those Chinese passages inserted, two hundred documents given
# another passage's vector and a hundred deleted, each write its own commit.
# Three copies: never packed; each touched cluster packed in a transaction
# of its own after every batch; and packed and split past twice the mean.
# Every twentieth batch, before its upkeep, the 600 questions against the
# exact answer over what is live then.
set -eu
lab=/target/release/lab
given=/src/tinystore/bench/corpus/vectors/mixed-1m
vectors=/perf/vectors
outs=/perf/outvc
data=/src/tinystore/reports/data/search-vchurn-2026-10-16
first=696059
cd /src/tinystore/search-plan-lab
sh fetch-vec1.sh /src/tinystore/bench/corpus/vec1
VEC1_C=/src/tinystore/bench/corpus/vec1/vec1.c cargo build --release --locked --features vec1 --bin lab
(cd "$given" && sha256sum -c --quiet /src/tinystore/search-plan-lab/vectors.sha256)
mkdir -p "$vectors" "$data"
for file in docs.f32 questions.f32; do
  [ "$(stat -c %s "$vectors/$file" 2>/dev/null || echo 0)" = "$(stat -c %s "$given/$file")" ] || cp "$given/$file" "$vectors/$file"
done
{
  echo "{\"uname\": \"$(uname -sr)\", \"nproc\": $(nproc), \"cpu\": \"$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //')\","
  echo " \"memory_kib\": $(grep MemTotal /proc/meminfo | tr -s ' ' | cut -d' ' -f2), \"rustc\": \"$(rustc --version)\","
  echo " \"cgroup_cpu\": \"$(cat /sys/fs/cgroup/cpu.max 2>/dev/null || echo none)\", \"cgroup_memory\": \"$(cat /sys/fs/cgroup/memory.max 2>/dev/null || echo none)\"}"
} > "$data/environment.json"
for file in builds churn; do : > "$data/$file.jsonl"; done
rm -rf "$outs" "$vectors/churn-truth"

LAB_ROWS=$first "$lab" vbuild "$vectors" "$outs/base" vectors 4096 131072 8 >> "$data/builds.jsonl"
for mode in never clusters split; do
  cp -r "$outs/base" "$outs/$mode"
  "$lab" vchurn "$vectors" "$outs/$mode" $first 100 1000 200 100 $mode 20 1 >> "$data/churn.jsonl"
  rm -rf "$outs/$mode"
done
rm -rf "$outs"
