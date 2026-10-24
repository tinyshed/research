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
docker rm -f rssprobe-pg rssprobe-my rssprobe-hub >/dev/null 2>&1
docker run -d --name rssprobe-pg --network $NET --cpuset-cpus=12-13 -e POSTGRES_PASSWORD=probe postgres:18-alpine >/dev/null
docker run -d --name rssprobe-my --network $NET --cpuset-cpus=14-15 -e MYSQL_ROOT_PASSWORD=probe mysql:9 >/dev/null

echo "######## A. jemalloc tuned, no limit"
for cpus in 0,1 0; do
  hub "musl" $cpus musl ""
  hub "jemalloc $CUR" $cpus jemalloc "" -e MALLOC_CONF=$CUR
  hub "jemalloc $GPT" $cpus jemalloc "" -e MALLOC_CONF=$GPT
  hub "jemalloc $MIN" $cpus jemalloc "" -e MALLOC_CONF=$MIN
done

echo "######## B. the agent, 2 cpus"
until docker exec rssprobe-my mysqladmin -uroot -pprobe ping >/dev/null 2>&1; do sleep 2; done
sleep 5
docker rm -f rssprobe-hub >/dev/null 2>&1
docker run -d --name rssprobe-hub --network $NET --cpuset-cpus=2,3 -v rssprobe-target:/target:ro $IMG /target/out/musl/hub >/dev/null
sleep 3
agent() {
  local label=$1 dir=$2; shift 2
  echo "== agent $label"
  docker run --rm --network $NET --cpuset-cpus=0,1 -v rssprobe-target:/target:ro "$@" \
    -e HUB=ws://rssprobe-hub:8080/agent -e PG="host=rssprobe-pg user=postgres password=probe" -e MY="mysql://root:probe@rssprobe-my:3306" \
    $IMG /target/out/$dir/agent | sed 's/^/  /' | grep -E 'idle|speed|after 100|10 s later'
}
agent "musl" musl
agent "jemalloc $CUR" jemalloc -e MALLOC_CONF=$CUR
agent "jemalloc decay 0, arenas as they are" jemalloc -e MALLOC_CONF=background_thread:true,dirty_decay_ms:0,muzzy_decay_ms:0
agent "jemalloc $MIN" jemalloc -e MALLOC_CONF=$MIN
docker rm -f rssprobe-hub >/dev/null

echo "######## C. the hub under container limits, 2 cpus"
for mem in 32m 48m 64m 128m; do
  hub "jemalloc $CUR" 0,1 jemalloc $mem -e MALLOC_CONF=$CUR
  hub "jemalloc $GPT" 0,1 jemalloc $mem -e MALLOC_CONF=$GPT
done
hub "musl" 0,1 musl 32m

docker rm -f rssprobe-pg rssprobe-my >/dev/null
