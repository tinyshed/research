#!/bin/sh
# Builds PISA's index of a prepared collection <out>: the binary collection
# prepare wrote compressed as block_simdbp, WAND data of fixed blocks of 64
# postings, PISA's default, scored by BM25 with k1 1.2 and b 0.75, and the
# term and document lexicons. Run in gcc:13 with the search-msmarco-pisa
# volume at /pisa.
set -eu
out=$1
bin=/pisa/build/bin
cd "$out"
quiet() { tr '\r' '\n' | grep -v '%' | tail -2; }
started=$(date +%s.%N)
"$bin/compress_inverted_index" -e block_simdbp -c msmarco -o pisa.block_simdbp.idx 2>&1 | quiet
compressed=$(date +%s.%N)
"$bin/create_wand_data" -c msmarco -o pisa.bm25.bmw -b 64 -s bm25 --bm25-k1 1.2 --bm25-b 0.75 2>&1 | quiet
wand=$(date +%s.%N)
"$bin/lexicon" build msmarco.terms pisa.termlex 2>&1 | quiet
"$bin/lexicon" build msmarco.documents pisa.doclex 2>&1 | quiet
printf '{"step":"build","engine":"pisa","compress_seconds":%s,"wand_seconds":%s,"index_bytes":%s,"wand_bytes":%s,"termlex_bytes":%s,"doclex_bytes":%s,"sizes_bytes":%s}\n' \
  "$(awk -v a="$started" -v b="$compressed" 'BEGIN { print b - a }')" \
  "$(awk -v a="$compressed" -v b="$wand" 'BEGIN { print b - a }')" \
  "$(stat -c %s pisa.block_simdbp.idx)" "$(stat -c %s pisa.bm25.bmw)" \
  "$(stat -c %s pisa.termlex)" "$(stat -c %s pisa.doclex)" "$(stat -c %s msmarco.sizes)"
