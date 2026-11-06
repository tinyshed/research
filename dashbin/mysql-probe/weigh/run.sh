#!/bin/sh
# each variant built in turn in the same container image and target volume; $1 holds weight-tree, a detached
# worktree of dashbin, and weight-variants, this folder
S="$1"
tree="$S/weight-tree"
for variant in baseline mysql_async mysql_async_tls sqlx; do
    git -C "$tree" checkout -q -- .
    rm -f "$tree/crates/cli/src/weight.rs"
    if [ "$variant" != baseline ]; then python "$S/weight-variants/apply.py" "$tree" "$variant"; fi
    started=$(date +%s)
    line=$(MSYS_NO_PATHCONV=1 docker run --rm -v "$(cygpath -w "$tree"):/src" -v "$(cygpath -w "$S/weight-variants"):/variants" \
        -v dashbin-rustup:/usr/local/rustup -v dashbin-cargo:/usr/local/cargo/registry -v dashbin-weight-target:/target \
        -e CARGO_TARGET_DIR=/target -w /src dashbin-musl:1.99.1 sh /variants/build.sh 2>&1 | tail -8)
    echo "$variant: $line ($(( $(date +%s) - started )) s)"
done
git -C "$tree" checkout -q -- .
rm -f "$tree/crates/cli/src/weight.rs"
