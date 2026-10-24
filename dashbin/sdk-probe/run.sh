#!/usr/bin/env bash
# every SDK bundle in LLRT (QuickJS + web APIs in Rust), web and node builds, then in Bun as a control
set -u
export MSYS_NO_PATHCONV=1
HERE="$(cygpath -m "$PWD" 2>/dev/null || pwd)"
NET=rssprobe
docker network create $NET >/dev/null 2>&1 || true
docker rm -f rssprobe-stripe rssprobe-mock rssprobe-pg >/dev/null 2>&1
docker run -d --name rssprobe-stripe --network $NET stripe/stripe-mock:latest >/dev/null
docker run -d --name rssprobe-mock --network $NET -v "$HERE:/p:ro" oven/bun:1.4.2-slim bun /p/mock.mjs >/dev/null
docker run -d --name rssprobe-pg --network $NET -e POSTGRES_PASSWORD=probe postgres:18-alpine >/dev/null
sleep 6

one() {
  local runtime=$1 build=$2 t=$3 out
  if [ "$runtime" = llrt ]; then
    out=$(docker run --rm --network $NET -v "$HERE:/p:ro" rust:1.99-bookworm timeout 30 /p/llrt/llrt /p/dist/$build/$t.mjs 2>&1)
  else
    out=$(docker run --rm --network $NET -v "$HERE:/p:ro" oven/bun:1.4.2-slim timeout 30 bun /p/dist/$build/$t.mjs 2>&1)
  fi
  printf '%-5s %-5s %-8s %s\n' "$runtime" "$build" "$t" "$(echo "$out" | tr '\n' ' ' | cut -c1-330)"
}

for t in empty stripe octokit openai notion slack libs; do one llrt web $t; done
for t in empty stripe octokit openai notion slack pg libs; do one llrt node $t; done
for t in empty stripe octokit openai notion slack libs; do one bun web $t; done
one bun node pg

docker rm -f rssprobe-stripe rssprobe-mock rssprobe-pg >/dev/null
