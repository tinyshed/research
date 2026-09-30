#!/bin/sh
# Runs one round again after the harness was fixed, the JSON it replaces kept
# beside it as <engine>-superseded-<time>.json, as run.sh runs a round:
#
#   ENGINE=stack-latency REPEATS=1 SECONDS_A_STAGE=15 sh rerun.sh
#
# OUT is the results directory the round belongs to.
set -u

out="${OUT:?OUT names the results directory}"
engine="${ENGINE:?ENGINE names the round}"
set -e
go build -o /tmp/compare .
go build -C ../../source/cmd/tinystore -o /tmp/tinystore .
set +e
export TINYSTORE_BIN=/tmp/tinystore

if [ -e "$out/$engine.json" ]; then
	mv "$out/$engine.json" "$out/$engine-superseded-$(date -u +%H%M).json"
fi
echo "$(date -u +%H:%M:%S) rerun $engine" >> "$out/progress.txt"
if /tmp/compare run -engine "$engine" -repeats "${REPEATS:-3}" -seconds "${SECONDS_A_STAGE:-5}" -dir /data \
	-out "$out/$engine.json"; then
	echo "$(date -u +%H:%M:%S) done $out/$engine.json" >> "$out/progress.txt"
else
	echo "$(date -u +%H:%M:%S) failed: rerun $engine" | tee -a "$out/failures.txt" >> "$out/progress.txt"
fi
