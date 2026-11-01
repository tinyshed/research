#!/bin/sh
# Builds or runs the alp/zstd3/zstd19 encoder in the golang:1.27 container.
#
#   run.sh build
#   run.sh <alp|zstd3|zstd19> <series file> <out dir> <L1,L2,...>
#   run.sh --verify <alp|zstd3|zstd19> <series file> <bodies file>
#
# Sources and the build live in the volumes codec-pages-alp-src and
# codec-pages-alp-build; the repository is mounted read-only. HTTPS_PROXY and
# HTTP_PROXY, when set, are passed to the build; CODEC_PAGES_TRACE to a run.
# Works from a Linux shell and from Git Bash on Windows.
set -eu

IMAGE=${IMAGE:-golang:1.27}
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../../.." && pwd)

# A path docker on this host understands.
hostpath() {
	if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi
}

export MSYS_NO_PATHCONV=1

if [ "$#" -eq 1 ] && [ "$1" = build ]; then
	set --
	[ -n "${HTTPS_PROXY:-}" ] && set -- "$@" -e "HTTPS_PROXY=$HTTPS_PROXY"
	[ -n "${HTTP_PROXY:-}" ] && set -- "$@" -e "HTTP_PROXY=$HTTP_PROXY"
	exec docker run --rm "$@" \
		-v "$(hostpath "$repo"):/repo:ro" \
		-v codec-pages-alp-src:/src \
		-v codec-pages-alp-build:/build \
		"$IMAGE" sh /repo/tinystore/bench/codec-pages/alp/build.sh
fi

if [ "$#" -eq 4 ] && [ "$1" = --verify ]; then
	codec=$2 series=$3 bodies=$4
	sdir=$(cd "$(dirname "$series")" && pwd)
	bdir=$(cd "$(dirname "$bodies")" && pwd)
	exec docker run --rm \
		-v codec-pages-alp-build:/build:ro \
		-v "$(hostpath "$sdir"):/in:ro" \
		-v "$(hostpath "$bdir"):/bodies:ro" \
		"$IMAGE" /build/codec-pages-alp --verify "$codec" "/in/$(basename "$series")" "/bodies/$(basename "$bodies")"
fi

if [ "$#" -ne 4 ]; then
	echo "usage: $0 build | $0 <alp|zstd3|zstd19> <series file> <out dir> <L1,L2,...>" >&2
	echo "       $0 --verify <alp|zstd3|zstd19> <series file> <bodies file>" >&2
	exit 2
fi
codec=$1 series=$2 out=$3 lengths=$4
sdir=$(cd "$(dirname "$series")" && pwd)
sname=$(basename "$series")
mkdir -p "$out"
odir=$(cd "$out" && pwd)
set --
[ -n "${CODEC_PAGES_TRACE:-}" ] && set -- -e "CODEC_PAGES_TRACE=$CODEC_PAGES_TRACE"
exec docker run --rm "$@" \
	-v codec-pages-alp-build:/build:ro \
	-v "$(hostpath "$sdir"):/in:ro" \
	-v "$(hostpath "$odir"):/out" \
	"$IMAGE" /build/codec-pages-alp "$codec" "/in/$sname" /out "$lengths"
