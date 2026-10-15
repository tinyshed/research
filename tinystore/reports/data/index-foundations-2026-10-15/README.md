# Reproduction and evidence ledger

Product source: `tinyshed/tinystore` rust revision
`76b844cdc8c3410e746a5d5dc3e9aafec83c9962`; Cargo.lock pins every Rust dependency.
Historical Go submodule is unchanged. Environment/corpus provenance is in the
adjacent JSON files. Main performance uses disk-backed Docker named volumes,
two CPU quota, 6 GiB RAM, release builds; all candidates run sequentially.
No build/test/lint runs concurrently with timing. Two passes are reported
separately; source fixtures are copied from a checkpointed template per case.
Template creation is excluded. Main static/rebuild use forward/reverse order;
external outbox comparisons use native/outbox/native/outbox.

From `tinystore/index-foundations` inside the prepared lab container:

```sh
python3 corpus.py
cargo build --release --locked
sh size-builds.sh
python3 run.py static
sh finish-series.sh
```

`finish-series.sh` runs correctness, actual Core admission, Bun/Wire delivery,
concurrent rebuild, external durable outbox, geo query, WAL/cache/order,
continuous-writer and large-crash series, then a separate allocation cohort.
The Channels adapter and outbox binaries must also have been built first:

```sh
cargo build --release --locked --manifest-path ../channels-wire-lab/Cargo.toml
cargo build --release --locked --manifest-path ../index-outbox-lab/Cargo.toml
```

The unchanged pinned Bun SDK must be available through `SDK_ROOT` (the driver's
lab default is `/data/sdk-js`); Bun 1.4.2 is on PATH. Set `CARGO_TARGET_DIR` to
the shared target volume (`/target`). Driver defaults refer to the documented
lab paths `/src`, `/data`, `/target`; override their environment variables or
mount the repository at `/src`. No credentials or secret values are needed by
the experiments. Retain normal network proxy/CA settings when fetching/building.

Disk-full and 64 MiB cgroup tests are separate correctness experiments:

```sh
python3 pressure.py
# On the outer Docker host, not concurrently with performance:
python3 tinystore/index-foundations/memory.py
# From the repository root:
python3 tinystore/index-foundations/summarize.py
```

Raw records retain UTC start, exact arguments, source revision, exit status,
stderr and complete counters/samples. Environment variables that alter a
sensitivity case are recorded as `experimental_env`; placeholders identify
scratch/target paths without depending on one machine. Source/table and index
bytes are separate dbstat objects. WAL totals include 24-byte frame headers;
logical WAL high-water and sampled file footprint are distinct metrics.

## Cohorts and limits

- `static.jsonl`: 96 completed fresh processes, two passes per scenario. Matched
  checksums agree across all 24 size/shape/pass/query-class groups. Corpus is
  repeated/augmented, not one million independent natural documents.
- `rebuilds.jsonl`: fixed minimum 2000 foreground transactions, two unpaced
  reader threads. Compare tx1/u10 against its baseline and tx32/u90 against its
  own baseline. Long builds can outlast the fixed foreground write window.
- `sensitivity.jsonl`: 64 separate WAL-page observer/cache/order diagnostics and
  24 continuous-write scenarios. Continuous load uses the actual Core
  1 MiB cache at both sizes, with none/frontier/delta and two passes. The main
  fixed-write matrix already covers 4 MiB, while WAL diagnostics isolate cache
  1/4/64 MiB; the extra continuous 4 MiB cross-product is deliberately omitted. Do not mix instrumented timing with main timing,
  or compare cumulative WAL over different mutation counts as identical work.
- `channels.jsonl`: private research route around unchanged Core and unchanged
  Bun receiver internals. Does not assert a native public Channels API exists.
  Volatile, commit, burst, no-credit and SDK-window cases are distinct workloads.
- `locks.jsonl`: real public Core version CAS/grouped writer; synchronous caller
  threads, 10-second TTL. Delayed-commit correctness demonstrates why the
  public CAS prototype is not a complete correct short-lease implementation.
- `outbox.jsonl`: SQLite source plus Tantivy durable watermark/read barrier,
  and matched native compact FTS. Independent single-index dispatch; not a
  general cross-engine transaction protocol.
- `allocations.jsonl`: Rust System allocation requests only; excludes C/mmap,
  includes queries/oracles and is not a phase-isolated allocator measurement.
- `memory.jsonl`: a disposable 64 MiB cgroup with swap disabled. Source copying
  runs inside that cgroup so private source pages are charged there; existing
  shared executable/image pages retain Linux first-touch accounting. The limit
  is a cgroup allocation/cache budget, not a mathematical per-process RSS cap.
- Process-crash probes use SIGKILL after a handshake, not power-failure tests.
  Successful recovery is checked against a fresh index from current source.
  Small definition-change probes append a definition marker; they do not test
  arbitrary tokenizer/DDL changes. The large probes repeat resume/switch only.

`preflight/` retains interrupted/failed/superseded experiments. In particular,
missing floor and an unkeyed postings DELETE were fixed before main static.
The initial rebuild stride 16 sampled only narrow geo buckets; stride 17 fixes
that alias, includes checkpoint in builder holds and reader drain in duration.
The partial old series is retained but excluded from final comparisons. Earlier
correctness revisions are preserved; count each revision separately. No number
from a failed or superseded cohort establishes a candidate's final throughput.
