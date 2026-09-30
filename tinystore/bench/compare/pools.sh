#!/bin/sh
# Runs again the contenders a database/sql pool of two idle connections held
# back: hand-made SQLite's readers, ncruces, mattn and Postgres, which closed
# and reopened a connection on most calls until keepReaders and the Postgres
# pool kept them. Each round has TinyStore beside them as a control, into
# $OUT/<engine>-pools.json; merge.py puts the new runs in place of the old.
# STEADY_SECONDS (0 leaves the long rounds out) as night.sh takes it.
set -u

out="${OUT:?OUT names the results directory}"
set -e
go build -o /tmp/compare .
CGO_ENABLED=1 go build -tags mattn -o /tmp/compare-mattn .
go build -C ../../source/cmd/tinystore -o /tmp/tinystore .
set +e
export TINYSTORE_BIN=/tmp/tinystore

# round <binary> <engine> <contenders> <repeats> <seconds>, into <engine>-pools.json
round() {
	json="$out/$2-pools.json"
	[ "$2" = sqldb ] && [ "$3" = mattn ] && json="$out/sqldb-mattn-pools.json"
	if [ -s "$json" ]; then
		echo "$(date -u +%H:%M:%S) have $json" >> "$out/progress.txt"
		return 0
	fi
	echo "$(date -u +%H:%M:%S) start pools $2 $3" >> "$out/progress.txt"
	if "$1" run -engine "$2" -contenders "$3" -repeats "$4" -seconds "$5" -dir /data -out "$json"; then
		echo "$(date -u +%H:%M:%S) done $json" >> "$out/progress.txt"
	else
		echo "$(date -u +%H:%M:%S) failed: pools $2 $3" | tee -a "$out/failures.txt" >> "$out/progress.txt"
	fi
}

round /tmp/compare stack tinystore,services 3 5
round /tmp/compare kv tinystore,sqlite 3 5
round /tmp/compare sqldb tinystore,sqlite,ncruces,postgres 3 5
round /tmp/compare-mattn sqldb mattn 3 5
round /tmp/compare records tinystore,sqlite 3 5
round /tmp/compare blobs tinystore,sqlite 3 5
round /tmp/compare kv-cold tinystore,sqlite 3 5
round /tmp/compare sqldb-cold tinystore,sqlite,postgres 3 5
round /tmp/compare kv-latency tinystore,sqlite 1 15
round /tmp/compare stack-latency tinystore,services 1 15
if [ "${STEADY_SECONDS:-0}" != 0 ]; then
	round /tmp/compare kv-steady sqlite 1 "$STEADY_SECONDS"
	round /tmp/compare stack-steady services 1 "$STEADY_SECONDS"
fi
