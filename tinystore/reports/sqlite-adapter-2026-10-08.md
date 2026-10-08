# SQLite adapter and rusqlite followup — 2026-10-08

This exploratory followup tests selected ideas from the
[low-level SQLite proposal](rust-research-summary-2026-10-08.md#low-level-sqlite-extension-plan-unmeasured).
It separates copying/allocation changes in the metrics prototype, SQLite
lookaside configuration, raw statement loops, and small rusqlite source patches.
Production TinyStore is unchanged. These observations do not establish a
production migration, a shared memory budget or concurrent server throughput.

The tested adapter and rusqlite changes did not establish a general metrics
speedup. Allocation count fell slightly without an RSS saving. An isolated
mutable-head read improved with a much larger lookaside pool, but the observed
difference mostly appeared after snapshot release and interacted with a tiny
metadata allocation. The evidence does not justify a dependency fork or a
larger default native pool on performance grounds.

| Question | Observation | What follows |
| --- | --- | --- |
| Does a batch arena pay here? | 4.4–4.8% fewer Rust allocation requests, slightly more requested bytes, overlapping query timings and no useful RSS reduction. | Keep the experiment; do not enable it as a general optimization. |
| Is there wrapper overhead? | Raw selected-row loops were 1.12–1.17× faster in the initial session. | Measure complete engine paths before replacing safe APIs. |
| Do inline/column-count patches remove it? | Usually only a few percent in either direction on complete operations; they did not close the raw-loop gap. | No demonstrated performance case for these patches as a default fork. |
| Should lookaside grow? | One head-read case improved about 19%, while the larger pool reserved another 418.25 KiB per reader/writer pair. | Separate buffer-placement effects from SQLite allocation work before tuning defaults. |

## Environment and reproduction

Both sessions ran on the same local AMD Ryzen 7 7700 host through Docker
Desktop/WSL2, Linux 6.18.33.2, glibc 2.41. The container exposed 16 logical CPUs
and no cgroup CPU/memory cap. Timed processes were pinned to logical CPU 0.
Two-thread execution was checked for correctness separately. Assistant-driven
builds and tests completed before timing. Host background activity is not a
controlled bare-Linux environment; interpret same-session ratios as local
exploratory evidence, not deployment latency promises.

The tools were Go 1.27.1, Rust 1.99.0 and the existing release profile without
Rust LTO/PGO or native CPU targeting. The C archive retained the pinned SQLite
3.53.4 source and `-O2` flags, native THREADSAFE=1, NO_MUTEX connections and
the existing schema, SQL, 1 MiB cache, WAL/FULL and PRAGMAs. Rusqlite was 0.40.1.
The production source was pinned to
`e307c48a40126aad0e2873b6bf3aaedef8115483`; the original optimized Rust source
was archived from research `8955ce22b6ce716317bff0cd937b0a9ef3008af5`.

The deterministic Go-generated fixtures and operation traces are those in
`metrics-max-bench`: 16 sealed series, 64 wide series, mutable heads, scrape,
ready-to-seal and IEEE edge fixtures. These fit the configured cache. Every
child receives a fresh copy of a closed fixture on a Linux Docker volume.
Calibration chooses one operation count per case from the current-session
baseline, then all variants receive identical warm/timed traces. Results have
equal consumer lifetimes within these Rust comparisons. Six passes rotate and
reverse variant order. Writes are also cross-read through production Go.

At the user's explicit request, candidate harnesses were uncommitted while
collecting measurements. Exact source, crate-patch and executable hashes are
retained. Preserving source after collection does not retrospectively make this
a committed-harness round. The two sessions have independent same-session
baselines; do not combine their medians into a new speedup.

The measured adapter and fork-builder source is retained in research commit
`6dc05b2257779161dd5c2b7c758aef09f9aff23f`, created after collection.

Reproduction and switch definitions are in
[SQLITE-ADAPTER.md](../metrics-max-bench/SQLITE-ADAPTER.md). The first session's
[environment](data/sqlite-adapter-2026-10-08/environment.json),
[all passes](data/sqlite-adapter-2026-10-08/passes.md) and
[summary](data/sqlite-adapter-2026-10-08/summary.json) preserve configuration,
UTC interval, counts and hashes. The dependency followup has its own
[environment](data/rusqlite-fork-2026-10-08/environment.json),
[all passes](data/rusqlite-fork-2026-10-08/passes.md),
[summary](data/rusqlite-fork-2026-10-08/summary.json) and
[minimal patches](data/rusqlite-fork-2026-10-08/build.json).

## Borrowing and an owned arena reduced allocation count, not whole-query cost

The metadata switch parses a borrowed `Row::get_ref()` BLOB while its row is
live. The arena switch copies batched external payloads into one exactly
reserved vector and shares owned ranges between blocks. Selection has already
charged their combined size to the byte budget. Payload decoding remains after
snapshot release. This preserves the required SQLite-to-owned-memory copy and
adds range metadata and shared ownership; it does not implement native memory
admission.

| Complete operation | Original optimized Rust, µs | Modified-source control, µs | Metadata + arena, µs | Original / candidate |
| --- | ---: | ---: | ---: | ---: |
| Read16 | 1607.23 | 1644.74 | 1677.01 | 0.96× |
| Read wide64 | 2214.45 | 2090.14 | 2066.11 | 1.07× |
| Stream wide64 | 1517.97 | 1539.39 | 1496.04 | 1.01× |
| Wide cut sum | 1296.61 | 1286.70 | 1276.62 | 1.02× |
| Scrape100, FULL | 4852.73 | 4770.22 | 4757.64 | 1.02× |

The wide-read difference includes changes already present in the control; the
candidate/control difference is only 1.01×. Arena-only against control was
1.01× for Read16 and 0.98× for wide Read. Pass ranges overlap and this does not
establish a useful general engine speedup.

The separate instrumented process counted 7,495 → 7,162 Rust allocation
requests per Read16 and 9,305 → 8,860 per wide Read, reductions of 4.4% and
4.8%. Requested bytes increased from 2,264,915 to 2,275,131 for Read16 and
2,558,126 to 2,570,198 for wide Read: the arena/range representation consumes
metadata too. Rust counters exclude native SQLite/zstd allocations and are not
live-heap measurements. Three-process retained-result RSS passes remained near
24.6–24.7 MiB for Read16 and 29.0–29.1 MiB for wide Read. No meaningful RSS
saving was established. Full profiles and memory samples are retained in
[profiles.json](data/sqlite-adapter-2026-10-08/profiles.json) and
[memory.json](data/sqlite-adapter-2026-10-08/memory.json).

## Lookaside already served these cached reads

For Read16 the default reader reported 541 lookaside hits per operation, zero
size/full misses and 22 simultaneously used slots at high water. For wide Read
there were 946 hits, three size misses and zero full misses. These are the
SQLite counter high-water fields divided by operation count; their current
fields are zero by API definition.

Disabling lookaside reduced tracked SQLite memory by 93.75 KiB for the two
connections. Configuring 512-byte slots × 512 increased it by 418.25 KiB:
Read16's connection pair used 372.70 KiB by default, 278.95 KiB disabled and
790.95 KiB with the larger pool. The larger setting reported ten size misses
per Read16; increasing reserved bytes did not improve every fit.

There was no broad query/write speedup from the larger pool. The initial
mutable-head read showed an isolated difference (248.00 µs original versus
202.16 µs with the larger pool), while combining it with metadata borrowing
took 261.88 µs. The dependency session repeats this case rather than declaring
the isolated result a production default.

## A raw loop isolated some wrapper cost

The safe and raw probes hold independently prepared PERSISTENT statements
outside the timer, execute identical SQL and retain the same owned
`(id, Vec<u8>)` rows. Raw access preserves type checks, reset/error handling and
connection ownership. These are selected payload rows rather than a public
metrics query. The initial six paired passes gave:

| Rows | Safe rusqlite, µs | Raw C statement, µs | Safe / raw |
| --- | ---: | ---: | ---: |
| 1 | 2.240 | 2.160 | 1.04× |
| 64 | 7.680 | 6.565 | 1.17× |
| 240 | 19.460 | 17.375 | 1.12× |

VM-step counts and statement memory agreed between safe and raw cases. A raw
loop still includes SQLite, allocation and copying; these ratios isolate the
selected access paths, not every rusqlite facility. Whole-engine gains cannot
be inferred by applying them to complete-query latency.

## Small rusqlite patches did not establish a general improvement

The dependency followup built three variants of locked rusqlite 0.40.1:

- An `#[inline]` hint on the internal `Statement::value_ref` conversion.
- A lazy column-count cache invalidated before every `step` and `reset`,
  preserving fresh metadata after SQLite reprepares a statement.
- Both changes together.

The library's checked column/type access, native SQLite configuration, engine
source and result ownership were held equal. Every other locked dependency
was checked unchanged. No API or ABI was added. The original prototype was
remeasured as an additional baseline, while the modified-source stock
dependency (`control`) is the appropriate baseline for each library patch.

| Complete operation | Stock dependency, µs | Inline, µs | Count cache, µs | Both, µs | Stock / both |
| --- | ---: | ---: | ---: | ---: | ---: |
| Head point | 12.11 | 12.09 | 12.59 | 12.14 | 1.00× |
| Head full8 | 257.36 | 268.06 | 248.98 | 242.81 | 1.06× |
| Sealed Read16 | 1632.94 | 1650.20 | 1624.38 | 1606.69 | 1.02× |
| Read wide64 | 2034.18 | 2044.64 | 2071.74 | 2079.69 | 0.98× |
| Stream wide64 | 1500.28 | 1530.56 | 1526.79 | 1495.04 | 1.00× |
| Whole-block sum | 133.54 | 133.89 | 137.65 | 135.52 | 0.98× |
| Wide cut sum | 1296.59 | 1266.76 | 1266.48 | 1263.70 | 1.03× |
| Scrape100, FULL | 4940.20 | 4925.36 | 4974.19 | 4949.86 | 1.00× |
| Seal ready | 30607.74 | 30832.69 | 31552.96 | 31138.65 | 0.98× |

For Head full8 the original-source binary took 249.25 µs in this session, so
the combined patch's difference from that additional baseline was 1.03×.
These small, mixed changes on overlapping local-VM passes do not establish a
general improvement. The row-count/inline hypotheses were narrower than all
possible rusqlite optimizations; rejecting these patches is not evidence that
every possible wrapper change is useless.

In the followup selected-row probe, stock safe/raw medians were 7.99/7.00 µs
for 64 rows and 19.39/16.77 for 240 rows. The combined patch gave 7.93/6.95
and 19.46/16.18. Inline-only and count-only safe paths moved by only about
1–2% on these cases. The raw gap remained, so these patches did not establish
repeated column-count calls or the conversion inline boundary as its dominant
cause. Every wrapper pass is in
[wrapper-summary.json](data/rusqlite-fork-2026-10-08/wrapper-summary.json).

The first session's instrumented Read16 spent 0.075 ms/op in payload fetch
out of 1.654 ms/op for the complete operation, and 0.334 ms/op in snapshot
fetch as a whole. These inclusive wall-time scopes are nested and cannot be
added as independent CPU costs. They explain why a selected-row wrapper ratio
alone is insufficient evidence of a large engine opportunity.

## The head/lookaside interaction needs a different explanation

The second session repeated the isolated head result: original-source Head
full8 was 249.25 µs, larger lookaside 202.25 µs (about 19% lower latency),
and larger lookaside plus metadata borrowing 258.49 µs. This is a repeatable
local observation, but its interaction with an eight-allocation metadata
change argues against treating it as a generic SQLite speedup.

Three separate instrumented diagnostic passes found the difference mainly in
post-snapshot processing. In the first two passes, control snapshot time was
30.55/30.26 µs and larger-lookaside snapshot time 29.68/32.15; read processing
was 214.56/214.00 versus 170.22/182.89. The third pass was noisier but kept the
same direction. Metadata borrowing plus the larger pool returned read
processing to 212.38/213.32 µs in the first two passes. Allocation counts were
750/op for control and larger lookaside, 742/op when metadata was borrowed.
Full diagnostic passes are in
[head-profiles.json](data/sqlite-adapter-2026-10-08/head-profiles.json).

An indirect allocator/buffer-placement effect is a hypothesis supported by
where the time moved; the mechanism is not isolated. A future experiment
should control allocation layout/alignment and use independent head fixtures
before paying the extra native memory or making lookaside a default.

## What follows

Retain stock rusqlite and the current lookaside configuration as this
prototype's performance baseline. The arena and dependency patches remain
isolated experiments. For further work, prioritize measured engine allocation
and decode/processing costs, and the repeatable head-buffer placement
interaction. A broader fork, unchecked column conversion or custom native
allocator needs a separately measured requirement. Native accounting and
admission can still be useful correctness features; this round did not test
them or establish a speed benefit for them.

## Validation and scope

The adapter source passed 43 release-mode Rust tests, including shared bytes
after statement/connection destruction, moving owned ranges to a worker,
missing/duplicate/wrong-size payloads, checksum failure, raw reset after a
conversion error and retained results after connection close. Both runners
checked 56 traces, error guards and selection/resource plans, plus forced
two-thread execution and Go cross-reading of writes. Format, float bits,
limits, result checksums and final logical digests matched the original Rust
prototype on those fixtures.

Stock rusqlite's upstream library tests have 11 failures under this pinned
`SQLITE_DQS=0` build because they use double-quoted string literals. All three
patches retain exactly that failure set: stock has 166 passing tests, and each
patch has 168 after adding the schema-reprepare/reset regressions. The tests
were not skipped or rewritten. The [stock log](data/rusqlite-fork-2026-10-08/upstream-stock-tests.txt)
and three patch logs remain available beside the source diffs. This is not a
claim that the complete upstream suite passed.

The schema-reprepare guard was also proven by removing both cache
invalidations: it failed on the stale column count, then passed after restoring
the measured source byte for byte. The
[guard proof](data/rusqlite-fork-2026-10-08/guard-proof.json) retains both test
outputs and the restored source hash.

No Windows/macOS native run, independent production corpus, mixed-client
throughput, concurrent WAL/checkpoint growth, injected native allocator OOM,
shared budget, static parameter binding, C compiler variant or custom VFS was
evaluated. No storage-layout change was made and no file-size saving is
claimed. These are still separate gates from the proposal.
