#!/bin/sh
# Words and vectors together on corpora out of bge-m3's fine-tuning: ESCI's
# products in English, Spanish and Japanese, and CQADupStack's twelve
# StackExchange forums, 600 judged questions each. Each cohort's compact
# words index and its vectors' lists are built here from what
# fetch-hybrid.sh fetched, hybrid-corpus.py laid out and embed.py embedded,
# copied first from the external disk they were made on; then the words,
# the vectors, and every fusion over the same lists, each question's ten
# kept to judge by kind of question.
set -eu
lab=/target/release/lab
given=/hybrid
data=/src/tinystore/reports/data/search-hybrid-2026-10-16
cd /src/tinystore/search-plan-lab
cargo build --release --locked --bin lab
(cd "$given/hybrid" && sha256sum -c --quiet /src/tinystore/search-plan-lab/hybrid.sha256)
mkdir -p "$data" /perf/hybrid
{
  echo "{\"uname\": \"$(uname -sr)\", \"nproc\": $(nproc), \"cpu\": \"$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //')\","
  echo " \"memory_kib\": $(grep MemTotal /proc/meminfo | tr -s ' ' | cut -d' ' -f2), \"rustc\": \"$(rustc --version)\","
  echo " \"cgroup_cpu\": \"$(cat /sys/fs/cgroup/cpu.max 2>/dev/null || echo none)\", \"cgroup_memory\": \"$(cat /sys/fs/cgroup/memory.max 2>/dev/null || echo none)\"}"
} > "$data/environment.json"
# SPECS runs some cohorts alone, and APPEND=1 adds them to what a run before wrote.
if [ "${APPEND:-}" != 1 ]; then
  for file in builds hybrid; do : > "$data/$file.jsonl"; done
  rm -rf /perf/hybrid/corpus && cp -r "$given/hybrid/lab" /perf/hybrid/corpus
fi

# cohort, clusters, the RaBitQ arm
for spec in ${SPECS:-esci:4096:rabitq:128:200 cqa:2048:rabitq:64:200}; do
  cohort=${spec%%:*}
  rest=${spec#*:}
  clusters=${rest%%:*}
  arm=${rest#*:}
  vectors=/perf/hybrid/vectors-$cohort
  out=/perf/hybrid/out-$cohort
  rm -rf "$vectors" "$out"
  mkdir -p "$vectors"
  cp "$given/vectors/$cohort/docs.f32" "$given/vectors/$cohort/questions.f32" "$given/vectors/$cohort/embed.json" "$vectors/"
  cp "$given/vectors/$cohort/embed.json" "$data/embed-$cohort.json"
  "$lab" build /perf/hybrid/corpus "$out" "$cohort" auto compact >> "$data/builds.jsonl"
  "$lab" vtruth "$vectors" 100 >> "$data/builds.jsonl"
  "$lab" vbuild "$vectors" "$out" vectors "$clusters" 131072 8 >> "$data/builds.jsonl"
  words=$out/$cohort-auto-compact
  for vectors_arm in truth "$arm"; do
    LAB_ROUND=$cohort LAB_EXECUTOR=window LAB_AHEAD=8 "$lab" vhybrid "$vectors" "$out" "$words" "$vectors_arm" 600 >> "$data/hybrid.jsonl"
  done
  mkdir -p "$data/raw-$cohort" "$given/judged/$cohort"
  for raw in "$out"/raw/hybrid-*.jsonl; do
    gzip -c "$raw" > "$data/raw-$cohort/$(basename "$raw").gz"
  done
  # The questions and their judgments are the corpus's, so they stay beside it.
  cp "$words/questions.json" "$given/judged/$cohort/questions.json"
  if [ "$cohort" = esci ]; then
    for locale in us es jp; do
      cp "/perf/hybrid/corpus/esci-$locale/qrels/graded.tsv" "$given/judged/esci/graded-$locale.tsv"
    done
  fi
done
