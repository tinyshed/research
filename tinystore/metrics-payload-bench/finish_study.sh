#!/bin/sh
set -eu
export PATH=/work/cargo/bin:$PATH
export GOWORK=off CGO_ENABLED=0 GOMAXPROCS=1
study_root=/src/tinystore/metrics-payload-bench
variants='baseline,compact-count8,rowid-compact-unpacked,rowid-compact-count32'
python3 "$study_root/run.py" performance --datasets tsbs,alibaba --variants "$variants"
python3 "$study_root/run.py" diagnostics --datasets tsbs,alibaba --variants "$variants"
python3 "$study_root/run.py" lifecycle --datasets tsbs,alibaba --variants "$variants"
