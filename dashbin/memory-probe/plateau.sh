#!/usr/bin/env bash
# does the hub's loaded RSS stop, or keep growing run over run; and how fast each js runtime is
set -u
export MSYS_NO_PATHCONV=1
IMG=rust:1.99-bookworm
rss() { printf '%-44s %s\n' "$1" "$(docker exec rssprobe-hub curl -s localhost:8080/rss)"; }
docker rm -f rssprobe-hub >/dev/null 2>&1
docker run -d --name rssprobe-hub -v rssprobe-target:/target:ro $IMG /target/out/hub >/dev/null
sleep 3
docker exec rssprobe-hub curl -s localhost:8080/seed >/dev/null
rss "seeded"
for round in 1 2 3 4 5 6; do
  docker exec rssprobe-hub bash -c "seq 1000 | xargs -P10 -I{} curl -s -o /dev/null localhost:8080/panel"
  rss "after round $round of 1000 panels"
done
docker exec rssprobe-hub bash -c "seq 1000 | xargs -P10 -I{} curl -s -o /dev/null localhost:8080/work"
rss "plus 1000 page requests"
docker rm -f rssprobe-hub >/dev/null

echo "== quickjs"
docker run --rm -v rssprobe-target:/target:ro $IMG /target/out/qjs | head -9
PLUGIN_DIR="$(cygpath -m "$PWD" 2>/dev/null || pwd)"
echo "== bun"
sleep 20 | docker run --rm -i -v "$PLUGIN_DIR:/p:ro" oven/bun:1.4.2-slim bun /p/plugin.js | grep took
echo "== node"
sleep 20 | docker run --rm -i -v "$PLUGIN_DIR:/p:ro" node:24-slim node --expose-gc /p/plugin.js | grep took
