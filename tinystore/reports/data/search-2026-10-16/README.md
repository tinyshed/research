# Search evidence ledger

Research-only native CLIs; product Core and public API unchanged. Existing
TinyStore Rust architecture is pinned to `76b844c`; these CLIs link rusqlite's
bundled SQLite, not a new public Core Search module. Environment and corpus
version/hashes are in adjacent JSON. Natural text is fetched and ignored.

Build and run inside the existing documented Linux Docker lab, disk-backed
`/data`, target/cache volumes, 2 CPU quota and 6 GiB. Preserve proxy/CA settings
for fetch/build. Do not overlap timings with builds/tests or other experiments.
From `<repo>/tinystore/search-lab`:

```sh
python3 corpus.py
python3 gold.py
cargo build --release --locked
python3 run.py checks
sh sizes.sh
python3 run.py quality
python3 run.py analyzer
python3 run.py concurrent
python3 run.py scale
```

Drivers record exact commands, source revision, binary SHA-256, UTC, exit status,
full quality counters and individual returned IDs. Main timings use the full
all-feature CLI; size profiles are separate optional-feature artifacts.
Shared analyzer source for those binaries is `f79080d`; later revisions add
read-only reduction/docs and a separately compiled standalone-store control.
Initial correct callback checks preceded query/document-role expansion; the
final gold probes exercise the expanded modes. One failed prepare-only probe is
retained in correctness.jsonl; query execution is the corrected registration test.

`retrieval.jsonl` expects 54 runs: two passes of 27 scenarios, forward/reverse
candidate order per corpus. RU/EN/ZH each have 200 questions; mixed has 600.
`gold.jsonl` has 12 one-pass diagnostics on an original small fixture, not an IR
population or throughput comparison. `analyzer.jsonl` expects 12 paired samples,
`concurrent.jsonl` four paired writer/read-load controls, `scale.jsonl` four
paired 100k repeated-pool controls (60 queries, not valid population-quality).
The 1m scenario is measured by the complete authoritative-store control below,
including stored document bytes and hydration, rather than repeating an external
postings-only 1m cross-product.
Jieba's naive whole-text integration differs in multi-position phrase/OR
interpretation between adapters; exclude it from matched backend claims.

The primary Tantivy arm maintains an independent postings index over a SQLite
source. Its source/enqueue latency and separate commit/reload total are reported
separately. This is not an atomic source/index outbox implementation. The later
[`search-store-lab`](../../../search-store-lab/README.md) controls compare actual
authoritative standalone collections with full stored fields and hydration:

```sh
# Only after the main series stops:
cd <repo>/tinystore/search-store-lab
cargo build --release --locked
python3 run.py
cd <repo>
python3 tinystore/search-lab/summarize.py
```

The declared authoritative-store cohort is eight processes: 100k and 1m rows,
FTS5 and Tantivy, two passes in opposite engine order. The frozen binary source
is `31ec988e5d061a4a2ba82e1151f8a951ef8625cf`; it imports the unchanged analyzer
from `f79080d22b4071adb88aa6d282e946079bc925d9`. Each record carries its binary
SHA-256. Main analyzer/concurrent/scale driver revision is `4ad47b9`; it reuses
the same `f79080d` binary. Initial lifecycle correctness probes predate that
binary and retain their own source revisions and hashes.

After all timings stop, run the evidence checks and untimed diagnostics in the
same lab (they never rebuild or replace the frozen binaries):

```sh
cd <repo>
python3 tinystore/search-lab/audit.py
python3 tinystore/search-store-lab/layout.py
python3 tinystore/search-lab/verify.py
python3 tinystore/search-lab/summarize.py
```

`postflight-audit.json` contains a paired relevance bootstrap on the fixed mixed
query set and an original literal/internal-token collision fixture. Its tiny
query timings are excluded from performance claims. `storage-layout.json`
attributes the 100k post-churn SQLite file by dbstat object/pages/payload/unused
space and the Tantivy directory by file; its reruns are structural inspections,
not extra timing passes. It does not decompose the original bulk-import totals
or extrapolate to one million rows. `verification.json` checks cohort inventory,
paired passes, binary identity, matched membership and independently recomputes
relevance from raw returned IDs and fetched qrels. A pass of this evidence
verifier is not a product correctness or crash-safety certificate.

The old index sweep was stopped for this owner-directed priority change;
completed old raw measurements remain in their original ledger. No benchmark
results from the interrupted old candidate are included here. `preflight/`
retains the first small gold probe with duplicate unused analyzer initialization;
its memory/setup are excluded from the final comparison.
