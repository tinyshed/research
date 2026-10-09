# Current metrics group, directory, inline and page layout — 9 October 2026

This exploratory round measures the current published engine, TinyStore
`e307c48a40126aad0e2873b6bf3aaedef8115483`. Its existing format already has
WITHOUT ROWID groups, 32 slots of up to 240 samples, 16-byte inline values,
shared clocks and v4 directories with exact summaries and predicted fields.
Those features are the baseline, not new proposals. No production code or
migration changed, and no historical prototype density is used as a comparator.

| Question | Answer | What follows |
| --- | --- | --- |
| What does the current engine actually store? | TSBS: 4,665,344 bytes / 5,090,400 samples = 0.916499 B/sample. Alibaba: 7,843,840 / 12,431,885 = 0.630945. Both include durable heads, registry and every index. | Start from the complete contemporary file, not an old value-codec figure. |
| Does replacing WITHOUT ROWID groups help? | With original directory/clock/value bytes, Alibaba's freshly rebuilt control falls from 6,774,784 to 5,324,800 bytes, 21.40%; TSBS grows 0.45%. | A real corpus-dependent layout opportunity, with the UNIQUE index charged. |
| Is a larger inline threshold automatically better? | No. Rowid + inline32 reaches 5,230,592 bytes on Alibaba; 64/128 are larger there. Rowid + inline64 wins the deterministic irregular fixture. | Treat thresholds and row layout together. |
| Do smaller pages or groups win everywhere? | No. 4-KiB pages remain best on both public corpora for the rowid layouts; cap16 is a competitive Alibaba alternative but costs more clocks/groups. | Keep measured counterexamples and query/maintenance costs beside density. |
| Is the original-file-to-fresh-control saving a format gain? | No. Alibaba's unchanged head/state rows occupy 1,253,376 bytes originally and 188,416 after fresh insertion; both files have zero freelist pages. | Separate insertion/occupancy history from layout savings. |

## Environment and reproduction

Linux amd64 in Docker Desktop / WSL2 on an AMD Ryzen 7 7700, 16 logical CPUs
visible, cgroup `cpu.max=max 100000`; no CPU affinity or quota reduction.
Go 1.27.1; Rust 1.99.0 / LLVM 23.1.1; Python 3.13.5.
The engine uses stock rusqlite 0.40.1 against external static SQLite 3.53.4
(the matched original archive), FULL WAL, 1-MiB SQLite cache, foreign keys,
fullfsync and checkpoint_fullfsync enabled, auto-checkpoint 1,000 pages,
trusted_schema off and mmap off. Metadata uses zstd 0.13.3 / zstd-sys
2.1.0 + zstd 1.5.7. Native value compression is never invoked for a variant.

Raw provenance contains complete source hashes and compiler/archive settings:
[phase 1](data/metrics-layout-2026-10-09/phase1-provenance.json),
[phase 2](data/metrics-layout-2026-10-09/phase2-provenance.json), and
[final performance binary](data/metrics-layout-2026-10-09/provenance.json).
Go fixture binary SHA-256: `94c40dc36e3aee8576df3335421a3dc0ccfc19e9e517e8f3189d58624b3d38d1`.
Final native binary SHA-256: `76d2fe4bd31b7913d3b70a6070da51010e29571607028d3f6ecde9dfbab55195`.
The user waived the premeasurement harness-commit rule; this round is explicitly
exploratory and retains exact source/binary hashes rather than citing a
premeasurement harness commit.

Measurements ran serially with the companion payload experiment paused.
Read passes ran from 2026-10-09T10:36:38Z to 2026-10-09T10:37:07Z; each cell has six passes,
forward/reverse variant order alternating. Handles are new each pass; the OS
file cache is warm, not a cold-device test. Synchronous local ratios are not
bare-Linux latency or concurrent production-throughput promises.

Reproduce using [the harness commands](../metrics-layout-bench/README.md),
placing stores and corpora on a Linux volume. Prepare the normalised inputs as
`<work>/metrics-storage-input/corpus/{tsbs,alibaba}-series.jsonl`, then run
`baseline`, the 21-variant `matrix`, `pilot`, `timing`, `publication`,
`retention`, `rss`, `crossread` and `provenance`. Use `--datasets tsbs alibaba
irregular` and `--variants exact rowid native_control rowid_native_control
cap16 rowid_inline32 rowid_inline64` for the performance stages. The baseline
and density matrix also include regular, nonsparse and IEEE-edge fixtures.

