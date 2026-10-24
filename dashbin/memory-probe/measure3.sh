#!/usr/bin/env bash
# musl against jemalloc on a small vps: the probes pinned to one or two cpus, the load client on others
set -u
export MSYS_NO_PATHCONV=1
IMG=rust:1.99-bookworm
NET=rssprobe
rss() { printf '  %-42s %s\n' "$1" "$(docker exec rssprobe-hub curl -s localhost:8080/$2)"; }
load() {
  printf '  %-42s %s\n' "speed: $1 $2 x10 conns" \
    "$(docker run --rm --network container:rssprobe-hub --cpuset-cpus=8-11 -v rssprobe-target:/target:ro $IMG /target/out/tools/load $1 $2 10)"
}

docker network create $NET >/dev/null 2>&1 || true
docker rm -f rssprobe-pg rssprobe-my rssprobe-hub >/dev/null 2>&1
docker run -d --name rssprobe-pg --network $NET --cpuset-cpus=12-13 -e POSTGRES_PASSWORD=probe postgres:18-alpine >/dev/null
docker run -d --name rssprobe-my --network $NET --cpuset-cpus=14-15 -e MYSQL_ROOT_PASSWORD=probe mysql:9 >/dev/null
until docker exec rssprobe-my mysqladmin -uroot -pprobe ping >/dev/null 2>&1; do sleep 2; done
sleep 5

variant() {
  local name=$1 cpus=$2 dir=$3; shift 3
  echo "==================== $name, cpuset $cpus"
  docker rm -f rssprobe-hub >/dev/null 2>&1
  docker run -d --name rssprobe-hub --network $NET --cpuset-cpus=$cpus -v rssprobe-target:/target:ro "$@" $IMG /target/out/$dir/hub >/dev/null
  sleep 3
  printf '  %-42s %s\n' "nproc inside" "$(docker exec rssprobe-hub nproc)"
  rss "idle, engines open, 3 s after bind" rss
  docker exec rssprobe-hub curl -s localhost:8080/seed >/dev/null; sleep 1
  docker run --rm --network container:rssprobe-hub --cpuset-cpus=8-11 -v rssprobe-target:/target:ro $IMG /target/out/tools/load work 2000 10 >/dev/null
  load work 10000
  rss "after 12000 page requests" rss
  load panel 3000
  load panel 3000
  rss "after 6000 panels" rss
  sleep 30
  rss "30 s after the load" rss
  sleep 30
  rss "60 s after the load" rss
  echo "-- quickjs"
  docker run --rm --cpuset-cpus=$cpus -v rssprobe-target:/target:ro "$@" $IMG /target/out/$dir/qjs | sed 's/^/  /' | grep -E 'speed|100 instances|unloaded|forced'
  echo "-- agent"
  docker run --rm --network $NET --cpuset-cpus=$cpus -v rssprobe-target:/target:ro "$@" \
    -e HUB=ws://rssprobe-hub:8080/agent -e PG="host=rssprobe-pg user=postgres password=probe" -e MY="mysql://root:probe@rssprobe-my:3306" \
    $IMG /target/out/$dir/agent | sed 's/^/  /' | grep -E 'idle|speed|after 100|10 s later'
  docker rm -f rssprobe-hub >/dev/null
}

BG="-e MALLOC_CONF=background_thread:true"
variant "musl" 0,1 musl
variant "jemalloc, background_thread" 0,1 jemalloc $BG
variant "musl" 0 musl
variant "jemalloc, background_thread" 0 jemalloc $BG

docker rm -f rssprobe-pg rssprobe-my >/dev/null
