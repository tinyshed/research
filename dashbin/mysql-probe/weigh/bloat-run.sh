#!/bin/sh
# cargo-bloat's crates for dashbin as it is and with each driver; $1 as run.sh takes it
S="$1"
tree="$S/weight-tree"
for variant in baseline mysql_async sqlx; do
    git -C "$tree" checkout -q -- .
    rm -f "$tree/crates/cli/src/weight.rs"
    if [ "$variant" != baseline ]; then python "$S/weight-variants/apply.py" "$tree" "$variant"; fi
    echo "## $variant"
    MSYS_NO_PATHCONV=1 docker run --rm -v "$(cygpath -w "$tree"):/src" -v "$(cygpath -w "$S/weight-variants"):/variants" \
        -v dashbin-rustup:/usr/local/rustup -v dashbin-cargo:/usr/local/cargo/registry -v dashbin-weight-target:/target \
        -e CARGO_TARGET_DIR=/target -w /src dashbin-musl:1.99.1 sh /variants/bloat.sh 2>&1
done
git -C "$tree" checkout -q -- .
rm -f "$tree/crates/cli/src/weight.rs"
