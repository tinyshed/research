#!/bin/sh
set -eu
cd "$(dirname "$0")"
revision=$(cat source-revision)
if [ ! -d source ]; then
    if ! git -C ../source cat-file -e "$revision^{commit}" 2>/dev/null; then
        git -C ../source fetch origin rust
    fi
    git -C ../source worktree add --detach "$(pwd)/source" "$revision"
fi
test "$(git -C source rev-parse HEAD)" = "$revision"
test -z "$(git -C source status --porcelain)"
cargo build --release --locked
