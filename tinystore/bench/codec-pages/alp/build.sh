#!/bin/sh
# Builds the codec-pages encoder for alp, zstd3 and zstd19 inside golang:1.27
# (Debian trixie, g++ 14.2): fetches ALP at a pinned commit and zstd's source
# release, checks both, and compiles everything from source. Run through
# run.sh build; README.md has the docker command it wraps.
#
#   SRC    fetched sources, a volume (default /src)
#   BUILD  objects and the binary codec-pages-alp, a volume (default /build)
#
# A proxy, if the network needs one, comes in as HTTPS_PROXY / HTTP_PROXY.
set -eu

ALP_REPO=https://github.com/cwida/ALP.git
ALP_COMMIT=31ca0ed11c93c99d3f5b5c30e01a3e1c3832d3ce
ZSTD_VERSION=1.5.7
ZSTD_URL=https://github.com/facebook/zstd/releases/download/v$ZSTD_VERSION/zstd-$ZSTD_VERSION.tar.gz
# The release's own zstd-1.5.7.tar.gz.sha256.
ZSTD_SHA256=eb33e51f49a15e023950cd7825ca74a4a2b43db8354825ac24fc1b7ee09e6fa3

HERE=$(cd "$(dirname "$0")" && pwd)
SRC=${SRC:-/src}
BUILD=${BUILD:-/build}
JOBS=$(nproc 2>/dev/null || echo 4)
CXXFLAGS="-std=c++17 -O2 -fno-strict-aliasing -fwrapv -Wall -Wno-unknown-pragmas -Wno-pragmas -Wno-sign-compare"

retry() {
	n=0
	until "$@"; do
		n=$((n + 1))
		[ "$n" -ge 6 ] && return 1
		echo "retrying: $*" >&2
		sleep 3
	done
}

mkdir -p "$SRC" "$BUILD"

# ALP, unmodified, at the pinned commit.
ALP=$SRC/ALP
if [ "$(git -C "$ALP" rev-parse HEAD 2>/dev/null || true)" != "$ALP_COMMIT" ]; then
	rm -rf "$ALP"
	git init -q "$ALP"
	git -C "$ALP" remote add origin "$ALP_REPO"
	retry git -C "$ALP" fetch -q --depth 1 origin "$ALP_COMMIT"
	git -C "$ALP" -c advice.detachedHead=false checkout -q FETCH_HEAD
fi
test "$(git -C "$ALP" rev-parse HEAD)" = "$ALP_COMMIT"
test -z "$(git -C "$ALP" status --porcelain)"

# zstd from its source release tarball, checked against the published hash.
TARBALL=$SRC/zstd-$ZSTD_VERSION.tar.gz
if ! echo "$ZSTD_SHA256  $TARBALL" | sha256sum -c - >/dev/null 2>&1; then
	rm -f "$TARBALL"
	retry curl -fsSL --retry 3 -o "$TARBALL" "$ZSTD_URL"
fi
echo "$ZSTD_SHA256  $TARBALL" | sha256sum -c -
ZSTD=$BUILD/zstd-$ZSTD_VERSION
if [ ! -f "$ZSTD/lib/libzstd.a" ]; then
	rm -rf "$ZSTD"
	tar -xzf "$TARBALL" -C "$BUILD"
	make -s -C "$ZSTD/lib" -j"$JOBS" libzstd.a
fi

# ALP's generated FastLanes and falp kernels, then the encoder.
OBJ=$BUILD/obj
mkdir -p "$OBJ"
for f in falp fastlanes_ffor fastlanes_generated_ffor fastlanes_unffor fastlanes_generated_unffor; do
	if [ ! -f "$OBJ/$f.o" ] || [ "$ALP/src/$f.cpp" -nt "$OBJ/$f.o" ]; then
		g++ $CXXFLAGS -I"$ALP/include" -c "$ALP/src/$f.cpp" -o "$OBJ/$f.o" &
	fi
done
wait
g++ $CXXFLAGS -DALP_COMMIT="\"$ALP_COMMIT\"" -I"$ALP/include" -I"$ZSTD/lib" \
	-c "$HERE/encode.cpp" -o "$OBJ/encode.o"
g++ -o "$BUILD/codec-pages-alp" "$OBJ/encode.o" \
	"$OBJ/falp.o" "$OBJ/fastlanes_ffor.o" "$OBJ/fastlanes_generated_ffor.o" \
	"$OBJ/fastlanes_unffor.o" "$OBJ/fastlanes_generated_unffor.o" \
	"$ZSTD/lib/libzstd.a" -pthread

echo "built $BUILD/codec-pages-alp: ALP $ALP_COMMIT, zstd $ZSTD_VERSION ($ZSTD_SHA256), $(g++ --version | head -1)"
