#!/bin/sh
# The whole comparison in one night: the rounds of run.sh, then the deep ones,
# into results/<date>/. Run as run.sh is, in a privileged container, since the
# cold rounds drop the kernel's page cache:
#
#   docker run --rm --privileged -v <research>:/src -v tinystore-compare-data:/data \
#     -e GOWORK=off -e GOFLAGS=-buildvcs=false -e TINYSTORE_COMMIT=<commit> \
#     tinystore-compare sh night.sh
#
# REPEATS and SECONDS_A_STAGE go to run.sh; CRASH_CYCLES (20), COLD_REPEATS
# (3), LATENCY_SECONDS (15) and STEADY_SECONDS (900; 0 leaves the long runs
# out) set the deep rounds, so that a smoke run is this script with small
# numbers. A round that fails is named in failures.txt; running the script
# again with the same OUT redoes only the rounds whose JSON is missing.
set -u

date=$(date -u +%Y-%m-%d)
export OUT="${OUT:-results/$date}"
mkdir -p "$OUT"
sh run.sh
export TINYSTORE_BIN=/tmp/tinystore

# round runs as run.sh's does: skipped when its JSON is there, and logged
round() {
	for last; do :; done
	if [ -s "$last" ]; then
		echo "$(date -u +%H:%M:%S) have $last" >> "$OUT/progress.txt"
		return 0
	fi
	echo "$(date -u +%H:%M:%S) start $*" >> "$OUT/progress.txt"
	if "$@"; then
		echo "$(date -u +%H:%M:%S) done $last" >> "$OUT/progress.txt"
	else
		echo "$(date -u +%H:%M:%S) failed: $*" | tee -a "$OUT/failures.txt" >> "$OUT/progress.txt"
	fi
}

round /tmp/compare crash -cycles "${CRASH_CYCLES:-20}" -dir /data -out "$OUT/crash.json"
for engine in kv-cold sqldb-cold; do
	round /tmp/compare run -engine "$engine" -repeats "${COLD_REPEATS:-3}" -dir /data -out "$OUT/$engine.json"
done
for engine in kv-latency stack-latency; do
	round /tmp/compare run -engine "$engine" -repeats 1 -seconds "${LATENCY_SECONDS:-15}" -dir /data -out "$OUT/$engine.json"
done
steady="${STEADY_SECONDS:-900}"
if [ "$steady" != 0 ]; then
	for engine in kv-steady stack-steady; do
		round /tmp/compare run -engine "$engine" -repeats 1 -seconds "$steady" -dir /data -out "$OUT/$engine.json"
	done
fi
echo "night finished $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$OUT/environment.txt"
