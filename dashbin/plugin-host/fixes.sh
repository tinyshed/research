#!/usr/bin/env bash
# the memory round's fixes: the same hub, eight instances and 64 MiB, with quickjs in its own arena, gc after a call,
# and runtimes unloaded when idle; each run adds one fix to the one before
set -u
export MSYS_NO_PATHCONV=1
HERE="$(cygpath -m "$PWD" 2>/dev/null || pwd)"
SDK="$(cygpath -m "$PWD/../sdk-probe" 2>/dev/null || echo "$PWD/../sdk-probe")"
NET=plughost
IMG=rust:1.99-bookworm
TUNED=background_thread:true,narenas:2,dirty_decay_ms:1000,muzzy_decay_ms:0
docker network create $NET >/dev/null 2>&1 || true
docker rm -f plughost-stripe plughost-mock plughost-hub >/dev/null 2>&1
docker run -d --name plughost-stripe --network $NET --cpuset-cpus=12-13 stripe/stripe-mock:latest >/dev/null
docker run -d --name plughost-mock --network $NET --cpuset-cpus=14-15 -v "$SDK:/m:ro" oven/bun:1.4.2-slim bun /m/mock.mjs >/dev/null
sleep 3
# the first container to read a freshly built binary is charged its page cache; read it once outside them all
docker run --rm -v plughost-target:/target:ro -v "$HERE/plugins/dist:/p:ro" $IMG sh -c 'cat /target/out/plughost-hub /p/*.js > /dev/null'

alive() { [ "$(docker inspect -f '{{.State.Running}}' plughost-hub 2>/dev/null)" = "true" ]; }
died() { printf '  %-30s %s\n' "DIED" "$(docker inspect -f 'OOMKilled={{.State.OOMKilled}} exit={{.State.ExitCode}}' plughost-hub)"; }
ask() { docker exec plughost-hub curl -s "localhost:8080/$1"; }
show() { printf '  %-30s %s\n' "$1" "$(ask rss)"; printf '  %-30s %s\n' "" "$(ask js)"; }
cg() {
  printf '  %-30s %s\n' "cgroup" "$(docker exec plughost-hub sh -c 'echo "current $(($(cat /sys/fs/cgroup/memory.current)/1048576)) MiB  peak $(($(cat /sys/fs/cgroup/memory.peak)/1048576)) MiB  $(grep -E "^(high|max|oom_kill) " /sys/fs/cgroup/memory.events | tr "\n" " ")"')"
}
# a client whose hub died would wait on its socket forever; five minutes is far past any run here
load() {
  docker run --rm --network container:plughost-hub --cpuset-cpus=8-11 -v rssprobe-target:/target:ro $IMG \
    timeout 300 /target/out/tools/load "$@" 2>&1 | tail -1
}

run() {
  local label=$1 mem=$2 instances=$3 install_limit=$4; shift 4
  echo "== $label, limit $mem, install limit $install_limit MiB, $*"
  docker rm -f plughost-hub >/dev/null 2>&1
  docker run -d --name plughost-hub --network $NET --cpuset-cpus=0,1 --memory=$mem --memory-swap=$mem \
    -v plughost-target:/target:ro -v "$HERE/plugins/dist:/p:ro" \
    -e OUR_TIMERS=1 -e NO_LLRT_TIMERS=1 -e MALLOC_CONF=$TUNED -e INSTALL_LIMIT_MIB=$install_limit \
    -e STRIPE_MOCK=http://plughost-stripe:12111 -e SDK_MOCK=http://plughost-mock:8080 -e INSTANCES="$instances" "$@" \
    $IMG /target/out/plughost-hub serve >/dev/null
  sleep 5
  alive || { died; return; }
  show "started, idle"
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
  sleep 3
  show "3 s after the load"
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
run "eight instances, as before" 64m "$EIGHT" 8
run "eight instances, quickjs in its own arena" 64m "$EIGHT" 8 -e JS_ARENA=1
run "eight instances, own arena, gc after a call" 64m "$EIGHT" 8 -e JS_ARENA=1 -e GC_AFTER_CALL=1
run "eight instances, own arena, gc, unloaded after 2 s idle" 64m "$EIGHT" 8 -e JS_ARENA=1 -e GC_AFTER_CALL=1 -e IDLE_UNLOAD_MS=2000
run "eight instances, own arena, gc, unloaded after every call" 64m "$EIGHT" 8 -e JS_ARENA=1 -e GC_AFTER_CALL=1 -e IDLE_UNLOAD_MS=0
run "sixteen instances, own arena, gc, unloaded after every call" 64m "$SIXTEEN" 8 -e JS_ARENA=1 -e GC_AFTER_CALL=1 -e IDLE_UNLOAD_MS=0
docker rm -f plughost-stripe plughost-mock >/dev/null
