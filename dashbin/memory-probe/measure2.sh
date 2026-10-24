#!/usr/bin/env bash
# every allocator through the same steps: hub under load, quickjs unload, agent rounds
set -u
export MSYS_NO_PATHCONV=1
IMG=rust:1.99-bookworm
NET=rssprobe
rss() { printf '  %-42s %s\n' "$1" "$(docker exec rssprobe-hub curl -s localhost:8080/$2)"; }
load() { printf '  %-42s %s\n' "speed: $1 $2 x10 conns" "$(docker exec rssprobe-hub /target/out/tools/load $1 $2 10)"; }

docker network create $NET >/dev/null 2>&1 || true
docker rm -f rssprobe-pg rssprobe-my rssprobe-hub >/dev/null 2>&1
docker run -d --name rssprobe-pg --network $NET -e POSTGRES_PASSWORD=probe postgres:18-alpine >/dev/null
docker run -d --name rssprobe-my --network $NET -e MYSQL_ROOT_PASSWORD=probe mysql:9 >/dev/null
until docker exec rssprobe-my mysqladmin -uroot -pprobe ping >/dev/null 2>&1; do sleep 2; done
sleep 5

variant() {
  local name=$1 dir=$2; shift 2
  echo "==================== $name"
  echo "-- hub"
  docker rm -f rssprobe-hub >/dev/null 2>&1
  docker run -d --name rssprobe-hub --network $NET -v rssprobe-target:/target:ro "$@" $IMG /target/out/$dir/hub >/dev/null
  sleep 3
  rss "idle, engines open, 3 s after bind" rss
  docker exec rssprobe-hub curl -s localhost:8080/seed >/dev/null; sleep 1
  rss "after seeding 50 series x 1 h" rss
  docker exec rssprobe-hub /target/out/tools/load work 2000 10 >/dev/null
  load work 10000
  rss "after 12000 page requests" rss
  load panel 3000
  rss "after 3000 panels" rss
  load panel 3000
  rss "after 6000 panels" rss
  sleep 30
  rss "30 s after the load" rss
  sleep 60
  rss "90 s after the load" rss
  rss "forced" trim
  echo "-- quickjs"
  docker run --rm -v rssprobe-target:/target:ro "$@" $IMG /target/out/$dir/qjs | sed 's/^/  /' | grep -v allocator
  echo "-- agent, TOKIO_WORKER_THREADS=2"
  docker run --rm --network $NET -v rssprobe-target:/target:ro -e TOKIO_WORKER_THREADS=2 "$@" \
    -e HUB=ws://rssprobe-hub:8080/agent -e PG="host=rssprobe-pg user=postgres password=probe" -e MY="mysql://root:probe@rssprobe-my:3306" \
    $IMG /target/out/$dir/agent | sed 's/^/  /' | grep -v allocator
  docker rm -f rssprobe-hub >/dev/null
}

variant "glibc (dynamic)" glibc
variant "musl static, its own mallocng" musl
variant "musl static + mimalloc" mimalloc
variant "musl static + jemalloc" jemalloc
variant "musl static + jemalloc, background_thread:true" jemalloc -e MALLOC_CONF=background_thread:true -e _RJEM_MALLOC_CONF=background_thread:true

docker rm -f rssprobe-pg rssprobe-my >/dev/null
