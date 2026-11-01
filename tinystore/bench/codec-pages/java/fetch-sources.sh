#!/bin/sh
# Runs inside golang:1.27 (it has git, gcc and make; see codec-pages.sh). Fetches
# the authors' sources at pinned commits into /src, a Docker volume, and
# generates fastutil's type-specific sources, which its repository keeps as
# C-preprocessor drivers.
#
# Nothing here is a binary: every class the harness runs is compiled from
# these trees by compile.sh.
set -eu

fetch() {
	name=$1 url=$2 commit=$3
	dir=/src/$name
	echo "$name $url $commit" >>/src/commits.txt
	if [ -d "$dir/.git" ] && [ "$(git -C "$dir" rev-parse HEAD)" = "$commit" ] && [ -z "$(git -C "$dir" status --porcelain --untracked-files=no)" ]; then
		echo "$name: already at $commit"
		return
	fi
	rm -rf "$dir"
	mkdir -p "$dir"
	git -C "$dir" init -q
	git -C "$dir" fetch -q --depth 1 "$url" "$commit"
	git -C "$dir" -c advice.detachedHead=false checkout -q FETCH_HEAD
	test "$(git -C "$dir" rev-parse HEAD)" = "$commit"
	echo "$name: $commit"
}

rm -f /src/commits.txt

# Gorilla (the value-only Compressor the Chimp paper measured), Chimp, Chimp128.
fetch chimp https://github.com/panagiotisl/chimp 320d397157c7e0696b3c64dc1711fc17a3add3da
# Elf, branch vldb2023-release: the VLDB 2023 paper's version.
fetch elf https://github.com/Spatio-Temporal-Lab/elf 386d69348e6c30475761d3519ee0bfe10ed473d3
# The bit streams of both repositories are copies of dsiutils' and import it
# and fastutil: dsiutils at the commit that set version=2.7.0 (the version
# both poms name; the repository has no 2.7.0 tag), fastutil at tag 8.5.8
# (the latest release when dsiutils 2.7.0 was made).
fetch dsiutils https://github.com/vigna/dsiutils 838c5070b74382a652649f24042c19e4f492dab5
fetch fastutil https://github.com/vigna/fastutil dc909bb15d7cb0b9d40d7382bf7bb0a5c6c517e0

if [ ! -f /src/fastutil/.generated ]; then
	echo "fastutil: generating sources"
	make -C /src/fastutil -s sources >/src/fastutil-generate.log 2>&1 || {
		tail -50 /src/fastutil-generate.log
		exit 1
	}
	touch /src/fastutil/.generated
fi
echo "fastutil: $(find /src/fastutil/src -name '*.java' | wc -l) java files"
