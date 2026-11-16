#!/bin/sh
# One platform's passes of dir-scan: for each fixture, generated from SEED
# under ROOT, a bare walk, a first scan into an empty list, a scan that finds
# nothing (the sorted table and, interleaved, the sort in memory), a scan that
# finds 1 % changed, and the list's division by b-tree. With COLD=1 (Linux,
# a privileged container) the same passes again, each after the kernel drops
# its page cache, dentries and inodes. One JSON line a pass on stdout.
#
#   BIN=<binary> ROOT=<a folder on the file system measured> COLD=0|1 sh run.sh
set -eu
seed=${SEED:-20261116}
fixtures=${FIXTURES:-"lib10k:lib:10000 lib100k:lib:100000 flat100k:flat:100000 lib1m:lib:1000000"}
passes=${PASSES:-7}
writes=${WRITE_PASSES:-5}
cold=${COLD:-0}

# drops the caches in the cold passes alone; $cache is the pass's
drop() {
  if [ "$cache" = cold ]; then sync; echo 3 > /proc/sys/vm/drop_caches; fi
}
stamp() {
  printf '{"op":"step","fixture":"%s","step":"%s","at":"%s"}\n' "$1" "$2" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
fresh_list() {
  rm -f "$1" "$1-wal" "$1-shm"
}

"$BIN" info
for fixture in $fixtures; do
  name=${fixture%%:*}
  shape=${fixture#*:}
  tree=$ROOT/$name
  list=$ROOT/$name.db
  "$BIN" remove "$tree"
  fresh_list "$list"
  stamp "$name" gen
  "$BIN" gen "$tree" "$shape" "$seed"
  for cache in warm cold; do
    if [ "$cache" = cold ] && [ "$cold" != 1 ]; then continue; fi
    stamp "$name" "$cache"
    if [ "$cache" = warm ]; then "$BIN" walk "$tree" "$name/warm/warm-up"; fi
    i=0; while [ $i -lt "$passes" ]; do
      drop; "$BIN" walk "$tree" "$name/$cache/walk"; i=$((i + 1))
    done
    i=0; while [ $i -lt "$writes" ]; do
      fresh_list "$list"; drop; "$BIN" scan "$tree" "$list" "$name/$cache/first"; i=$((i + 1))
    done
    i=0; while [ $i -lt "$passes" ]; do
      drop; "$BIN" scan "$tree" "$list" "$name/$cache/unchanged" table
      drop; "$BIN" scan "$tree" "$list" "$name/$cache/unchanged" memory
      i=$((i + 1))
    done
    i=0; while [ $i -lt "$writes" ]; do
      if [ "$cache" = warm ]; then touch_seed=$((i + 1)); else touch_seed=$((i + 101)); fi
      "$BIN" touch "$tree" 10 "$touch_seed"
      drop; "$BIN" scan "$tree" "$list" "$name/$cache/changed"; i=$((i + 1))
    done
    if [ "$cache" = warm ]; then "$BIN" size "$list" "$name"; fi
  done
  stamp "$name" remove
  "$BIN" remove "$tree"
  fresh_list "$list"
done
