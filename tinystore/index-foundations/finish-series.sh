#!/bin/sh
set -eu
# All builds must finish first. Sequential phases; fail visibly on any error.
python3 run.py checks
python3 run.py locks
python3 /src/tinystore/channels-wire-lab/run.py
python3 run.py rebuilds
python3 /src/tinystore/index-outbox-lab/run.py
python3 geo-certified.py
python3 sensitivity.py wal
python3 sensitivity.py continuous
python3 large-crashes.py
BENCH_BIN=/target/release/index-foundations-alloc python3 diagnostics.py
touch /data/index-foundations-followup.done
