#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
revision=$(cat source-revision)
if [ ! -d source ]; then
    git clone --no-checkout https://github.com/tinyshed/tinystore.git source
fi
git -C source fetch origin experiment/wal-load
git -C source checkout --detach "$revision"
