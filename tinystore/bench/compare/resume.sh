#!/bin/sh
# A saved baseline and candidate, interleaved with the other engines.
# Mount the baseline at /baseline and this research checkout at /src.
set -eu

out="${OUT:-results/2026-10-01}"
baseline="${BASELINE_ROOT:-/baseline}"
candidate="${CANDIDATE_ROOT:-../../source}"
mkdir -p "$out"

cp go.mod /tmp/compare-baseline.mod
cp go.sum /tmp/compare-baseline.sum
go mod edit -modfile=/tmp/compare-baseline.mod \
  -replace=github.com/tinyshed/tinystore="$baseline" \
  -replace=github.com/tinyshed/tinystore/server="$baseline/server"
go build -tags batchapi -modfile=/tmp/compare-baseline.mod -o /tmp/compare-baseline .
go build -tags batchapi -o /tmp/compare-candidate .
CGO_ENABLED=1 go build -tags mattn,batchapi \
  -ldflags=-X=github.com/ncruces/go-sqlite3/driver.driverName= -o /tmp/compare-mattn .
go build -C "$baseline/cmd/tinystore" -o /tmp/tinystore-baseline .
go build -C "$candidate/cmd/tinystore" -o /tmp/tinystore-candidate .

export TINYSTORE_BIN=/tmp/tinystore-candidate
export COMPARE_BASELINE_BIN=/tmp/compare-baseline
export COMPARE_BASELINE_SERVER=/tmp/tinystore-baseline

{
  date -u +%Y-%m-%dT%H:%M:%SZ
  go version
  uname -r
  nproc
  grep -m1 'model name' /proc/cpuinfo
  redis-server --version
  /usr/lib/postgresql/*/bin/postgres --version
  victoria-metrics-prod -version
  printf 'baseline %s\ncandidate %s\nharness %s\n' \
    "${COMPARE_BASELINE_COMMIT:?}" "${TINYSTORE_COMMIT:?}" "${HARNESS_COMMIT:?}"
} > "$out/environment.txt" 2>&1

round() {
  name=$1
  shift
  printf '%s start %s\n' "$(date -u +%H:%M:%S)" "$name" >> "$out/progress.txt"
  if "$@" -out "$out/$name.json" >> "$out/runs.log" 2>&1; then
    printf '%s done %s\n' "$(date -u +%H:%M:%S)" "$name" >> "$out/progress.txt"
  else
    printf '%s failed %s\n' "$(date -u +%H:%M:%S)" "$name" >> "$out/progress.txt"
    printf '%s\n' "$name" >> "$out/failures.txt"
  fi
}

for engine in ${ENGINES:-stack kv sqldb jobs blobs records metrics}; do
  contenders=""
  if [ "$engine" = stack ]; then
    contenders="-contenders tinystore-batch,tinystore-batch-baseline,tinystore,tinystore-baseline,tinystore-sidecar,tinystore-server,services"
  fi
  # The contender names above are literal words, not shell input.
  round "$engine" /tmp/compare-candidate run -engine "$engine" $contenders \
    -repeats "${REPEATS:-3}" -seconds "${SECONDS_A_STAGE:-5}" -dir /data
  if [ "$engine" = sqldb ]; then
    round sqldb-mattn /tmp/compare-mattn run -engine sqldb -contenders mattn \
      -repeats "${REPEATS:-3}" -seconds "${SECONDS_A_STAGE:-5}" -dir /data
  fi
done

for engine in sqldb jobs blobs; do
  round "$engine-served" /tmp/compare-candidate run -engine "$engine" \
    -contenders tinystore,tinystore-sidecar,tinystore-server \
    -repeats "${REPEATS:-3}" -seconds "${SECONDS_A_STAGE:-5}" -dir /data
done
round crash /tmp/compare-candidate crash -cycles "${CRASH_CYCLES:-30}" -dir /data
round weight /tmp/compare-candidate weight
printf '%s DONE\n' "$(date -u +%H:%M:%S)" >> "$out/progress.txt"
