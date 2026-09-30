# Comparison, 30 September 2026: raw output, no report yet

The rounds of `tinystore/bench/compare`, as they came out, for the report still
to be written. Nothing here is a conclusion; `python summarize.py <dir>` in
`bench/compare` turns a directory into median tables, `cards.py` and
`charts.py` draw from it.

| Directory | Machine | TinyStore |
|---|---|---|
| `wsl2/` | Ryzen 7 7700, 16 threads, WSL2 6.18, Docker, NVMe | `d7fb7ff` |
| `yc/` | Yandex Cloud, 8 vCPU Ice Lake, 16 GB, network-ssd-io-m3, Ubuntu 24.04 | `d7fb7ff` |
| `wsl2-gather/` | as `wsl2/` | `2b1c2a5`, stopped after stack, stack-steps, sqldb and mattn |

`environment.txt` in each has the versions; `progress.txt` what ran when.
The records corpus is private production logs on `wsl2/` and LogHub on `yc/`;
only aggregates are kept.

What changed during the night, and how the files show it:

- `<engine>.json` for sqldb, kv, records, blobs, the cold, latency and steady
  rounds and stack: the database/sql contenders (hand-made SQLite's readers,
  ncruces, mattn, Postgres) were run again after their pools were kept open,
  since database/sql kept two idle connections and reopened the rest on most
  calls. `merge.py` put the new runs in; `<engine>-before-pools.json` is the
  night's file, `<engine>-pools.json` the rerun, with TinyStore beside it as a
  control under `controls`.
- `stack-latency-superseded-*.json`: before the served client waited for a
  stream instead of being refused past 256, and before Postgres's pool was
  bounded below its `max_connections`.
- `sdk-kv-superseded-lcg.json`: its mixed stages drew from an LCG whose low bit
  alternated, so they wrote 20 % or 0 % of the time; get and set stand.
- The records round on `wsl2/` failed first on two untimed corpus lines placed
  at 1970, then ran again after they took their neighbour's time.
- `*-served.json`: sqldb, jobs and blobs embedded, through a sidecar and a
  remote server in one round. `sdk-kv.json`: kv from Bun and Python.
  `stack-steps.json`: each step of a stack request alone at 64.
