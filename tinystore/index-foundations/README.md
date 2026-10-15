# Index foundations and integration follow-up

Research-only executables. No TinyStore source or public API is changed.
The Rust dependency pins `rust` at `76b844cdc8c3410e746a5d5dc3e9aafec83c9962`;
the historical Go submodule is deliberately unchanged.

## Questions registered before measurements

| Owner | Existing access structure | Possible new need | Required consistency |
|---|---|---|---|
| SQL | SQLite primary/secondary/partial/expression indexes | FTS and spatial predicates on application tables | Same SQLite snapshot and commit |
| KV | `(bucket,path)` WITHOUT ROWID, expiry index, clear markers, version | Explicit projections from selected typed values | Must define codecs, encryption, branches, expiry first |
| Records | Packed segments/blocks, time bounds, trace/value Bloom filters | Search within retained events | Stable event identity and retention-aware deletion |
| Metrics | Label postings, series identity, time groups, compressed payloads | Existing label/time predicates already indexed | Series/label lifecycle, no per-sample generic index |

Hypotheses, not selections: native B-tree probably covers ordinary SQL secondary
indexes; native FTS/RTree may have the lowest atomicity cost; external Tantivy
may pay for its separate commit protocol with faster text search; in-memory
RTree/bitmap caches may be attractive only when rebuild/startup memory is allowed.
No generic Index Engine or public index API is assumed necessary.

Candidates: B-tree vs relational postings vs Roaring; FTS5 contentful and external
content vs Tantivy with identical pre-normalized term queries; SQLite RTree vs
SQLite grid vs rstar. Scans are semantically matched baselines, never LIKE vs
MATCH. All searches materialize and sort every matching ID, without ranking.

Online candidates: monolithic transaction; fixed high watermark and key frontier
with transactional dual-write coverage; seen-ID backfill with eager indexed
changes; dirty-ID capture with final reconciliation. A generation's definition is
immutable: superseding requests abort and start a different generation.

Time limits are soft, checked between rows, and cannot bound COMMIT/fsync. WAL
frame targets adapt the next chunk and are not hard bounds on distinct dirty
pages. Overruns are reported. Old generations remain until readers/direct SQL
can be retired; the extra disk is deliberately visible.

## Reproduce

Use Linux, Rust 1.99.0, two CPU quota and six GiB memory. Put database files on
a disk-backed volume, not `/tmp`. Preserve the environment's proxy and trusted
CA. Build before timing and keep the machine quiet. Corpus is research-only
CC BY-NC-SA 4.0; fetch, never commit it:

```sh
python3 corpus.py
cargo build --release
RUSSIAN_CORPUS=<corpus>/russian.txt BENCH_DB=<scratch>/case.db \
  target/release/index-foundations rebuild fts frontier 100000 russian 2000 64 selective 1 10 5000
BENCH_DB=<scratch>/check.db target/release/index-foundations rebuild-check geo seen
```

`run.py` executes serial fresh processes, two interleaved passes, records exact
commands, revisions, UTC timestamps and failures. Checkpointed fixture templates
are copied before each candidate; template creation is excluded from timings.
`prepare source|fts|geo N SHAPE ordered|shuffled` builds those templates.

RSS includes the SQLite source load where a cache needs it. Rust allocation
counters are opt-in diagnostics (`--features alloc-metrics`); they exclude C and
mmap, and their timings must not be mixed into ordinary runs. WAL hooks read
the WAL salt per commit to distinguish recycling; this observer overhead is
common to native candidates. WAL bytes estimate physical replication input,
not a measured replication implementation.

The public Core KV CAS integration uses the actual product's grouped commits.
Its research lease row is never TTL-deleted, preserving fencing counters. The
FIFO prototype uses condition variables and is intentionally simple; per-resource
admission must not become one global lock. Successful completed wait samples
exclude canceled requests, which are counted separately.
