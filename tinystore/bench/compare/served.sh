#!/bin/sh
# The SQL, jobs and blobs engines embedded, through a sidecar and through a
# remote server, in one round each, into $OUT/<engine>-served.json. Run as
# run.sh is, after it; like it, a round whose JSON is there is skipped.
set -u

date=$(date -u +%Y-%m-%d)
out="${OUT:-results/$date}"
mkdir -p "$out"
set -e
go build -o /tmp/compare .
go build -C ../../source/cmd/tinystore -o /tmp/tinystore .
set +e
export TINYSTORE_BIN=/tmp/tinystore

round() {
	for last; do :; done
	if [ -s "$last" ]; then
		echo "$(date -u +%H:%M:%S) have $last" >> "$out/progress.txt"
		return 0
	fi
	echo "$(date -u +%H:%M:%S) start $*" >> "$out/progress.txt"
	if "$@"; then
		echo "$(date -u +%H:%M:%S) done $last" >> "$out/progress.txt"
	else
		echo "$(date -u +%H:%M:%S) failed: $*" | tee -a "$out/failures.txt" >> "$out/progress.txt"
	fi
}

for engine in sqldb jobs blobs; do
	round /tmp/compare run -engine "$engine" -contenders tinystore,tinystore-sidecar,tinystore-server \
		-repeats "${REPEATS:-3}" -seconds "${SECONDS_A_STAGE:-5}" -dir /data -out "$out/$engine-served.json"
done