TSBS DevOps uses generator revision
`8323e59c74027b108f4ad5ec5d3e498b0101a02e`, seed 123, scale 20, ten-second
intervals from 2026-01-01T00:00:00Z through 07:00:00Z: 2,020 series,
5,090,400 samples, normalised SHA-256
`e4f502af7b0b2ff2c4dba92057a8f2b95e636882f3cb9900986e013d189572cf`.
Alibaba uses the official public 2018 machine trace, every sixteenth machine,
seconds strictly before 172,800: 248 machines, 1,240 series, 12,431,885 samples,
no duplicate timestamps. Normalised SHA-256
`7ace58aaba7b3c34efc34c41585014521121c76aa991646c30735127a619afdd`;
[archive, selector and converter provenance](data/metrics-layout-2026-10-09/alibaba-corpus-provenance.json).
Neither corpus is committed. All corpus metrics are gauges; deterministic
fixtures also include counters and IEEE edge patterns.

## Actual production baseline first

The public Go API ingests one series at a time in batches of at most 7,680,
maintains after each batch and to quiescence, then closes. The clock is fixed
after the fixture's newest time. No wall-clock operation forces a quiet head
to seal. Go verifies every timestamp/value bit through `metrics.Read`; native
decoding then independently matches those per-series SHA-256 hashes.

| Fixture | All samples | Head samples | Closed file B | File B/sample | Sealed value body B | Stored directory B | Unique clock B | Head B |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| tsbs | 5,090,400 | 242,400 | 4,665,344 | 0.916499 | 2,663,060 | 417,158 | 136 | 198,187 |
| alibaba | 12,431,885 | 154,685 | 7,843,840 | 0.630945 | 2,792,592 | 1,303,051 | 220,841 | 112,411 |
| regular | 491,552 | 32 | 86,016 | 0.174989 | 0 | 10,294 | 786 | 992 |
| irregular | 491,552 | 32 | 544,768 | 1.108261 | 96,256 | 55,717 | 212,394 | 992 |
| nonsparse | 491,552 | 32 | 4,648,960 | 9.457718 | 3,295,691 | 127,199 | 786 | 992 |
| edge | 5,768 | 8 | 69,632 | 12.072122 | 1,450 | 765 | 42 | 248 |

Sealed values include their original per-block CRCs and inline bytes; they
exclude first values kept in directory fields. Stored clock bytes are unique
shared envelopes. Per-block residual clocks below count their logical copies,
so they must not be added to unique clock bytes. Expanded directory fields
describe the precompression representation and must not be added to stored
directory bytes:

| Fixture | First-value field B | Summary scalar/flags/resets B | Exact sum/increase B | Value-length B | Payload-address B | Inline value B | Logical clock residual B |
| --- | --- | --- | --- | --- | --- | --- | --- |
| tsbs | 60,586 | 198,586 | 139,388 | 22,400 | 24,153 | 4,880 | 0 |
| alibaba | 109,997 | 617,275 | 420,583 | 59,116 | 86,777 | 200,299 | 424,460 |

Physical `dbstat` cell payload includes record headers and scalar columns as
well as BLOBs. Its btree/index/overflow pages are a separate accounting view.
For every file, object page bytes plus freelist bytes equal the whole file,
with no overlap. Python SQLite 3.46.1 reads `dbstat` through
an immutable, read-only connection because the matched engine archive omits
the virtual table. Production `internal/dbstat` independently agrees with
every object's page total. Python SQLite never writes or times the engine.

| Fixture | Layout | Groups B | Groups UNIQUE index B | Payloads B | Clocks B | Head/state B | Other objects B | Freelist B |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| tsbs | baseline | 520,192 | 0 | 3,076,096 | 4,096 | 446,464 | 618,496 | 0 |
| tsbs | exact | 520,192 | 0 | 3,076,096 | 4,096 | 319,488 | 614,400 | 0 |
| tsbs | rowid | 495,616 | 45,056 | 3,076,096 | 4,096 | 319,488 | 614,400 | 0 |
| alibaba | baseline | 3,063,808 | 0 | 2,973,696 | 262,144 | 1,253,376 | 290,816 | 0 |
| alibaba | exact | 3,063,808 | 0 | 2,973,696 | 262,144 | 188,416 | 286,720 | 0 |
| alibaba | rowid | 1,560,576 | 53,248 | 2,973,696 | 262,144 | 188,416 | 286,720 | 0 |

