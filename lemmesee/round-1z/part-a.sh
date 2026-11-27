#!/bin/sh
# Round 1z, part A after the fits: thresholds, the freeze, the clean sets'
# candidates, each set's one reading, and the timing on one processor.
#
#     sh round-1z/part-a.sh <lock folder> <repo, as docker sees it> [external]
#
# Run from lemmesee/. Each step that uses the card holds gpu.lock for that
# step alone; the build of pairs holds pairs.lock. `external` adds the kept
# pairs of Spider, BIRD, CTU and LiveSQLBench. The timing runs last, with
# nothing else of the round running.
set -e
L=$1; REPO=$2; EXT=$3
P=round-1b/.venv/Scripts/python
T=corpus/round-1z/.venv/Scripts/python
NETS="char-s granite"
SETS="style3 plain-a"

$P -I round-1z/read.py threshold $NETS
$P -I round-1z/read.py freeze $NETS
for s in $SETS; do
  sh round-1z/with-lock.sh "$L" pairs $P -I round-1z/read.py candidates $s
done
if [ "$EXT" = external ]; then SETS="$SETS spider bird ctu livesqlbench-development"; fi
for n in $NETS; do
  for s in $SETS; do
    sh round-1z/with-lock.sh "$L" gpu $T -I round-1z/net.py score $n corpus/round-1z/sets/$s.pkl corpus/round-1z/nets/$n/scores/$s.npy
  done
done
for s in $SETS; do
  $P -I round-1z/read.py read $s
done
for n in $NETS; do
  MSYS_NO_PATHCONV=1 docker run --rm --cpuset-cpus 3 -v "$REPO/lemmesee:/src/lemmesee" -v lemmesee-1z-py:/venv \
    -e HF_HOME=/venv/hf -w /src/lemmesee python:3.13-slim /venv/bin/python -I round-1z/net.py time $n \
    corpus/round-1z/nets/$n/time.json corpus/round-1z/sets/spider.pkl corpus/round-1z/sets/bird.pkl corpus/round-1z/sets/ctu.pkl \
    > corpus/round-1z/logs/time-$n.log 2>&1
  tail -1 corpus/round-1z/logs/time-$n.log
done
for n in $NETS; do
  $T -I round-1z/net.py quantise $n > corpus/round-1z/logs/quantise-$n.log 2>&1 || echo "quantise $n failed"
done
