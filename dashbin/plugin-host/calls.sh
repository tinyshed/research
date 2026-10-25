#!/usr/bin/env bash
# chapter 6 against the host: install limits, the example plugin on stripe-mock, and requests a call leaves behind
set -u
export MSYS_NO_PATHCONV=1
HERE="$(cygpath -m "$PWD" 2>/dev/null || pwd)"
NET=plughost
bash plugins/build.sh >/dev/null
docker run --rm -v "$HERE:/src" -v rssprobe-cargo:/usr/local/cargo/registry -v plughost-target:/target \
  -e CARGO_TARGET_DIR=/target -e CC_x86_64_unknown_linux_musl=musl-gcc -e CXX_x86_64_unknown_linux_musl=g++ \
  -w /src rssprobe-musl bash -c 'T=x86_64-unknown-linux-musl; cargo build -q --release --target $T \
  && mkdir -p /target/out && cp /target/$T/release/plughost /target/out/plughost-calls' || exit 1
docker network create $NET >/dev/null 2>&1 || true
docker rm -f plughost-stripe >/dev/null 2>&1
docker run -d --name plughost-stripe --network $NET stripe/stripe-mock:latest >/dev/null
sleep 3
host() {
  docker run --rm --network $NET -v plughost-target:/target:ro -v "$HERE/plugins/dist:/p:ro" \
    -e OUR_TIMERS=1 -e NO_LLRT_TIMERS=1 \
    -e MALLOC_CONF=background_thread:true,narenas:2,dirty_decay_ms:1000,muzzy_decay_ms:0 \
    rust:1.99-bookworm timeout 120 /target/out/plughost-calls "$@" 2>&1
}
echo "== install under limits"
host discover /p/install/loop.js /p/install/recursion.js /p/install/memory.js /p/install/fetch.js \
  /p/install/hang.js /p/install/nothing.js /p/stripe.js /p/abort.js
echo "== the example on stripe-mock"
host stripe /p/stripe.js http://plughost-stripe:12111
echo "== requests a call leaves, each case in a process of its own"
for case in waits signal leaves retries sdk-like-no-timer sdk-like stripe; do
  host abort $case /p/abort.js /p/stripe.js http://plughost-stripe:12111
  echo "  exit $?"
done
docker image inspect stripe/stripe-mock:latest --format '{{index .RepoDigests 0}}'
docker rm -f plughost-stripe >/dev/null
