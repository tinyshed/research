#!/usr/bin/env bash
# the memory round, parts 1-3: install limits against the process's peak, what real SDKs need, a spike inside a call;
# every line is a process of its own, so its peak is its own
set -u
export MSYS_NO_PATHCONV=1
HERE="$(cygpath -m "$PWD" 2>/dev/null || pwd)"
SDK="$(cygpath -m "$PWD/../sdk-probe" 2>/dev/null || echo "$PWD/../sdk-probe")"
NET=plughost
TUNED=background_thread:true,narenas:2,dirty_decay_ms:1000,muzzy_decay_ms:0
DECAY0=background_thread:true,narenas:2,dirty_decay_ms:0,muzzy_decay_ms:0
bash plugins/build.sh >/dev/null
bash build-hub.sh || exit 1
docker network create $NET >/dev/null 2>&1 || true
docker rm -f plughost-stripe plughost-mock >/dev/null 2>&1
docker run -d --name plughost-stripe --network $NET stripe/stripe-mock:latest >/dev/null
docker run -d --name plughost-mock --network $NET -v "$SDK:/m:ro" oven/bun:1.4.2-slim bun /m/mock.mjs >/dev/null
sleep 3
host() {
  local conf=$1; shift
  docker run --rm --network $NET -v plughost-target:/target:ro -v "$HERE/plugins/dist:/p:ro" \
    -e OUR_TIMERS=1 -e NO_LLRT_TIMERS=1 -e MALLOC_CONF=$conf \
    rust:1.99-bookworm timeout 60 /target/out/plughost-hub "$@" 2>&1
}
echo "######## 1. installs: a module that declares, and one that allocates until it cannot"
for conf in $TUNED $DECAY0; do
  echo "== MALLOC_CONF=$conf"
  for limit in 4 8 16 24; do
    echo "- a module that only declares, limit $limit MiB"; host $conf install /p/abort.js $limit
    echo "- the memory bomb, limit $limit MiB"; host $conf install /p/install/memory.js $limit
  done
done
echo "######## 2. what each SDK needs: its install and one call at the same limit"
STRIPE='{"key":"sk_test_123","api":"http://plughost-stripe:12111"}'
OPENAI='{"key":"sk-test","api":"http://plughost-mock:8080"}'
NOTION='{"key":"secret_test","api":"http://plughost-mock:8080"}'
OCTOKIT='{"api":"http://plughost-mock:8080"}'
for limit in 2 3 4 6 8 12 16; do
  echo "== limit $limit MiB"
  echo "- stripe";  host $TUNED needs /p/stripe.js charges "$STRIPE" $limit
  echo "- openai";  host $TUNED needs /p/openai.js reply "$OPENAI" $limit
  echo "- notion";  host $TUNED needs /p/notion.js pages "$NOTION" $limit
  echo "- octokit"; host $TUNED needs /p/octokit.js repo "$OCTOKIT" $limit
done
echo "######## 3. a read that allocates until its runtime runs out"
for conf in $TUNED $DECAY0; do
  echo "== MALLOC_CONF=$conf"
  for limit in 8 16 32; do
    echo "- limit $limit MiB"; host $conf spike /p/bomb.js $limit
  done
done
docker rm -f plughost-stripe plughost-mock >/dev/null
