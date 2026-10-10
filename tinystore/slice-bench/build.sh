#!/bin/sh
# Builds what the slice round measures, each from a `git archive` of its own
# commit of TinyStore, into /perf/slice on the measurements' volume.
#
#   build.sh sources   in any image with git: the two trees
#   build.sh rust      in rust:1.99: the Rust program, the core's library and tinystore
#   build.sh go        in golang:1.27: the Go program and Go's tinystore
#
# /tiny is a TinyStore checkout that has both commits, /src this repository.
set -eu

RUST_COMMIT=${RUST_COMMIT:-51b2592}
GO_COMMIT=${GO_COMMIT:-e81a050}
WORK=${WORK:-/perf/slice}

case "$1" in
sources)
	rm -rf "$WORK"
	mkdir -p "$WORK/rust-src" "$WORK/go-src" "$WORK/bin"
	git -c safe.directory=/tiny -C /tiny archive "$RUST_COMMIT" | tar -x -C "$WORK/rust-src"
	git -c safe.directory=/tiny -C /tiny archive "$GO_COMMIT" | tar -x -C "$WORK/go-src"
	git -c safe.directory=/tiny -C /tiny rev-parse "$RUST_COMMIT" > "$WORK/rust-commit"
	git -c safe.directory=/tiny -C /tiny rev-parse "$GO_COMMIT" > "$WORK/go-commit"
	;;
rust)
	# the workspace's own member, so that it builds with the workspace's
	# SQLite, its lockfile and its lints; the flags are CI's on Linux
	rm -rf "$WORK/rust-src/crates/slice-bench"
	cp -r /src/tinystore/slice-bench/rust "$WORK/rust-src/crates/slice-bench"
	cd "$WORK/rust-src"
	export LIBSQLITE3_FLAGS='SQLITE_DQS=0 -DHAVE_FDATASYNC=1'
	cargo build --release -p slice-bench -p tinystore-ffi -p tinystore-cli
	cp "$CARGO_TARGET_DIR/release/slice-bench" "$WORK/bin/rust-slice"
	cp "$CARGO_TARGET_DIR/release/tinystore" "$WORK/bin/tinystore-rust"
	cp "$CARGO_TARGET_DIR/release/libtinystore_ffi.so" "$WORK/bin/libtinystore_ffi.so"
	rustc --version > "$WORK/rustc-version"
	;;
go)
	rm -rf "$WORK/go-bench"
	cp -r /src/tinystore/slice-bench/go "$WORK/go-bench"
	cd "$WORK/go-bench"
	go mod edit -replace "github.com/tinyshed/tinystore=$WORK/go-src"
	go mod tidy
	CGO_ENABLED=0 go build -o "$WORK/bin/go-slice" .
	cd "$WORK/go-src/cmd/tinystore"
	CGO_ENABLED=0 go build -o "$WORK/bin/tinystore-go" .
	go version > "$WORK/go-version"
	;;
*)
	echo "build.sh sources|rust|go" >&2
	exit 2
	;;
esac
