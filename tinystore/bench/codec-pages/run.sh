#!/bin/sh
# One dataset through every encoder and into SQLite files, in Linux containers.
#
#   sh run.sh <corpus> <dataset> [codecs] [lengths] [pages]
#
# <corpus>/series/<dataset>.series is copied into the Docker volume
# codec-pages-work; each codec encodes every block length into
# /work/out/<dataset>/, writing its own JSON lines to
# /work/results/<dataset>.encode.jsonl and its complaints to
# <dataset>.encode.stderr; store.py then builds, divides and predicts the
# SQLite files into /work/results/<dataset>.store.jsonl. Bodies of the
# lengths in KEEP are kept for the PostgreSQL pass; the rest are deleted once
# stored. The three encoders are built first (README.md).
set -eu

corpus=$1 ds=$2
codecs=${3:-tinystore alp zstd3 zstd19 gorilla chimp chimp128 elf}
lengths=${4:-120,160,200,240,320,400,480,640,800,960,1000,1024,1280,1600,1920,2048,2560,3200,3840,4096,5120,6144,7168,8192}
pages=${5:-1024,4096,8192,16384}
KEEP=${KEEP:-240 1000 1024}

export MSYS_NO_PATHCONV=1
hostpath() { if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s' "$1"; fi; }
here=$(cd "$(dirname "$0")" && pwd)
repo=$(hostpath "$(cd "$here/../../.." && pwd)")
corpus=$(hostpath "$(cd "$corpus" && pwd)")

GO=golang:1.27@sha256:2920bb124f1a2e12a1d39f0cab756ae0068441ee1da7655dec7f5e374944095b
JDK=eclipse-temurin:21-jdk@sha256:ebfbb18d11ff163ee83b21258acf12248af48d22a0fdfdc68fa1a1516c05976a
R=/work/results/$ds

docker run --rm -v codec-pages-work:/work -v "$corpus/series:/corpus:ro" $GO sh -c "
	mkdir -p /work/series /work/out/$ds /work/results
	[ -f /work/series/$ds.series ] || cp /corpus/$ds.series /work/series/$ds.series"

for codec in $codecs; do
	args="$codec /work/series/$ds.series /work/out/$ds $lengths"
	case $codec in
	tinystore) docker run --rm -v codec-pages-work:/work -v codec-pages-go:/go:ro $GO \
		sh -c "/go/bin/cp-tinystore $args >> $R.encode.jsonl 2>> $R.encode.stderr" ;;
	alp | zstd3 | zstd19) docker run --rm -v codec-pages-work:/work -v codec-pages-alp-build:/build:ro $GO \
		sh -c "/build/codec-pages-alp $args >> $R.encode.jsonl 2>> $R.encode.stderr" ;;
	gorilla | chimp | chimp128) docker run --rm -v codec-pages-work:/work -v codec-pages-java-build:/build:ro $JDK \
		sh -c "java -cp /build/chimp codecpages.CodecPages $args >> $R.encode.jsonl 2>> $R.encode.stderr" ;;
	elf) docker run --rm -v codec-pages-work:/work -v codec-pages-java-build:/build:ro $JDK \
		sh -c "java -cp /build/elf codecpages.CodecPages $args >> $R.encode.jsonl 2>> $R.encode.stderr" ;;
	*) echo "unknown codec $codec" >&2; exit 2 ;;
	esac
	drop=""
	for L in $(echo "$lengths" | tr , ' '); do
		case " $KEEP " in *" $L "*) ;; *) drop="$drop /work/out/$ds/$codec.L$L.bodies" ;; esac
	done
	docker run --rm -v codec-pages-work:/work -v "$repo:/src:ro" $GO sh -c "
		python3 -I /src/tinystore/bench/codec-pages/store.py \
			--model /src/tinystore/bench/page-model/page_model.py --dataset $ds --pages $pages \
			--tmp /tmp/db --out $R.store.jsonl /work/out/$ds/$codec.L*.bodies > /dev/null && rm -f $drop"
	echo "$(date -u +%H:%M:%S) $ds $codec stored" >&2
done
