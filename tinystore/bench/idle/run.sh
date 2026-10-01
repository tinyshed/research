#!/bin/sh
set -eu
out="${OUT:?}"
mkdir -p "$out" /tmp/idle-bin
for case in empty root kv sqldb jobs blobs records metrics all; do
  go build -trimpath -ldflags='-s -w' -o "/tmp/idle-bin/$case" "./cmd/$case"
  go list -deps "./cmd/$case" > "$out/$case-deps.txt"
done
{
  date -u +%Y-%m-%dT%H:%M:%SZ
  go version
  uname -r
  nproc
  grep -m1 'model name' /proc/cpuinfo
  printf 'source %s\nharness %s\n' "$TINYSTORE_COMMIT" "$HARNESS_COMMIT"
} > "$out/environment.txt"
python3 run.py --bin /tmp/idle-bin --data /data --out "$out/idle.json" --repeats "${REPEATS:-5}"
