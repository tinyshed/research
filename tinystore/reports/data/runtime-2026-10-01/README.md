# Runtime Round Data

Local-only draft for `../../runtime-continuation-2026-10-01.md`.

- `wsl2/`: Ryzen 7 7700, 16 visible CPUs, Docker Desktop Linux/WSL2.
- `yc/`: existing 8-vCPU Ice Lake Yandex VM, Ubuntu 24.04, network SSD.
- `micro/`: two-pass port/idle-slot comparisons with per-worker and shared
  contexts. The idle-slot branch was rejected.
- `wsl2/physical/`: shorter child runs retain their closed databases on the
  measurement volume. Only JSON and per-object page ownership is kept here;
  the three page-output lines are candidate metrics, baseline metrics and
  candidate records. No private database is committed.
- Each machine's `rejected/metrics.json` contains the original wide reads
  that all failed TinyStore's default output bound. Do not quote their
  attempted-operation rates as successful query rates.

The accepted `metrics.json` uses the corrected, saved harness `127c3a0`.
Other full-round and SDK JSON uses `264981d`. Source commits are `ad4f047`
(candidate), `20c71a3` (modernc baseline), and `ccbd2ce`/`eeb0aea` for the
micro comparisons. `environment.txt` and `metrics-environment.txt` separate
the commands' environments. The SDK JSON's `started` is its aggregation
time; the actual start is in `progress.txt`.

The JSON is retained without dropping losing contenders, slow passes or
queue counts. No corpus, database file, private log line, account token or
machine home directory is published here. `audit.md` is generated from the
JSON with `bench/compare/audit.py`; that command returns nonzero on failed
runs/stages or lost/wrong crash values. Waiting jobs are printed, not
silently treated as foreground-operation errors.

Memory is client high-water RSS plus the greater of service high-water RSS
and final service-tree PSS. It is not a simultaneous whole-tree peak.
Final disk bytes after timed writes depend on how much each writer wrote;
fixed metrics and records corpora are the comparable footprint stages.

```sh
python tinystore/bench/compare/audit.py tinystore/reports/data/runtime-2026-10-01/wsl2
python tinystore/bench/compare/audit.py tinystore/reports/data/runtime-2026-10-01/yc
python tinystore/bench/compare/cards.py tinystore/reports/data/runtime-2026-10-01/wsl2 <tinystore>/.github/assets
```
