# The release against what it replaces

The harness of the [2026-10-23 round](../reports/release-2026-10-23.md):
TinyStore `v0.1.0` as npm and crates.io publish it, beside PostgreSQL, Redis,
Valkey, MinIO, VictoriaMetrics, Prometheus and Loki, one stack at a time in
one container.

| Path               | What it is                                                                                               |
|--------------------|----------------------------------------------------------------------------------------------------------|
| `Dockerfile`       | the image: PostgreSQL 18's, every other service's binary copied from its own image, MinIO built from its tag, Bun 1.4.2 |
| `run.py`           | a case: its services started, its program run, every process's memory and CPU sampled by the program's phases |
| `services.py`      | each service as its vendor ships it, and every setting the round changed, with why                      |
| `bench/*.ts`       | the Bun programs: `idle`, `app`, `kv`, `kv-memory`, `sql`, `files`, and what they share                  |
| `archive/`         | the Rust program of the archive rounds: TSBS into TinyStore, VictoriaMetrics and Prometheus; LogHub into TinyStore and Loki |
| `fetch-corpus.sh`  | LogHub from Zenodo, checked; two TSBS DevOps corpora from the pinned generator                           |
| `night.sh`         | every round, one after another, skipping a round already written                                         |
| `summarize.py`     | a results directory as `summary.json` and `summary.md`                                                   |
| `cards.py`         | the README's cards from the summary, in the form TinyStore's landing page reads                         |
| `charts.py`        | the report's charts from the summary                                                                     |
| `peek.py`          | a round's lines at a glance while it runs                                                                |

The report's last section has the commands, in order.