The original-to-fresh-control delta is insertion history: TSBS head/state
falls 126,976 bytes, Alibaba falls 1,064,960; minor registry/index differences
account for the rest. Head BLOB bytes and every logical state row remain
identical. This is not a retention deletion or a freelist reclamation, and it
is excluded from the layout percentage comparisons.

## The overflow cost is in the groups object

Alibaba's production groups object uses 3,063,808 bytes, including 1,904,640
bytes of overflow pages. Those overflow pages contain 312,228 payload bytes
and 1,590,552 unused bytes. The byte-preserving rowid version reduces the
groups object to 1,560,576 bytes and adds a 53,248-byte UNIQUE index; all other
charged objects remain the same as the fresh control. Its net saving is
1,449,984 bytes. TSBS has no groups overflow: its groups object shrinks
24,576 bytes while its new UNIQUE index costs 45,056, a 20,480-byte loss.
The measured object division establishes the cause; BLOB length alone would
miss the page-local payload/overflow boundary.

## Staged density matrix

`exact`, `rowid` and page-only variants copy original directories, clocks and
value bodies byte for byte. `native_control` re-encodes only group envelopes
using pinned native metadata compression. It saves 8,192 bytes on each public
corpus; that compressor effect is not attributed to a layout. The matching
rowid native control is also retained. Caps split existing groups without
joining unrelated original boundaries; no value block is rebuilt. Thresholds
other than 16 use a self-identifying experimental v5 directory with a stored
u16 threshold and a distinct decoder. Caps stay at 8/16/32; 64 would require
a separate mask/format design and was not implemented.

Every variant verifies immutable block head/time/value/summary bytes by a
canonical SHA-256, confirms surviving external payload IDs never move, and
matches all samples including durable heads. Registry, head/state and migration
rows are copied verbatim, with two-way SQL EXCEPT verification in the final
publication runs. Extra rowid indexes are always charged.

| Variant | TSBS file B | Alibaba file B | Irregular file B | Nonsparse file B |
| --- | --- | --- | --- | --- |
| exact | 4,534,272 | 6,774,784 | 520,192 | 4,571,136 |
| rowid | 4,554,752 | 5,324,800 | 520,192 | 4,440,064 |
| page1024 | 5,423,104 | 6,354,944 | 450,560 | 4,400,128 |
| page2048 | 5,203,968 | 7,204,864 | 546,816 | 4,411,392 |
| native_control | 4,526,080 | 6,766,592 | 520,192 | 4,571,136 |
| cap8 | 4,665,344 | 5,808,128 | 659,456 | 4,431,872 |
| cap16 | 4,526,080 | 5,414,912 | 532,480 | 4,579,328 |
| inline0 | 4,632,576 | 5,738,496 | 520,192 | 4,571,136 |
| inline32 | 4,530,176 | 6,660,096 | 520,192 | 4,571,136 |
| inline64 | 4,521,984 | 7,884,800 | 634,880 | 4,571,136 |
| inline128 | 7,200,768 | 8,515,584 | 634,880 | 4,571,136 |
| rowid_native_control | 4,546,560 | 5,312,512 | 520,192 | 4,411,392 |
| rowid_inline0 | 4,648,960 | 5,763,072 | 520,192 | 4,411,392 |
| rowid_inline32 | 4,550,656 | 5,230,592 | 520,192 | 4,411,392 |
| rowid_inline64 | 4,542,464 | 5,345,280 | 430,080 | 4,411,392 |
| rowid_inline128 | 4,579,328 | 5,443,584 | 430,080 | 4,411,392 |
| rowid_page1024 | 4,608,000 | 5,607,424 | 445,440 | 4,384,768 |
| rowid_page2048 | 5,093,376 | 5,480,448 | 464,896 | 4,405,248 |
| rowid_cap16 | 4,546,560 | 5,464,064 | 532,480 | 4,423,680 |
| rowid_cap16_inline64 | 4,542,464 | 5,304,320 | 450,560 | 4,423,680 |
| rowid_cap16_inline128 | 4,579,328 | 5,361,664 | 450,560 | 4,423,680 |

