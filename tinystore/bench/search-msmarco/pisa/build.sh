#!/bin/sh
# Builds PISA's tools at one release tag inside gcc:13, the image PISA's own
# CI builds in, into /pisa on the search-msmarco-pisa volume. CMake comes from
# Debian's own repository; Boost and every other dependency from PISA's
# submodules at the commits the tag pins. Run with the proxy, if any, in
# http_proxy and https_proxy.
set -eu
tag=v0.10.0
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq && apt-get install -y -qq cmake git > /dev/null
cmake --version | head -1
if [ ! -d /pisa/src/.git ]; then
  git -c http.proxy="${https_proxy:-}" clone --quiet --branch "$tag" --depth 1 \
    https://github.com/pisa-engine/pisa.git /pisa/src
fi
cd /pisa/src
git -c http.proxy="${https_proxy:-}" submodule update --init --recursive --depth 1 --quiet \
  || git -c http.proxy="${https_proxy:-}" submodule update --init --recursive --quiet
echo "pisa $(git rev-parse HEAD) $(git describe --tags)"
mkdir -p /pisa/build && cd /pisa/build
cmake -DCMAKE_BUILD_TYPE=Release -DPISA_BUILD_TOOLS=ON -DPISA_ENABLE_TESTING=OFF \
  -DPISA_ENABLE_BENCHMARKING=OFF /pisa/src > /pisa/cmake.log 2>&1 || { tail -40 /pisa/cmake.log; exit 1; }
cmake --build . -j "${JOBS:-6}" --target compress_inverted_index create_wand_data \
  evaluate_queries queries lexicon > /pisa/make.log 2>&1 || { tail -60 /pisa/make.log; exit 1; }
ls -la /pisa/build/bin
