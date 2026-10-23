#!/bin/sh
# Runs the round inside the image: the harness copied onto the volume, so that
# neither Bun's modules nor a store sit on the host's shared folder, then
# run.py with the caller's environment. Results go to /results.
#
#   docker run --rm --init --privileged -v <release-bench>:/src:ro -v <results>:/results \
#     -v tinystore-release:/work -e ROUNDS=idle -e OUT=/results/<name> tinystore-release-bench sh /src/go.sh
set -eu
rm -rf "/work/bench" "/work/runs"
mkdir -p /work/bench
cd /src
cp -r Dockerfile go.sh package.json bun.lock run.py services.py versions.sh peek.py bench /work/bench/
cd /work/bench
bun install --frozen-lockfile > /dev/null
exec python3 run.py