The strongest Alibaba combination is rowid/32 slots/inline32/4-KiB pages:
5,230,592 bytes, 0.420740 B/sample. Against the matching native WITHOUT ROWID
control (6,766,592 bytes), the layout gain is 22.70%; against the matching
native rowid/inline16 control (5,312,512), inline32 alone saves 81,920 bytes,
1.54%. It moves 5,105 blocks / 103,946 body bytes inline: payload pages fall
151,552 bytes and group pages grow 69,632. TSBS has no bodies in the 17–32-byte
interval, so the same change only charges the v5 header.

Larger thresholds are counterexamples: WITHOUT ROWID inline128 reaches
7,200,768 bytes on TSBS and 8,515,584 on Alibaba. Rowid avoids much of that
overflow but 64/128 still lose to 32 on Alibaba. Irregular clocks favour
rowid/inline64 instead (430,080 versus 520,192 fresh-control bytes). Small-page
wins in tiny constant/edge fixtures largely remove minimum root-page cost;
they do not justify changing the public-corpus default.

## Six balanced read passes

The point trace reads an existing first sample, rotating series without loading
the corpus. The range covers 1/16 of one series; the cut sum covers its middle
third. Whole sums use exact summaries where available and raw heads; cuts use
raw samples at block boundaries. Selected external bodies are fetched before
the snapshot ends; decoding and answer construction occur afterwards. Every
variant/pass yields the same trace hash. Raw-versus-summary checks cover the
first 16 series of each finite fixture; full sample checks cover every series.

A 32-operation pilot runs every variant; the slowest sets one shared count
per fixture/case, target 120 ms and maximum 4,096 operations. Projected read
time was 48.9 seconds. Counts and all pilot
values are in [pilots.json](data/metrics-layout-2026-10-09/pilots.json).
Ratios below are median time / median exact-control time, so below 1 is faster:

| Fixture | Trace | Ops/pass | Exact median µs/op | Rowid ratio | Cap16 ratio | Rowid inline32 ratio | Rowid inline64 ratio |
| --- | --- | --- | --- | --- | --- | --- | --- |
| tsbs | point | 417 | 20.35 | 1.051× | 1.006× | 0.989× | 0.955× |
| tsbs | range | 1238 | 20.90 | 1.065× | 1.003× | 0.996× | 1.009× |
| tsbs | whole_summary | 2835 | 25.67 | 0.967× | 1.034× | 1.021× | 1.009× |
| tsbs | cut_summary | 1639 | 26.23 | 0.958× | 1.034× | 1.056× | 1.050× |
| alibaba | point | 195 | 46.16 | 0.923× | 0.712× | 1.005× | 1.186× |
| alibaba | range | 838 | 61.47 | 0.963× | 0.821× | 0.935× | 0.890× |
| alibaba | whole_summary | 1105 | 66.69 | 0.997× | 1.047× | 0.974× | 1.022× |
| alibaba | cut_summary | 698 | 71.40 | 0.972× | 0.955× | 0.971× | 0.965× |
| irregular | point | 324 | 59.24 | 0.987× | 0.586× | 0.937× | 1.025× |
| irregular | range | 1059 | 80.61 | 1.079× | 0.743× | 1.019× | 1.046× |
| irregular | whole_summary | 889 | 110.58 | 0.971× | 0.973× | 0.955× | 1.014× |
| irregular | cut_summary | 715 | 140.95 | 1.003× | 0.630× | 0.943× | 0.978× |

Alibaba's actual six pass observations (µs/op, in pass order) make short-run
variation visible; the complete seven-variant observations are retained in
[timings.json](data/metrics-layout-2026-10-09/timings.json):

