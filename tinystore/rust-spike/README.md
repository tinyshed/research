# Rust algorithm experiment

Local prototype against TinyStore commit
`e307c48a40126aad0e2873b6bf3aaedef8115483`.
It compares isolated algorithms, not a rewritten storage engine.

The Go baselines retain the source algorithms. Rust implements their bit and
ordering contracts. Alternative packing readers and writers exist in both
languages; the ingestion typed-sort case changes Go's sorting implementation.

- Metrics: change-value encoding/decoding and fixed-width residual packing,
  240 samples per block.
- Records: fixed-width and Rice packing, 1024 integers per block column.
- Ingestion preparation: ordered, shuffled and duplicate timestamps, 240 or
  4096 arrivals. Validation, label canonicalization and SQLite are excluded.

All 54 cases compare canonical input fingerprints and every output byte.
Tests cover float bits, last-arrival-wins ordering, overflow, truncation,
padding and shifts by 64. Records encoding uses fresh output ownership;
production Rice encoding reuses scratch, which this prototype does not model.

## Reproduction

Use Linux with Go 1.27.1, Rust 1.99.0, Python 3, `taskset` and GNU `time`.
There are no third-party Go, Rust or Python dependencies. Keep the machine
quiet while timings run.

The directory layout is `tinystore/source`, `tinystore/rust-spike` and
`tinystore/reports`. In a research checkout, select the source commit:

```sh
cd "<repo>"
git submodule update --init tinystore/source
git -C tinystore/source checkout e307c48a40126aad0e2873b6bf3aaedef8115483
cd tinystore/rust-spike
GOWORK=off CGO_ENABLED=0 python3 run.py --passes 5 --seconds 0.15
```

`run.py` builds ordinary release binaries and a separate Rust allocation
instrumentation binary, runs tests, verifies output, then alternates Go/Rust
passes on one CPU with `GOMAXPROCS=1`. Timings include fresh output ownership
and normal reclamation: immediate Rust destruction and amortized Go GC.

Memory-only processes retain equal logical output bytes after a short
warmup; Go snapshots are taken after GC. Both programs construct fixtures
before filtering, discard unrelated cases, and report their baseline RSS.
Peak RSS includes startup and construction. Retained harness metadata is
reported separately because its Go/Rust representation differs.

Allocation counts are measured separately. Go's `TotalAlloc` measures rounded
heap allocations; Rust's allocator records requested sizes, counting every
reallocation request in full. Neither is live heap size or total RSS.

The measured harness was local and uncommitted. Source content hashes in
`environment.json` pin that preliminary experiment. Publishing its preserved
artifacts does not reclassify it as a formal measurement round or satisfy the
committed-harness requirement retroactively.
