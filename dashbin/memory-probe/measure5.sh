#!/usr/bin/env bash
# jemalloc tuned three ways against musl; the agent on a squeezed jemalloc; the hub under container limits
set -u
export MSYS_NO_PATHCONV=1
IMG=rust:1.99-bookworm
NET=rssprobe
CUR="background_thread:true"
GPT="background_thread:true,narenas:2,dirty_decay_ms:1000,muzzy_decay_ms:0"
MIN="background_thread:true,narenas:1,dirty_decay_ms:0,muzzy_decay_ms:0"

alive() { [ "$(docker inspect -f '{{.State.Running}}' rssprobe-hub 2>/dev/null)" = "true" ]; }
died() { printf '  %-36s %s\n' "DIED" "$(docker inspect -f 'OOMKilled={{.State.OOMKilled}} exit={{.State.ExitCode}}' rssprobe-hub)"; }
rss() { printf '  %-36s %s\n' "$1" "$(docker exec rssprobe-hub curl -s localhost:8080/rss)"; }
cg() {
  printf '  %-36s %s\n' "cgroup" "$(docker exec rssprobe-hub sh -c 'echo "current $(($(cat /sys/fs/cgroup/memory.current)/1048576)) MiB  peak $(($(cat /sys/fs/cgroup/memory.peak)/1048576)) MiB  $(grep -E "^(high|max|oom_kill) " /sys/fs/cgroup/memory.events | tr "\n" " ")"')"
}
load() {
  local out
  out=$(docker run --rm --network container:rssprobe-hub --cpuset-cpus=8-11 -v rssprobe-target:/target:ro $IMG /target/out/tools/load $1 $2 10 2>&1 | tail -1)
  printf '  %-36s %s\n' "speed: $1 $2" "$out"
}

hub() {
  local label=$1 cpus=$2 dir=$3 mem=$4; shift 4
  echo "== hub $label, cpus $cpus${mem:+, limit $mem}"
  docker rm -f rssprobe-hub >/dev/null 2>&1
  docker run -d --name rssprobe-hub --network $NET --cpuset-cpus=$cpus ${mem:+--memory=$mem --memory-swap=$mem} \
    -v rssprobe-target:/target:ro "$@" $IMG /target/out/$dir/hub >/dev/null
  sleep 3
  alive || { died; return; }
  rss "idle"
  docker exec rssprobe-hub curl -s localhost:8080/seed >/dev/null; sleep 1
  docker run --rm --network container:rssprobe-hub --cpuset-cpus=8-11 -v rssprobe-target:/target:ro $IMG /target/out/tools/load work 2000 10 >/dev/null 2>&1
  load work 10000; alive || { died; return; }
  rss "after 12000 pages"
  load panel 3000; load panel 3000; alive || { died; return; }
  rss "after 6000 panels"
  cg
  sleep 30
  rss "30 s after"
  docker rm -f rssprobe-hub >/dev/null
}

docker network create $NET >/dev/null 2>&1 || true
DECAY="background_thread:true,dirty_decay_ms:1000,muzzy_decay_ms:0"
echo "######## D. which half of the tuning cut the peak, and what two arenas cost on 8 cores"
hub "jemalloc $DECAY" 0,1 jemalloc "" -e MALLOC_CONF=$DECAY
hub "jemalloc $DECAY" 0,1 jemalloc 32m -e MALLOC_CONF=$DECAY
for conf in "$CUR" "$GPT" "$DECAY"; do
  hub "jemalloc $conf" 0-7 jemalloc "" -e MALLOC_CONF=$conf
done
hub "musl" 0-7 musl ""