| Trace | Variant | Six passes µs/op |
| --- | --- | --- |
| point | exact | 184.38; 43.77; 52.84; 46.07; 42.73; 46.25 |
| point | rowid | 108.51; 42.66; 42.57; 50.01; 39.96; 41.34 |
| point | cap16 | 116.44; 32.52; 33.23; 42.58; 31.11; 31.72 |
| point | rowid_inline32 | 109.40; 42.11; 42.98; 50.38; 49.82; 42.33 |
| range | exact | 60.62; 66.80; 59.56; 62.31; 62.82; 58.36 |
| range | rowid | 71.11; 64.65; 61.64; 56.69; 56.33; 55.74 |
| range | cap16 | 53.52; 50.12; 50.58; 48.99; 55.98; 50.32 |
| range | rowid_inline32 | 59.88; 59.71; 52.77; 59.66; 55.07; 55.28 |
| whole_summary | exact | 67.74; 69.48; 65.87; 66.74; 64.80; 66.63 |
| whole_summary | rowid | 70.49; 66.81; 65.05; 63.73; 66.22; 67.49 |
| whole_summary | cap16 | 70.10; 64.98; 69.50; 77.61; 67.72; 71.11 |
| whole_summary | rowid_inline32 | 66.26; 62.28; 69.06; 63.69; 69.51; 62.18 |
| cut_summary | exact | 71.53; 69.59; 71.59; 71.26; 69.96; 73.97 |
| cut_summary | rowid | 69.66; 69.84; 69.09; 67.28; 67.76; 70.99 |
| cut_summary | cap16 | 67.44; 66.95; 68.95; 73.60; 67.19; 71.69 |
| cut_summary | rowid_inline32 | 70.92; 68.40; 67.78; 68.01; 70.31; 75.23 |

## Publication and partial retention

Publication is a native transaction creating the complete schema, copying the
same registry/state rows and publishing the immutable blocks, followed by
`wal_checkpoint(TRUNCATE)`. Its timer excludes input preparation and postwrite
proofs. It is a prototype fresh-publication cost, not production Go ingest:
byte-copy `exact`/`rowid` use SQLite `INSERT SELECT`; re-encoded variants also
pay metadata/envelope publication work. Compare those with native controls.
Retention includes metadata fetch, liveness updates, payload deletion, clock
reference release and the FULL commit. It excludes independent visible-hash
validation and the subsequent checkpoint. Six balanced milliseconds follow:

| Fixture | Variant | Six publication ms | Six retention ms |
| --- | --- | --- | --- |
| tsbs | exact | 72.7; 64.0; 60.9; 65.0; 64.0; 68.9 | 45.5; 46.1; 45.4; 50.9; 46.0; 46.5 |
| tsbs | rowid | 63.0; 61.2; 65.2; 59.7; 68.8; 60.3 | 44.4; 53.1; 48.6; 48.4; 50.3; 45.4 |
| tsbs | native_control | 98.7; 100.4; 100.8; 97.4; 103.0; 99.0 | 46.4; 46.7; 50.3; 53.7; 49.9; 46.2 |
| tsbs | rowid_native_control | 97.6; 99.9; 105.4; 92.6; 101.6; 99.6 | 53.7; 48.1; 51.6; 48.3; 51.8; 47.2 |
| tsbs | cap16 | 93.1; 95.1; 105.6; 93.2; 97.9; 106.4 | 47.2; 49.6; 47.6; 46.2; 49.8; 45.9 |
| tsbs | rowid_inline32 | 103.5; 94.3; 96.3; 102.7; 95.9; 96.7 | 50.6; 43.9; 48.1; 52.3; 47.0; 45.8 |
| tsbs | rowid_inline64 | 91.2; 94.8; 93.1; 94.7; 95.5; 96.7 | 45.5; 52.1; 45.2; 46.7; 45.8; 45.4 |
| alibaba | exact | 70.3; 66.3; 72.2; 72.3; 69.6; 68.1 | 123.6; 116.1; 116.3; 122.9; 123.7; 110.2 |
| alibaba | rowid | 63.3; 62.3; 60.8; 69.4; 65.9; 63.3 | 105.4; 107.4; 103.1; 124.0; 107.4; 105.8 |
| alibaba | native_control | 169.6; 148.7; 157.5; 170.4; 192.3; 175.1 | 121.1; 113.7; 116.3; 150.3; 124.3; 124.6 |
| alibaba | rowid_native_control | 162.5; 152.8; 151.4; 153.4; 166.6; 166.2 | 112.6; 106.0; 107.1; 138.0; 101.4; 104.5 |
| alibaba | cap16 | 176.2; 203.1; 215.8; 171.4; 171.5; 196.3 | 114.1; 111.0; 109.8; 116.5; 126.6; 122.9 |
| alibaba | rowid_inline32 | 145.1; 151.7; 151.1; 147.9; 163.9; 218.3 | 114.8; 106.4; 111.8; 101.6; 112.7; 112.8 |
| alibaba | rowid_inline64 | 127.5; 126.3; 123.2; 127.0; 115.8; 142.7 | 95.2; 95.8; 106.0; 102.0; 107.6; 102.9 |
| irregular | exact | 33.5; 33.0; 36.1; 33.5; 33.0; 32.7 | 12.8; 12.3; 12.7; 13.4; 12.3; 13.5 |
| irregular | rowid | 31.5; 32.2; 36.2; 32.4; 30.3; 32.4 | 13.3; 13.8; 12.4; 13.0; 12.2; 12.2 |
| irregular | native_control | 37.3; 38.3; 39.1; 35.0; 44.0; 37.2 | 13.2; 12.2; 12.9; 13.7; 13.1; 12.5 |
| irregular | rowid_native_control | 34.0; 37.3; 48.0; 36.6; 39.1; 34.5 | 12.5; 12.2; 12.7; 12.1; 13.4; 13.0 |
| irregular | cap16 | 34.3; 38.1; 40.9; 37.2; 40.6; 36.8 | 12.4; 12.6; 13.8; 13.3; 14.2; 12.7 |
| irregular | rowid_inline32 | 34.2; 37.5; 41.6; 35.0; 37.8; 39.3 | 13.3; 12.3; 14.1; 12.8; 14.0; 12.4 |
| irregular | rowid_inline64 | 33.9; 34.5; 34.1; 34.7; 34.7; 35.7 | 11.4; 11.3; 12.2; 13.3; 11.4; 11.6 |

