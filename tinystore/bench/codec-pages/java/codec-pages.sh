#!/bin/sh
# The Java encoders of ../README.md, built and run in Linux containers.
#
#   java/codec-pages.sh build
#       Fetches the authors' sources at pinned commits (fetch-sources.sh, in
#       golang:1.27 for its git, gcc and make) into the Docker volume
#       codec-pages-java-src, and compiles them with the harness
#       (compile.sh, in eclipse-temurin:21-jdk) into codec-pages-java-build.
#       Set CODEC_PAGES_PROXY (e.g. http://<proxy host>:<port>) to
#       clone through an HTTP proxy.
#
#   java/codec-pages.sh <gorilla|chimp|chimp128|elf> <series file> <out dir> <L1,L2,...>
#       The encoder: writes <out dir>/<codec>.L<L>.bodies and prints one
#       JSON line for each block length. The series file's folder is
#       mounted read-only, the out dir read-write.
set -eu

GIT_IMAGE=golang:1.27@sha256:2920bb124f1a2e12a1d39f0cab756ae0068441ee1da7655dec7f5e374944095b
JDK_IMAGE=eclipse-temurin:21-jdk@sha256:ebfbb18d11ff163ee83b21258acf12248af48d22a0fdfdc68fa1a1516c05976a

# Git Bash would rewrite the container paths; Docker Desktop takes D:/... host paths.
export MSYS_NO_PATHCONV=1
hostpath() { (cd "$1" && (pwd -W 2>/dev/null || pwd)); }
here=$(hostpath "$(dirname "$0")")

user=""
if [ "$(uname -s)" = Linux ]; then user="--user $(id -u):$(id -g)"; fi

if [ "${1:-}" = build ]; then
	proxy=""
	if [ -n "${CODEC_PAGES_PROXY:-}" ]; then
		proxy="-e https_proxy=$CODEC_PAGES_PROXY -e http_proxy=$CODEC_PAGES_PROXY"
	fi
	# shellcheck disable=SC2086
	docker run --rm $proxy -v codec-pages-java-src:/src -v "$here":/harness:ro "$GIT_IMAGE" sh /harness/fetch-sources.sh
	docker run --rm -v codec-pages-java-src:/src:ro -v codec-pages-java-build:/build -v "$here":/harness:ro "$JDK_IMAGE" sh /harness/compile.sh
	exit 0
fi

if [ $# -ne 4 ]; then
	echo "usage: $0 build | <gorilla|chimp|chimp128|elf> <series file> <out dir> <L1,L2,...>" >&2
	exit 2
fi
codec=$1 series=$2 out=$3 lengths=$4
case $codec in
gorilla | chimp | chimp128) tree=chimp ;;
elf) tree=elf ;;
*) echo "unknown codec $codec" >&2; exit 2 ;;
esac
[ -f "$series" ] || { echo "no series file $series" >&2; exit 2; }
mkdir -p "$out"
in_dir=$(hostpath "$(dirname "$series")")
out_dir=$(hostpath "$out")

# shellcheck disable=SC2086
exec docker run --rm $user \
	-v codec-pages-java-build:/build:ro \
	-v "$in_dir":/in:ro \
	-v "$out_dir":/out \
	"$JDK_IMAGE" \
	java -cp /build/$tree codecpages.CodecPages "$codec" "/in/$(basename "$series")" /out "$lengths"
