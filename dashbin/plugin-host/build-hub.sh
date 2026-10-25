#!/usr/bin/env bash
# the host with the hub around it, static musl, into the plughost-target volume as /target/out/plughost-hub
export MSYS_NO_PATHCONV=1
HERE="$(cygpath -m "$PWD" 2>/dev/null || pwd)"
docker run --rm -v "$HERE:/src" -v rssprobe-cargo:/usr/local/cargo/registry -v plughost-target:/target \
  -e CARGO_TARGET_DIR=/target -e CC_x86_64_unknown_linux_musl=musl-gcc -e CXX_x86_64_unknown_linux_musl=g++ \
  -e LIBSQLITE3_FLAGS="SQLITE_DQS=0 -USQLITE_ENABLE_MEMORY_MANAGEMENT SQLITE_ENABLE_DBPAGE_VTAB -DHAVE_FDATASYNC=1" \
  -w /src rssprobe-musl bash -c 'T=x86_64-unknown-linux-musl; cargo build -q --release --target $T --features hub \
  && mkdir -p /target/out && cp /target/$T/release/plughost /target/out/plughost-hub && ls -l /target/out/plughost-hub'
