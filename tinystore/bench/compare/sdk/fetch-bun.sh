#!/bin/sh
# Fetches the Bun this round pins into ./bun, where GitHub is reachable, for
# Dockerfile to copy; a machine that cannot reach it is sent the file.
set -eu
version=1.4.2
curl -fsSL -o /tmp/bun.zip "https://github.com/oven-sh/bun/releases/download/bun-v$version/bun-linux-x64.zip"
unzip -o -j /tmp/bun.zip bun-linux-x64/bun -d "$(dirname "$0")"
"$(dirname "$0")/bun" --version
