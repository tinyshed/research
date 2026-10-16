#!/bin/sh
# Vectors beside the words, a million passages of mixed-1m embedded by
# embed.py: the exact answer in memory, our lists in SQLite and three
# controls built from the same rows, every arm asked in one session in two
# rounds of opposite order, then the words, the vectors and their fusion
# judged on the 600 questions. The compact words index is the one
# run-compact.sh left in /perf/outc.
set -eu
lab=/target/release/lab
here=/src/tinystore/search-plan-lab
given=/src/tinystore/bench/corpus/vectors/mixed-1m
vectors=/perf/vectors
out=/perf/outv
words=/perf/outc/mixed-1m-auto-compact
data=/src/tinystore/reports/data/search-vectors-2026-10-16
cd "$here"
sh fetch-vec1.sh /src/tinystore/bench/corpus/vec1
VEC1_C=/src/tinystore/bench/corpus/vec1/vec1.c cargo build --release --locked --features vec1 --bin lab
(cd "$given" && sha256sum -c --quiet /src/tinystore/search-plan-lab/vectors.sha256)
mkdir -p "$vectors" "$out" "$data"
for file in docs.f32 questions.f32; do
  [ "$(stat -c %s "$vectors/$file" 2>/dev/null || echo 0)" = "$(stat -c %s "$given/$file")" ] || cp "$given/$file" "$vectors/$file"
done
cp "$given/embed.json" "$data/embed.json"
{
  echo "{\"uname\": \"$(uname -sr)\", \"nproc\": $(nproc), \"cpu\": \"$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //')\","
  echo " \"memory_kib\": $(grep MemTotal /proc/meminfo | tr -s ' ' | cut -d' ' -f2), \"rustc\": \"$(rustc --version)\","
  echo " \"cgroup_cpu\": \"$(cat /sys/fs/cgroup/cpu.max 2>/dev/null || echo none)\", \"cgroup_memory\": \"$(cat /sys/fs/cgroup/memory.max 2>/dev/null || echo none)\"}"
} > "$data/environment.json"
for file in builds queries hybrid; do : > "$data/$file.jsonl"; done

"$lab" vtruth "$vectors" 100 >> "$data/builds.jsonl"
"$lab" vbuild "$vectors" "$out" vectors 4096 131072 8 >> "$data/builds.jsonl"
"$lab" vbuild "$vectors" "$out" hnsw >> "$data/builds.jsonl"
"$lab" vbuild "$vectors" "$out" sqlitevec >> "$data/builds.jsonl"
"$lab" vbuild "$vectors" "$out" vec1 1024 32 102400 >> "$data/builds.jsonl"

# An arm and the questions it asks: the exact scans the first hundred, and
# sqlite-vec, which reads every vector as given, the first fifty.
arms="exact:100 sqlitevec:50"
for probes in 32 64 128 256 512; do arms="$arms codes:$probes:40:600 rabitq:$probes:100:600"; done
for keep in 25 50 200 400; do arms="$arms rabitq:128:$keep:600"; done
for keep in 25 50 100 200 400; do arms="$arms signs:128:$keep:600"; done
for expansion in 16 32 64 128 256; do arms="$arms hnsw:$expansion:600"; done
for probes in 8 16 32 64 128; do arms="$arms vec1:$probes:100:600"; done
reversed=$(for arm in $arms; do echo "$arm"; done | tac | tr '\n' ' ')

ask() {
  round=$1
  shift
  for spec in "$@"; do
    questions=${spec##*:}
    arm=${spec%:*}
    LAB_ROUND=$round "$lab" vquery "$vectors" "$out" "$words" "$arm" "$questions" >> "$data/queries.jsonl"
  done
}
# shellcheck disable=SC2086
ask 1 $arms
# shellcheck disable=SC2086
ask 2 $reversed

for arm in truth rabitq:64:200 rabitq:128:200 hnsw:128 vec1:32:200; do
  LAB_EXECUTOR=window LAB_AHEAD=8 "$lab" vhybrid "$vectors" "$out" "$words" "$arm" 600 >> "$data/hybrid.jsonl"
done
