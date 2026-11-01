#!/bin/sh
# Builds pass.cpp against the PISA checkout build.sh left on the
# search-msmarco-pisa volume, into /pisa/driver/pass. Run in gcc:13.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
export DEBIAN_FRONTEND=noninteractive
command -v cmake > /dev/null || { apt-get update -qq && apt-get install -y -qq cmake > /dev/null; }
mkdir -p /pisa/driver && cd /pisa/driver
cmake -DPISA_SOURCE=/pisa/src "$here" > /pisa/driver-cmake.log 2>&1 || { tail -40 /pisa/driver-cmake.log; exit 1; }
cmake --build . -j "${JOBS:-6}" --target pass > /pisa/driver-make.log 2>&1 || { tail -60 /pisa/driver-make.log; exit 1; }
ls -la /pisa/driver/pass
