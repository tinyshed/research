#!/bin/sh
# each variant built in turn in the same container image and target volume as dashbin's `just size`; $1 is a
# scratch folder holding weight-tree, a detached worktree of dashbin at the commit weighed
S="$1"
here="$(cd "$(dirname "$0")" && pwd)"
tree="$S/weight-tree"
for variant in baseline hub_ws agent_plain agent_ring_webpki agent_ring_native agent_awslc_webpki hub_and_agent; do
    git -C "$tree" checkout -q -- .
    rm -f "$tree/crates/cli/src/weight.rs"
    if [ "$variant" != baseline ]; then python "$here/apply.py" "$tree" "$variant" >/dev/null; fi
    started=$(date +%s)
    line=$(MSYS_NO_PATHCONV=1 docker run --rm -v "$(cygpath -w "$tree"):/src" -v "$(cygpath -w "$here"):/variants" \
        -v dashbin-rustup:/usr/local/rustup -v dashbin-cargo:/usr/local/cargo/registry -v dashbin-weight-target:/target \
        -e CARGO_TARGET_DIR=/target -w /src dashbin-musl:1.99.1 sh /variants/build.sh 2>&1 | tail -8)
    echo "$variant: $line ($(( $(date +%s) - started )) s)"
done
git -C "$tree" checkout -q -- .
rm -f "$tree/crates/cli/src/weight.rs"
