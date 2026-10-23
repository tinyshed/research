#!/bin/sh
# The whole round, one after another so that no case shares the machine:
# the product rounds three passes each, then the archive rounds two. Each
# round's lines go to its own file in OUT, and a round already written is
# skipped, so that a night stopped half way runs on from where it stopped.
#
#   docker run --rm --init --privileged -v <release-bench>:/src:ro -v <results>:/results \
#     -v tinystore-release:/work -v tinystore-corpus:/corpus -v tinystore-archive-target:/archive \
#     -e OUT=/results/<name> tinystore-release-bench sh /src/night.sh
set -eu
: "${OUT:?OUT names the results directory}"
rm -rf /work/bench /work/runs
mkdir -p /work/bench "$OUT"
cp -r /src/Dockerfile /src/go.sh /src/night.sh /src/package.json /src/bun.lock /src/run.py /src/services.py \
	/src/versions.sh /src/peek.py /src/bench /work/bench/
cd /work/bench
bun install --frozen-lockfile > /dev/null
round() {
	name=$1
	shift
	if [ -s "$OUT/$name.jsonl" ]; then
		echo "$(date -u +%H:%M:%S) $name already measured"
		return
	fi
	env "$@" ROUNDS="$name" python3 run.py || echo "$(date -u +%H:%M:%S) $name had a failed case"
	echo "$(date -u +%H:%M:%S) $name done" >> "$OUT/progress.txt"
}
round idle PASSES=3 REST=60
round app PASSES=3 SECONDS=20 REST=20
round kv PASSES=3 SECONDS=10
round sql PASSES=3 SECONDS=10
round kv-memory PASSES=2 SECONDS=10
round files PASSES=2 SECONDS=10
round metrics PASSES=2
round logs PASSES=2 REPEAT=3
echo "$(date -u +%H:%M:%S) NIGHT-DONE" >> "$OUT/progress.txt"
