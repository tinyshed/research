#!/bin/sh
# Prints every program's version, as the report names them.
set -eu
echo "kernel $(uname -r)"
echo "cpus $(nproc)"
echo "bun $(bun --version)"
echo "postgres $(postgres --version)"
echo "redis $(redis-server --version)"
echo "valkey $(valkey-server --version)"
echo "victoria-metrics $(victoria-metrics -version 2>&1 | head -1)"
echo "prometheus $(prometheus --version 2>&1 | head -1)"
echo "loki $(loki -version 2>&1 | head -1)"
echo "minio $(minio --version 2>&1 | head -1)"
echo "tinystore $(node_modules/@tinyshed/tinystore-linux-x64/bin/tinystore --version 2>&1 | head -1)"
echo "tinystore-npm $(grep -o '"@tinyshed/tinystore@[^"]*"' bun.lock | head -1)"
