#!/bin/sh
# Every round of the comparison, one engine after another, into results/<date>/.
# Run inside the image this directory's Dockerfile builds, from <research>:
#
#   sh tinystore/bench/compare/fetch-victoria.sh   (where GitHub is reachable)
#   docker build -t tinystore-compare tinystore/bench/compare
#   docker run --rm -v <research>:/src -v tinystore-compare-data:/data \
#     -e GOWORK=off -e GOFLAGS=-buildvcs=false tinystore-compare sh run.sh
#
# ENGINES, REPEATS, SECONDS_A_STAGE and OUT narrow a run; a round that fails
# is named in failures.txt and the others go on. Nothing else may run on the
# machine meanwhile: see the measure skill.
set -u

date=$(date -u +%Y-%m-%d)
out="${OUT:-results/$date}"
mkdir -p "$out"
set -e
go build -o /tmp/compare .
CGO_ENABLED=1 go build -tags mattn -o /tmp/compare-mattn .
go build -C ../../source/cmd/tinystore -o /tmp/tinystore .
set +e
export TINYSTORE_BIN=/tmp/tinystore

{
	echo "started $(date -u +%Y-%m-%dT%H:%M:%SZ)"
	go version
	redis-server --version
	/usr/lib/postgresql/*/bin/postgres --version
	victoria-metrics-prod -version
	uname -r
	grep -m1 'model name' /proc/cpuinfo
	nproc
	free -b | head -2
	echo "tinystore ${TINYSTORE_COMMIT:-unknown}"
} > "$out/environment.txt" 2>&1

round() {
	"$@" || echo "$(date -u +%H:%M:%S) failed: $*" >> "$out/failures.txt"
}

round /tmp/compare weight -out "$out/weight.json"
for engine in ${ENGINES:-stack kv sqldb metrics records jobs blobs}; do
	round /tmp/compare run -engine "$engine" -repeats "${REPEATS:-3}" -seconds "${SECONDS_A_STAGE:-5}" \
		-dir /data -out "$out/$engine.json"
	if [ "$engine" = sqldb ]; then
		round /tmp/compare-mattn run -engine sqldb -contenders mattn -repeats "${REPEATS:-3}" \
			-seconds "${SECONDS_A_STAGE:-5}" -dir /data -out "$out/sqldb-mattn.json"
	fi
done
echo "finished $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$out/environment.txt"
