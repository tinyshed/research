#!/bin/sh
# Drops an index's pages from the page cache, the Linux VM's and so every
# container's: `evict.sh <arm>`. A clean page goes on POSIX_FADV_DONTNEED,
# which `dd iflag=nocache count=0` asks for a whole file; sync first, so that
# none is dirty. Prints the bytes of the arm's files in the cache before and
# after, by fincore. Run in gcc:13, which has fincore, with the volume at /perf.
set -eu
out=/perf/msmarco
case "$1" in
compact) files=$(ls $out/compact.db*) ;;
fts5) files=$(ls $out/fts5.db*) ;;
tantivy) files=$(ls $out/tantivy/*) ;;
pisa*) files="$out/pisa.block_simdbp.idx $out/pisa.bm25.bmw" ;;
lucene) files=$(ls $out/lucene/*) ;;
*) echo "no arm $1" >&2; exit 2 ;;
esac
cached() {
  total=0
  for file in $files; do
    bytes=$(fincore -b -n -o RES "$file" | awk '{ print $1 }')
    total=$((total + ${bytes:-0}))
  done
  echo "$total"
}
before=$(cached)
sync
for file in $files; do
  dd if="$file" iflag=nocache count=0 status=none
done
printf '{"evict":"%s","cached_before":%s,"cached_after":%s}\n' "$1" "$before" "$(cached)"
