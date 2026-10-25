#!/usr/bin/env bash
# the memory round, part 4: the hub and its plugin instances in one process, under a container limit, under load
set -u
export MSYS_NO_PATHCONV=1
HERE="$(cygpath -m "$PWD" 2>/dev/null || pwd)"
SDK="$(cygpath -m "$PWD/../sdk-probe" 2>/dev/null || echo "$PWD/../sdk-probe")"
NET=plughost
IMG=rust:1.99-bookworm
TUNED=background_thread:true,narenas:2,dirty_decay_ms:1000,muzzy_decay_ms:0
DECAY0=background_thread:true,narenas:2,dirty_decay_ms:0,muzzy_decay_ms:0
docker network create $NET >/dev/null 2>&1 || true
docker rm -f plughost-stripe plughost-mock plughost-hub >/dev/null 2>&1
docker run -d --name plughost-stripe --network $NET --cpuset-cpus=12-13 stripe/stripe-mock:latest >/dev/null
docker run -d --name plughost-mock --network $NET --cpuset-cpus=14-15 -v "$SDK:/m:ro" oven/bun:1.4.2-slim bun /m/mock.mjs >/dev/null
sleep 3

alive() { [ "$(docker inspect -f '{{.State.Running}}' plughost-hub 2>/dev/null)" = "true" ]; }
died() { printf '  %-30s %s\n' "DIED" "$(docker inspect -f 'OOMKilled={{.State.OOMKilled}} exit={{.State.ExitCode}}' plughost-hub)"; }
ask() { docker exec plughost-hub curl -s "localhost:8080/$1"; }
show() { printf '  %-30s %s\n' "$1" "$(ask rss)"; printf '  %-30s %s\n' "" "$(ask js)"; }
cg() {
  printf '  %-30s %s\n' "cgroup" "$(docker exec plughost-hub sh -c 'echo "current $(($(cat /sys/fs/cgroup/memory.current)/1048576)) MiB  peak $(($(cat /sys/fs/cgroup/memory.peak)/1048576)) MiB  $(grep -E "^(high|max|oom_kill) " /sys/fs/cgroup/memory.events | tr "\n" " ")"')"
}
load() {
  docker run --rm --network container:plughost-hub --cpuset-cpus=8-11 -v rssprobe-target:/target:ro $IMG /target/out/tools/load "$@" 2>&1 | tail -1
}

run() {
  local label=$1 mem=$2 instances=$3 install_limit=$4 conf=${5:-$TUNED}
  echo "== $label${mem:+, limit $mem}, install limit $install_limit MiB, MALLOC_CONF=$conf"
  docker rm -f plughost-hub >/dev/null 2>&1
  docker run -d --name plughost-hub --network $NET --cpuset-cpus=0,1 ${mem:+--memory=$mem --memory-swap=$mem} \
    -v plughost-target:/target:ro -v "$HERE/plugins/dist:/p:ro" \
    -e OUR_TIMERS=1 -e NO_LLRT_TIMERS=1 -e MALLOC_CONF=$conf -e INSTALL_LIMIT_MIB=$install_limit \
    -e STRIPE_MOCK=http://plughost-stripe:12111 -e SDK_MOCK=http://plughost-mock:8080 -e INSTANCES="$instances" \
    $IMG /target/out/plughost-hub serve >/dev/null
  sleep 4
  alive || { died; return; }
  show "loaded, idle"
  ask seed >/dev/null
  local names; names=$(echo "$instances" | tr ',' '\n' | cut -d= -f1)
  for n in $names; do ask "plugin/$n" >/dev/null; done
  show "one call each"
  cg
  echo "  load, all at once:"
  {
    echo "    work 10000:  $(load work 10000 10)" &
    echo "    panel 3000:  $(load panel 3000 10)" &
    for n in $names; do echo "    plugin/$n 400: $(load plugin/$n 400 2)" & done
    wait
  } | sort
  alive || { died; return; }
  show "after the load"
  cg
  printf '  %-30s %s\n' "the memory bomb installed" "$(ask install/memory)"
  alive || { died; return; }
  printf '  %-30s %s\n' "a real plugin installed" "$(ask install/stripe)"
  cg
  sleep 30
  show "30 s later"
  docker rm -f plughost-hub >/dev/null
}

EIGHT=stripe-1=/p/stripe.js,stripe-2=/p/stripe.js,openai-1=/p/openai.js,openai-2=/p/openai.js,notion-1=/p/notion.js,notion-2=/p/notion.js,octokit-1=/p/octokit.js,octokit-2=/p/octokit.js
SIXTEEN=$EIGHT,stripe-3=/p/stripe.js,stripe-4=/p/stripe.js,openai-3=/p/openai.js,openai-4=/p/openai.js,notion-3=/p/notion.js,notion-4=/p/notion.js,octokit-3=/p/octokit.js,octokit-4=/p/octokit.js
run "no plugins" 64m "" 8
run "eight instances, no limit" "" "$EIGHT" 8
run "eight instances" 64m "$EIGHT" 8
run "eight instances, jemalloc returning pages at once" 64m "$EIGHT" 8 $DECAY0
run "sixteen instances" 64m "$SIXTEEN" 8
docker rm -f plughost-stripe plughost-mock >/dev/null
