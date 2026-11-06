#!/bin/sh
# Builds the probe from a scratch worktree of dashbin and starts its world; run from Git Bash on the host.
#
#   sh build.sh <scratch>    <scratch>/probe-tree is `git worktree add --detach <scratch>/probe-tree a8ef861`
#
# The Linux binary is built static, in the image `just size` uses, into docker/; the driver is built for the host.
set -eu
S="$1"
here="$(cd "$(dirname "$0")" && pwd)"
tree="$S/probe-tree"
rm -rf "$tree/crates/agent-probe"
mkdir -p "$tree/crates/agent-probe"
cp -r "$here/Cargo.toml" "$here/src" "$tree/crates/agent-probe/"
MSYS_NO_PATHCONV=1 docker run --rm -v "$(cygpath -w "$tree"):/src" -v "$(cygpath -w "$here/docker"):/out" \
    -v dashbin-rustup:/usr/local/rustup -v dashbin-cargo:/usr/local/cargo/registry -v dashbin-probe-target:/target \
    -e CARGO_TARGET_DIR=/target -w /src dashbin-musl:1.99.1 sh -c \
    'rustup target add x86_64-unknown-linux-musl >/dev/null 2>&1 &&
     cargo build -q --release --target x86_64-unknown-linux-musl -p agent-probe &&
     cp /target/x86_64-unknown-linux-musl/release/agent-probe /out/agent-probe'
(cd "$tree" && CARGO_TARGET_DIR="$S/probe-target" cargo build -q -p agent-probe)
cp "$S/probe-target/debug/agent-probe.exe" "$S/agent-probe-driver.exe"
(cd "$here" && docker compose up -d --build --wait)
