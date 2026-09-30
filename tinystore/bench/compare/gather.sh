#!/bin/sh
# The rounds that write through the group commit, again on a TinyStore that
# gathers a commit's writers (2b1c2a5), every contender in the same run, into
# $OUT, which is a directory of its own beside the night's.
set -u
out="${OUT:?OUT names the results directory}"
export OUT="$out"
ENGINES="stack stack-steps sqldb kv jobs blobs" sh run.sh
export TINYSTORE_BIN=/tmp/tinystore
round() {
	for last; do :; done
	if [ -s "$last" ]; then return 0; fi
	echo "$(date -u +%H:%M:%S) start $*" >> "$out/progress.txt"
	if "$@"; then
		echo "$(date -u +%H:%M:%S) done $last" >> "$out/progress.txt"
	else
		echo "$(date -u +%H:%M:%S) failed: $*" | tee -a "$out/failures.txt" >> "$out/progress.txt"
	fi
}
round /tmp/compare crash -cycles 20 -dir /data -out "$out/crash.json"
for engine in stack-latency kv-latency; do
	round /tmp/compare run -engine "$engine" -repeats 1 -seconds 15 -dir /data -out "$out/$engine.json"
done
sh served.sh
