# Current metrics layout experiments

This is a layout experiment over production TinyStore
`e307c48a40126aad0e2873b6bf3aaedef8115483`, not a production migration or an
alternate value codec. Go creates stores through the public API, maintains
them, closes them, and verifies every sample through `metrics.Read` using
timestamp and IEEE-754 bits. Durable quiet heads remain part of every total.

`run.py baseline` writes common immutable inputs for this and the companion
payload experiment. `blocks.jsonl` carries complete original block heads,
clock residuals, sealed value bodies, exact summary encodings, original
addresses and lossless bit fields; `groups.jsonl` carries the original v4
directory and shared clock envelopes. A manifest stores independent Go
per-series SHA-256 over concatenated little-endian timestamp/value-bit pairs.
The corpus, stores and exports belong on a Linux volume and are not committed.

The first matrix preserves original metadata bytes for rowid and page-size
changes. Other variants split original groups at 8 or 16 slots and vary inline
thresholds. They preserve every immutable microblock byte, clock residual,
exact summary and existing external payload address. A separate native-zstd
32/16 control charges any directory recompression effect. Inline limits other
than 16 use experimental directory version 5, which stores its limit in the
header and rejects the production v4 decoder. No 64-slot format is implemented.

Every database retains registry, state, head, migration and all indexes.
Rowid groups retain the composite primary-key UNIQUE index, and its pages are
charged. Native writes use stock rusqlite 0.40.1, external static SQLite 3.53.4,
FULL WAL, a 1-MiB cache and the original fsync/checkpoint settings. Metadata
compression uses pinned zstd 0.13.3 / zstd-sys 2.1.0 + zstd 1.5.7. Value bodies
never pass through an encoder in this experiment.

The matched native SQLite omits `dbstat`. Physical scans use Python's builtin
SQLite `dbstat` read-only with `immutable=1`, after checkpoint and closure.
Every object's page total is independently cross-checked with production
`internal/dbstat`. Engine reads/writes and timings never use Python SQLite.

From Linux, after preparing the official normalized corpora and matched native
SQLite archive as in `../sqlite-bench/README.md`:

```sh
cd <repo>/tinystore/metrics-layout-bench
export GOWORK=off CGO_ENABLED=0 GOMAXPROCS=1
export SQLITE3_STATIC=1
export SQLITE3_LIB_DIR="$PWD/../sqlite-bench/generated/lib"
export SQLITE3_INCLUDE_DIR="$PWD/../sqlite-bench/generated/sqlite-amalgamation-3530400"
export CARGO_TARGET_DIR=<work>/metrics-layout-target
mkdir -p <work>/metrics-layout
go -C go build -trimpath -o <work>/metrics-layout/go-input .
cargo build --release --locked --manifest-path rust/Cargo.toml
cp "$CARGO_TARGET_DIR/release/tinystore-metrics-layout-bench" <work>/metrics-layout/layout
<work>/metrics-layout/go-input --out <work>/metrics-storage-input/preflight --dataset edge --samples 721 --series 4
TINYSTORE_STORAGE_INPUT=<work>/metrics-storage-input/preflight cargo test --release --locked --manifest-path rust/Cargo.toml
python3 run.py baseline --work <work>/metrics-layout --input <work>/metrics-storage-input --datasets tsbs alibaba regular irregular nonsparse edge
python3 run.py matrix --work <work>/metrics-layout --input <work>/metrics-storage-input --datasets tsbs alibaba regular irregular nonsparse edge
# This helper runs provenance, bounded pilots, all six-pass stages, RSS,
# production Go cross-reads and the report after the complete matrix exists.
sh finish_study.sh
```

Use an isolated quiet window for every measurement stage. Six passes alternate
variant order and hold trace/count constant; read loops use one series at a
time. These synchronous layout traces omit production admission, cancellation,
pooling, locks and background scheduling. They measure local ratios under
Docker/WSL2, not bare-Linux latency or concurrent production throughput.

Retention deletes blocks strictly before the cutoff, keeps cut blocks, updates
live masks and clock ownership, and preserves surviving external addresses.
Its visible hashes are compared before/after with the same clipped ranges.
It is one synchronous transaction over groups, not the production indexed
maintenance scheduler. Closed growth/free-page state is reported separately
from `VACUUM`; neither is a promise of byte erasure.