The cutoff is 60% through the fixture's overall time interval. Only blocks
whose last sample is strictly below it are deleted; straddling blocks retain
their original bodies and summaries. Equal-to-cutoff blocks stay live.
Every retention pass verifies clipped visible samples before/after and rejects
any remaining live expired block. Clock refs and surviving payload IDs remain
owned by their original groups; no payload body is moved during retention.

The initial and retained files below are closed, checkpointed files; WAL peak
is the WAL observed after the retention FULL commit and before the explicit
TRUNCATE checkpoint. All post-checkpoint WAL byte counts are zero. Transient
SHM is not charged as persistent database storage. `VACUUM` is a separate
postmeasurement operation, never part of the retention timing:

| Fixture | Variant | Before file B | After retention B | Freelist B | After VACUUM B | WAL before checkpoint B | Expired samples |
| --- | --- | --- | --- | --- | --- | --- | --- |
| tsbs | exact | 4,534,272 | 4,534,272 | 1,142,784 | 2,568,192 | 2,768,672 | 2,908,800 |
| tsbs | rowid | 4,554,752 | 4,554,752 | 1,142,784 | 2,617,344 | 2,739,832 | 2,908,800 |
| tsbs | native_control | 4,526,080 | 4,526,080 | 1,142,784 | 2,568,192 | 2,760,432 | 2,908,800 |
| tsbs | rowid_native_control | 4,546,560 | 4,546,560 | 1,142,784 | 2,617,344 | 2,731,592 | 2,908,800 |
| tsbs | cap16 | 4,526,080 | 4,526,080 | 1,142,784 | 2,568,192 | 2,760,432 | 2,908,800 |
| tsbs | rowid_inline32 | 4,550,656 | 4,550,656 | 1,142,784 | 2,621,440 | 2,735,712 | 2,908,800 |
| tsbs | rowid_inline64 | 4,542,464 | 4,542,464 | 1,159,168 | 2,621,440 | 2,698,632 | 2,908,800 |
| alibaba | exact | 6,774,784 | 6,774,784 | 1,425,408 | 4,517,888 | 4,861,632 | 7,689,600 |
| alibaba | rowid | 5,324,800 | 5,324,800 | 1,155,072 | 3,284,992 | 3,675,072 | 7,689,600 |
| alibaba | native_control | 6,766,592 | 6,766,592 | 1,433,600 | 4,509,696 | 4,845,152 | 7,689,600 |
| alibaba | rowid_native_control | 5,312,512 | 5,312,512 | 1,150,976 | 3,276,800 | 3,666,832 | 7,689,600 |
| alibaba | cap16 | 5,414,912 | 5,414,912 | 1,236,992 | 2,711,552 | 3,876,952 | 7,689,600 |
| alibaba | rowid_inline32 | 5,230,592 | 5,230,592 | 1,122,304 | 3,280,896 | 3,621,512 | 7,689,600 |
| alibaba | rowid_inline64 | 5,345,280 | 5,345,280 | 806,912 | 3,936,256 | 4,033,512 | 7,689,600 |
| irregular | exact | 520,192 | 524,288 | 155,648 | 274,432 | 313,152 | 291,840 |
| irregular | rowid | 520,192 | 524,288 | 147,456 | 274,432 | 313,152 | 291,840 |
| irregular | native_control | 520,192 | 524,288 | 155,648 | 274,432 | 313,152 | 291,840 |
| irregular | rowid_native_control | 520,192 | 524,288 | 147,456 | 274,432 | 313,152 | 291,840 |
| irregular | cap16 | 532,480 | 536,576 | 147,456 | 290,816 | 362,592 | 291,840 |
| irregular | rowid_inline32 | 520,192 | 524,288 | 147,456 | 274,432 | 313,152 | 291,840 |
| irregular | rowid_inline64 | 430,080 | 430,080 | 176,128 | 237,568 | 193,672 | 291,840 |

