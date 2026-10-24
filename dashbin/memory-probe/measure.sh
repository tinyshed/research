#!/usr/bin/env bash
# runs every probe in linux containers and prints VmRSS at each step
set -u
export MSYS_NO_PATHCONV=1
IMG=rust:1.99-bookworm
NET=rssprobe
run() { docker run "$@"; }
rss() { printf '%-44s %s\n' "$1" "$(docker exec "$2" curl -s localhost:8080/rss)"; }
load() { docker exec "$1" bash -c "seq $3 | xargs -P10 -I{} curl -s -o /dev/null localhost:8080/$2"; }

docker network create $NET >/dev/null 2>&1 || true
docker rm -f rssprobe-pg rssprobe-my rssprobe-bare rssprobe-hub >/dev/null 2>&1
run -d --name rssprobe-pg --network $NET -e POSTGRES_PASSWORD=probe postgres:18-alpine >/dev/null
run -d --name rssprobe-my --network $NET -e MYSQL_ROOT_PASSWORD=probe mysql:9 >/dev/null

echo "== bare axum"
run -d --name rssprobe-bare -v rssprobe-target:/target:ro $IMG /target/out/bare >/dev/null
sleep 3
rss "idle, 3 s after bind" rssprobe-bare
load rssprobe-bare "" 1000
rss "after 1000 requests, ten at once" rssprobe-bare
docker rm -f rssprobe-bare >/dev/null

hub() {
  local workers=$1
  echo "== hub: axum + tinystore::aio, TOKIO_WORKER_THREADS=${workers:-default}"
  docker rm -f rssprobe-hub >/dev/null 2>&1
  run -d --name rssprobe-hub --network $NET -v rssprobe-target:/target:ro ${workers:+-e TOKIO_WORKER_THREADS=$workers} $IMG /target/out/hub >/dev/null
  sleep 3
  rss "idle, engines open, 3 s after bind" rssprobe-hub
  docker exec rssprobe-hub curl -s localhost:8080/seed >/dev/null; sleep 1
  rss "after seeding 50 series x 1 h" rssprobe-hub
  docker exec rssprobe-hub curl -s localhost:8080/work >/dev/null
  rss "after one page request" rssprobe-hub
  load rssprobe-hub work 1000
  rss "after 1000 page requests, ten at once" rssprobe-hub
  load rssprobe-hub panel 1000
  rss "after 1000 panels, ten at once" rssprobe-hub
  sleep 30
  rss "30 s after the load" rssprobe-hub
  sleep 60
  rss "90 s after the load" rssprobe-hub
  printf '%-44s %s\n' "after malloc_trim" "$(docker exec rssprobe-hub curl -s localhost:8080/trim)"
}
hub ""

echo "== quickjs instances"
run --rm -v rssprobe-target:/target:ro $IMG /target/out/qjs

PLUGIN_DIR="$(cygpath -m "$PWD" 2>/dev/null || pwd)"
echo "== bun 1.4.2, plugin as its own process"
sleep 20 | run --rm -i -v "$PLUGIN_DIR:/p:ro" oven/bun:1.4.2-slim bun /p/plugin.js
echo "== node 24, plugin as its own process"
sleep 20 | run --rm -i -v "$PLUGIN_DIR:/p:ro" node:24-slim node --expose-gc /p/plugin.js

until docker exec rssprobe-my mysqladmin -uroot -pprobe ping >/dev/null 2>&1; do sleep 2; done
sleep 5
agent() {
  local bin=$1 workers=$2
  echo "== $bin, TOKIO_WORKER_THREADS=${workers:-default}"
  run --rm --network $NET -v rssprobe-target:/target:ro ${workers:+-e TOKIO_WORKER_THREADS=$workers} \
    -e HUB=ws://rssprobe-hub:8080/agent -e PG="host=rssprobe-pg user=postgres password=probe" -e MY="mysql://root:probe@rssprobe-my:3306" \
    $IMG /target/out/$bin
}
agent agent 2
agent agent-buffer 2

docker rm -f rssprobe-pg rssprobe-my rssprobe-hub >/dev/null
