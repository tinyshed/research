#!/bin/sh
# The round in the Linux container: every index built, then every cohort's
# questions answered, each step a process of its own. Two rounds alternate
# FTS5 and Tantivy, A B A B, each one warm pass and one recorded pass.
set -eu
lab=/target/release/lab
corpus=/src/tinystore/bench/corpus/nomiracl
out=/perf/out
data=/src/tinystore/reports/data/search-plan-2026-10-16
cd /src/tinystore/search-plan-lab
cargo build --release --locked --bin lab
(cd "$corpus" && sha256sum -c --quiet /src/tinystore/search-plan-lab/corpus.sha256)
mkdir -p "$data" "$out"
rm -rf "$out"/*
{
  echo "{\"uname\": \"$(uname -sr)\", \"nproc\": $(nproc), \"cpu\": \"$(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | sed 's/^ //')\","
  echo " \"memory_kib\": $(grep MemTotal /proc/meminfo | tr -s ' ' | cut -d' ' -f2), \"rustc\": \"$(rustc --version)\","
  echo " \"cgroup_cpu\": \"$(cat /sys/fs/cgroup/cpu.max 2>/dev/null || echo none)\", \"cgroup_memory\": \"$(cat /sys/fs/cgroup/memory.max 2>/dev/null || echo none)\"}"
} > "$data/environment.json"

build() { "$lab" build "$corpus" "$out" "$@" >> "$data/builds.jsonl"; }
query() { "$lab" query "$out" "$@" >> "$data/queries.jsonl"; }
all=or,and,adaptive,pruned-or,adaptive-pruned
: > "$data/builds.jsonl"
: > "$data/queries.jsonl"

# 1. The plans, on FTS5 and on Tantivy, over the same tokens.
for cohort in ru en zh mixed; do
  build "$cohort" auto fts
  build "$cohort" auto tantivy
done
for round in 1 2; do
  for cohort in ru en zh mixed; do
    for backend in fts tantivy; do
      LAB_ROUND=$round query "$cohort" auto "$backend" "$all" 1 0.05 clean exact
    done
  done
done
# How many documents make a word too frequent to drive a query.
for round in 1 2; do
  for cut in 0.01 0.02 0.1 0.2; do
    LAB_ROUND=$round query mixed auto fts pruned-or,adaptive-pruned 1 "$cut" clean exact
  done
done

# 2. Scripts written without spaces: bigrams against ICU4X and against none.
for cohort in zh ja th; do
  for analysis in auto icu words; do
    [ -d "$out/$cohort-$analysis-fts" ] || build "$cohort" "$analysis" fts
  done
done
for round in 1 2; do
  for cohort in zh ja th; do
    for analysis in auto icu words; do
      LAB_ROUND=$round query "$cohort" "$analysis" fts or,pruned-or 1 0.05 clean exact
    done
  done
done

# 3. Stems: none, the script's default, the language's own, its accents kept.
for cohort in de fr es; do
  for analysis in surface auto "$cohort" "$cohort-nofold"; do
    build "$cohort" "$analysis" fts
  done
done
for cohort in ru en; do
  build "$cohort" surface fts
done
for round in 1 2; do
  for cohort in de fr es; do
    for analysis in surface auto "$cohort" "$cohort-nofold"; do
      LAB_ROUND=$round query "$cohort" "$analysis" fts or,pruned-or 1 0.05 clean exact
    done
  done
  for cohort in ru en; do
    LAB_ROUND=$round query "$cohort" surface fts or,pruned-or 1 0.05 clean exact
  done
done

# 4. Typos: a question with one, found again through the FST or not.
for round in 1 2; do
  for cohort in ru en mixed; do
    for variant in "clean exact" "typos exact" "typos fuzzy" "clean fuzzy"; do
      LAB_ROUND=$round query "$cohort" auto fts or,pruned-or 1 0.05 $variant
    done
  done
done

mkdir -p "$data/raw"
for file in "$out"/raw/*.jsonl; do
  gzip -c "$file" > "$data/raw/$(basename "$file").gz"
done
du -sh "$out"/* > "$data/index-dirs.txt"