Retained-file growth/free space and vacuum-compacted density are separate
states. This round does not simulate indefinite append/expire churn or claim
that deleting rows shrinks a file; a production candidate still needs that
lifecycle gate under the real indexed maintenance scheduler.

## Correctness, memory and limits

The final Linux Rust suite passes 30 tests, including independent production
Go manifest readback, IEEE payloads, -0, all residual/clock/envelope modes,
bounded decompression, truncations and checksum guards, and v5 decoder
identification. Additional public Go cross-reads pass 15 closed v4
files across TSBS, Alibaba and irregular fixtures (exact, rowid, cap16,
page1024 and native metadata control). Experimental inline v5 intentionally
requires its own decoder; it is not silently passed to the production engine.

Peak process RSS (KiB), GNU time, separate 512-operation processes, includes
native SQLite/zstd and allocator memory; these observations are not used for
latency comparisons:

| Fixture | Variant | Point RSS | Range RSS | Whole sum RSS | Cut sum RSS |
| --- | --- | --- | --- | --- | --- |
| tsbs | exact | 12692 | 12788 | 12488 | 12680 |
| tsbs | rowid | 12580 | 12816 | 12532 | 12672 |
| tsbs | cap16 | 12724 | 12688 | 12388 | 12760 |
| tsbs | rowid_inline32 | 12788 | 12800 | 12496 | 12596 |
| tsbs | rowid_inline64 | 12672 | 12580 | 12424 | 12788 |
| alibaba | exact | 8084 | 8136 | 8196 | 8032 |
| alibaba | rowid | 8080 | 8180 | 8148 | 8080 |
| alibaba | cap16 | 8148 | 7972 | 8144 | 8096 |
| alibaba | rowid_inline32 | 7988 | 8076 | 8208 | 8028 |
| alibaba | rowid_inline64 | 8024 | 8152 | 8172 | 8024 |
| irregular | exact | 4872 | 4888 | 4788 | 5000 |
| irregular | rowid | 4800 | 4804 | 4788 | 4968 |
| irregular | cap16 | 4784 | 4748 | 4848 | 4996 |
| irregular | rowid_inline32 | 4792 | 4872 | 4804 | 4928 |
| irregular | rowid_inline64 | 4532 | 4740 | 4784 | 4892 |

The synchronous research harness omits production admission, cancellation,
connection pools, directory locks, background work and maintenance scheduling.
It does not change value codecs, block sample counts, exact summary precision,
retention visibility, or payload address rules. No TinyStore source/format
migration, commit or release was made in this round.

## What follows

Carry the rowid groups alternative to a production-shaped candidate: preserve
the UNIQUE(series_id,start_ts) lookup, byte-exact v4 directories, CRCs and
stable payload IDs, then rerun the real public API query/admission and indexed
retention/merge tests. Keep the TSBS regression visible. Inline32 is a small
additional Alibaba win after rowid and needs an explicit new directory format;
it is not a universal threshold recommendation. Cap16 remains an alternate
tradeoff worth judging against its read/maintenance rows. Keep 4-KiB pages for
the public-corpus control. Do not combine these percentages with the companion
payload study without measuring the combined file and lifecycle.

Full raw data, exact source hashes, six-pass values, per-object page types,
overflow, unused bytes, input manifests and file hashes are in
[data/metrics-layout-2026-10-09](data/metrics-layout-2026-10-09/).
