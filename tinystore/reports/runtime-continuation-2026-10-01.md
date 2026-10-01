# Runtime continuation: saved revisions and the final comparison

Measured on `research/runtime-benchmarks`. TinyStore's `main` took the final
candidate on 2 October 2026; the report is a measurement, not a product
guarantee. The full engine comparison completed on both machines;
SDK and corrected wide-read rounds are collected separately below.

| Question | Answer | What follows |
|---|---|---|
| Is the ncruces port complete enough to measure? | Windows checks and SDKs, Linux race, platform lint, snapshots, virtual tables and SQLite page accounting pass. | Measure a saved candidate, not the former uncommitted worktree. |
| Does an idle-slot fast path improve the port? | No repeatable gain in the four local/cloud context variants. One local shared-context spike did not repeat. | Keep the prototype revision, omit it from the final candidate. |
| Can a point read run on the connection reader? | The prototype fails cancellation while the store's memory is held. | Keep dispatch through workers; the cancellation case becomes a gate. |
| Why did the previous cloud run stall? | Its Go measurement client was waiting for upload credit for over twelve hours. Final stream responses and connection loss did not stop that wait; a shared allowance notified only one sender. | Add explicit upload termination and notify every sender that may now fit; bound each benchmark process and its children. |

## Source states

- Baseline: TinyStore `20c71a347a0ab8f2737d33d4b3caeb68b7a61a4a`, modernc,
  grouped SQL batches, hosted jobs and half-full log-buffer flushing.
- Port: `ccbd2ce`, ncruces, registered FTS5/R*Tree on application database
  connections, driver error/cancellation/parameter checks and closed-file
  page accounting. `15ecc8e` fixes the memory gate's early completion signal.
- Final candidate: `ad4f0477f4ba96cf59b0cecbb3e5134f6e064d8a`, the port plus
  credit and upload termination fixes and the point-read cancellation gate.
- Rejected idle-slot candidate: `eeb0aea673f07de89201a1febb7a9ce28c5c542a`.
- Harness: `264981dab0b51df3d8382bbe4e156f8b9a5d5181`; functional changes
  originate in `3309bc2`, subset selection in `e7113df`.
- Corrected metrics and deeper rounds: harness
  `127c3a03bf3bec4db55ba1847e01c608f557b73e`, with explicit metrics output
  bounds and a card generator that refuses failed runs/stages.

All timed sources are extracted from Git archives. The application and its
server binary use the same source revision. Baseline and candidate use one
harness; each run uses a new directory and process, and order reverses on
the second repeat. The final comparison has three repeats of five-second
stages. Stores live on Docker volumes, not host bind mounts.

## Environment and reproduction

Local: AMD Ryzen 7 7700, 16 visible CPUs, Windows host, Docker Desktop 29.6.2,
WSL2 Linux 6.18.33.2, Go 1.27.1 linux/amd64. Cloud: the existing 8-vCPU,
16-GiB Intel Ice Lake Yandex machine, Ubuntu 24.04 and network SSD. Exact
image identities and server versions are retained with the round output.

The local records fixture is private production logs; cloud uses the prepared
LogHub corpus. Neither the private records nor identifying container names
are part of the report. Metrics uses the prepared TSBS DevOps JSONL corpus.
The inputs are pinned as follows:

| Input | Count | SHA-256 |
|---|---:|---|
| TSBS DevOps JSONL, both machines | 2,020 series; 5,090,400 samples | `e4f502af7b0b2ff2c4dba92057a8f2b95e636882f3cb9900986e013d189572cf` |
| Private production logs, local | 67 files; 445,136 records | `d562d80f9643f68b9154b0c2f95188a20c10ca04ca57d79018d0548a9ab993bd` |
| Prepared LogHub, cloud | 10 files; 20,000 records | `319e182f599c7ad3204261d52d38db97dcef64b9634098b7683bf71075266526` |

The log hashes are SHA-256 of the newline-terminated lowercase SHA-256
digests of `.log` files sorted by digest. Private names and
contents are withheld; its input totals 124,158,832 bytes. LogHub's input
totals 2,683,313 bytes. Records are shifted in time by the same loader for
every contender. Different log fixtures mean local/cloud records rates
must not be treated as a hardware-only comparison.
The TSBS hash and public log aggregate were checked on the cloud machine
as well as locally; no cloud corpus identity is inferred from its count.

The local comparison image is
`sha256:7e0a1c36f59bc0a044c55075e2ab9a70488c96fb99a958e0b81a56730ac0a5ee`;
the cloud image is
`sha256:f1b0a026be5d8e2eb2b3885c3b9378014b835dbf25c9f087b68bf35778500a08`.
Both print Redis 8.0.2, PostgreSQL 17.11 and VictoriaMetrics 1.153.0.
The cloud kernel is Linux 6.8.0-142-generic. Full runs started at
2026-10-01 00:14:55 UTC locally and 00:17:08 UTC in the cloud.

From a research checkout with its source submodule pinned to the candidate,
extract the baseline into `<baseline>` and run:

```sh
docker run --rm \
  -v <research>:/src -v <baseline>:/baseline \
  -v <corpus>:/src/tinystore/bench/corpus:ro \
  -v tinystore-compare-go:/go -v tinystore-compare-data:/data \
  -e GOWORK=off -e CGO_ENABLED=0 -e GOFLAGS=-buildvcs=false \
  -e OUT=results/final -e REPEATS=3 -e SECONDS_A_STAGE=5 \
  -e COMPARE_BASELINE_COMMIT=20c71a3 -e TINYSTORE_COMMIT=ad4f047 \
  -e HARNESS_COMMIT=264981d tinystore-compare sh resume.sh
```

`resume.sh` builds both harness binaries, both `tinystore` binaries and the
cgo comparison separately. Its defaults cover every engine, the whole
application with one-file and two-file writes, sidecar/TCP modes, served
SQL/jobs/blobs, crash smoke and binary weight. No build, lint or test runs
on either machine during timed stages.

## Idle slots

These runs compare `ccbd2ce` with `eeb0aea`, two three-second passes each.
Rates below are exact-query operations per second at 64 concurrent callers.

| Machine and context | Port passes | Idle-slot passes |
|---|---:|---:|
| Local, one per worker | 381,567; 378,024 | 383,643; 371,049 |
| Local, one shared | 153,451; 164,496 | 305,569; 169,934 |
| Cloud, one per worker | 195,204; 194,125 | 194,221; 195,474 |
| Cloud, one shared | 190,068; 190,790 | 191,439; 190,923 |

The first local shared-context result is an outlier against the repeat and
the cloud control. It is not evidence of a general doubling. Neither normal
worker contexts nor the cloud shared-context case supports retaining the
additional branches. The complete output also includes sets, mixed calls,
CPU, latency and memory; the final code keeps the existing slot mechanism.

## Correctness and verification

The port passed `task check` on Windows, including all three Go modules,
formatting, lint, dependency checks and the linux/amd64 size probe. The size
probe is 13,040 KiB, with an import delta of 11,568 KiB, built without cgo.
Both SDK suites passed: 217 Bun tests and 188 Python tests, type/style checks
included. Schema-row fuzzing ran for twenty seconds and 2.34 million cases.
The final retained branch reran `task check`, all three Linux race suites
and `task sdk`; Windows SDK verification used Bun 1.4.2 and Python 3.14.6,
with Python coverage 85.54%. This is separate from the SDK benchmark's
Python 3.13.5 environment.

Linux containers ran all three Go modules with `-race -count=3 -shuffle=on`.
Platform lint covered Linux and Darwin; the root fallback built for Plan 9.
Darwin was linted, not executed. Closed-file page counts were compared with
modernc's actual `dbstat` on eighteen combinations of page size, text
encoding and auto-vacuum, including overflow, freelist, WITHOUT ROWID,
FTS5 and R*Tree tables.

Credit/upload gates were shown to fail before the fix, and rerun with race
thirty times. The point-read cancellation gate fails the old inline-reader
prototype and passes through workers. Served 1-MiB blobs completed over
both local sidecar and TCP in a short smoke run before the full round.
The original event that first stopped the old cloud upload was not proven;
the observed unbounded waits and the broken wake-up behavior were reproduced
directly. The final comparison is not used to invent that missing cause.

The final public-corpus integration gate also reopened all 5,090,400 TSBS
samples, checked every value bitwise and checked 64 exact aggregates. Its
[complete output](data/runtime-2026-10-01/wsl2/physical/public-corpus-gate.txt)
is retained. That test ingests/maintains by whole series with its own test
options, unlike the comparison's time windows; its 0.894773-byte/sample file
is not substituted for the comparison's 1.327676-byte/sample state.

## Completed comparison

### Application stack

A request checks a session, reads a note, emits a log record and records a
metric. Ten percent also update a note and enqueue an indexing job; two
percent upload a 16-KiB attachment. The services use Redis, PostgreSQL,
VictoriaMetrics and files. PostgreSQL commits the note and job together.
`tinystore-batch` keeps jobs in the application database and commits the note
and job through one grouped batch; `tinystore` keeps the default two files.

Requests per second at 64 concurrent clients, all three passes:

| Machine | Candidate, one file | modernc, one file | Candidate, two files | Services |
|---|---:|---:|---:|---:|
| Local | 81,531; 81,791; 81,325 | 47,948; 47,509; 47,568 | 51,344; 51,976; 52,027 | 52,247; 55,625; 54,904 |
| Cloud | 44,295; 42,916; 43,178 | 34,161; 32,580; 35,455 | 41,953; 39,918; 37,669 | 28,372; 26,859; 28,567 |

The one-file candidate is 1.71 times the saved modernc version locally and
1.26 times in the cloud, by the median of the passes. Against the services it
is 1.49 and 1.52 times respectively. This is the named application workload,
not a claim to beat every specialized engine. The default two-file design is
near the services locally and faster on this cloud disk; joining the queue
to the application's file is an explicit application choice.

The one-file candidate and services have zero operation errors, dropped log
records and jobs left waiting in all passes. Other served/default modes
occasionally leave a small queue at 64 clients (up to 44 jobs). At 256 clients,
the one-file candidate consistently leaves a material queue: 4,963; 3,664;
5,937 jobs locally and 3,037; 2,235; 2,987 in the cloud. Its 256-client request
rate is therefore not a demonstrated sustainable whole-application rate.
There are no dropped logs at any measured depth. The README uses the
64-client configuration and explicitly identifies its joined queue.

Reported memory in MiB, each pass, includes the client's high-water RSS plus
the greater of service-parent high-water RSS and service-tree PSS measured
at the end:

| Machine | Candidate, one file | modernc, one file | Services |
|---|---:|---:|---:|
| Local | 113.3; 113.7; 113.9 | 145.4; 145.8; 144.7 | 162.0; 171.5; 206.7 |
| Cloud | 103.9; 105.3; 104.4 | 134.4; 132.6; 134.9 | 134.6; 128.9; 127.6 |

PSS avoids counting PostgreSQL's shared pages once per connection. This
composite is not a sampled peak of the simultaneous whole process tree;
the report retains the separate RSS and PSS fields so its definition is
checkable. CPU per one-file request is about 51 microseconds locally and
94 in the cloud, versus 161 and 178 for services.

### Saved-revision engine changes

Operations per second. Every cell is the three passes in repeat order;
ratios use medians. KV/SQL and 4-KiB blobs use 64 concurrent callers;
job drain and 1-MiB blob reads use eight. KV holds 100,000 keys with
128-byte values; mixed calls are 90% gets and 10% sets. These contexts are
per worker, not one shared cancellation lock.

| Machine | Operation | modernc baseline | Candidate |
|---|---|---:|---:|
| Local | KV get | 311,996; 315,502; 313,055 | 374,988; 388,085; 384,797 |
| Local | KV set | 11,169; 11,193; 11,245 | 16,775; 19,304; 19,282 |
| Local | KV mixed | 69,911; 70,728; 70,818 | 117,323; 120,328; 120,923 |
| Cloud | KV get | 139,049; 138,985; 137,169 | 192,449; 192,122; 192,462 |
| Cloud | KV set | 5,641; 5,554; 5,677 | 5,403; 5,420; 5,500 |
| Cloud | KV mixed | 48,655; 45,586; 46,124 | 58,100; 59,053; 59,004 |
| Local | SQL get | 335,370; 338,555; 342,857 | 371,622; 375,647; 364,419 |
| Local | SQL insert | 18,412; 17,993; 17,968 | 32,021; 32,140; 31,985 |
| Local | SQL mixed | 79,795; 79,806; 80,652 | 137,555; 138,909; 138,655 |
| Cloud | SQL get | 160,959; 158,336; 160,304 | 206,480; 205,079; 207,454 |
| Cloud | SQL insert | 19,953; 20,448; 20,647 | 41,463; 39,735; 39,514 |
| Cloud | SQL mixed | 50,706; 51,248; 50,869 | 49,701; 49,422; 49,335 |
| Local | Jobs enqueue | 15,382; 15,436; 15,019 | 25,956; 25,992; 25,549 |
| Local | Jobs drain | 2,761; 2,709; 2,718 | 4,622; 4,589; 4,648 |
| Cloud | Jobs enqueue | 18,205; 11,262; 15,863 | 29,172; 18,765; 26,393 |
| Cloud | Jobs drain | 3,448; 1,889; 3,030 | 7,866; 4,736; 6,556 |
| Local | Blobs put 4 KiB | 8,451; 8,559; 8,523 | 11,638; 11,759; 11,838 |
| Local | Blobs get 4 KiB | 121,862; 119,249; 117,302 | 184,258; 183,412; 188,818 |
| Cloud | Blobs put 4 KiB | 4,469; 4,353; 4,221 | 4,391; 4,188; 4,187 |
| Cloud | Blobs get 4 KiB | 61,352; 51,471; 57,075 | 100,932; 101,187; 101,638 |
| Local | Blobs get 1 MiB | 13,068; 13,211; 13,249 | 12,454; 13,120; 13,141 |
| Cloud | Blobs get 1 MiB | 4,172; 4,134; 4,167 | 4,198; 4,169; 4,147 |

The port improves local KV sets 1.72 times and SQL inserts 1.78 times.
Cloud SQL inserts improve 1.94 times, but KV sets are 4% slower and SQL mixed
calls 3% slower. Large blobs still use files and do not show a material
driver-related read gain. The cloud jobs passes vary substantially, so
their range matters as well as their median. The candidate is retained for
its combined application, read, insert and enqueue results, not a promise
that changing a driver makes every storage operation faster.

### Other engines

Median operations/s at 64 callers, with all underlying passes in the raw
JSON and the generated summaries:

| Machine | KV contender | Gets | Durable sets | Mixed |
|---|---|---:|---:|---:|
| Local | TinyStore | 384,797 | 19,282 | 120,328 |
| Local | Pebble | 380,904 | 20,416 | 178,393 |
| Local | bbolt, copied values | 1,067,066 | 4,407 | 43,535 |
| Local | Badger | 932,969 | 640 | 6,007 |
| Local | Redis, Unix socket | 182,954 | 9,431 | 10,368 |
| Local | SQLite, modernc by hand | 144,386 | 365 | 3,335 |
| Cloud | TinyStore | 192,449 | 5,420 | 59,004 |
| Cloud | Pebble | 228,697 | 40,811 | 133,092 |
| Cloud | bbolt, copied values | 504,666 | 3,269 | 32,521 |
| Cloud | Badger | 503,924 | 866 | 6,974 |
| Cloud | Redis, Unix socket | 140,241 | 11,872 | 11,568 |
| Cloud | SQLite, modernc by hand | 69,840 | 485 | 4,414 |

Pebble's grouped sync writes are stronger on this cloud disk, while bbolt
and Badger read an in-process key much faster. A Redis call crosses a
process boundary; the served TinyStore and language-SDK rounds below are
the corresponding comparisons. The borrowed bbolt variant remains in the
raw data, separate from the copied-value API.

SQL insert medians are 32,021 locally and 39,735 in the cloud for TinyStore,
against PostgreSQL's 20,523 and 37,714. PostgreSQL's cloud mixed workload is
93,998/s against TinyStore's 49,422/s; it is not dominated. Hand-written
modernc, ncruces and mattn drivers are separate contenders; none groups an
application's separate inserts the way TinyStore's API does. mattn uses cgo
and is only a comparison dependency, never a product dependency.

Jobs' enqueue/drain medians are 25,956/4,622 locally and 26,393/6,556 in the
cloud, against goqite's 363/184 and 489/233. Queue protocols and batching
are named in the harness; these are implementations of a comparable task,
not identical APIs. Whole-file blob reads retain integrity checking in
TinyStore, while the files contender reads plain files without that check.

Final file sizes after the timed write stages are not equal-input storage
efficiency measurements: faster writers inserted more rows. The fixed
records and metrics corpora below are the footprint comparisons.

### Records

The local fixture has 445,136 records; cloud has 20,000. Append rates exclude
the separately timed maintenance/settle stage. The final directory includes
the closed files. Median MiB and window reads/s at eight callers:

| Machine | Contender | Disk MiB | Window reads/s |
|---|---|---:|---:|
| Cloud | TinyStore | 0.57 | 1,028 |
| Cloud | modernc baseline | 0.58 | 1,009 |
| Cloud | SQLite | 3.73 | 28,252 |
| Cloud | JSONL | 3.19 | 6,101 |
| Cloud | JSONL + zstd | 0.32 | 2,015 |

Local TinyStore window reads are 5,795; 5,819; 5,818/s versus the baseline's
5,129; 5,158; 5,237. SQLite's COUNT is much faster at 166,043; 168,928;
168,016/s; TinyStore's records query decodes and checks records. Cloud's
small fixture shows almost no read gain. JSONL+zstd is smaller than TinyStore
on that fixture. The trade-off is not erased by selecting only favorable
storage or append figures.

### Metrics and the rejected wide-read output

The original harness asked for 763,560 output samples with TinyStore's
default 100,000-sample query budget. Every TinyStore wide-read operation
failed: 3,805 errors locally and 1,586 in the cloud across both revisions.
Those attempted-operation rates are not successful query rates. Their JSON
is retained as rejected output; the corrected harness explicitly sets a
1,048,576-sample output budget, leaving all other engine bounds in place.
Both saved revisions and both specialized contenders are rerun together.

Corrected ingest operations/s and one-series reads/s at eight callers,
three passes per cell:

| Machine | Contender | Ingest, samples/s | Series reads/s |
|---|---|---:|---:|
| Local | TinyStore | 983,384; 981,931; 974,939 | 17,471; 17,451; 17,330 |
| Local | modernc baseline | 839,702; 845,617; 834,670 | 12,203; 12,234; 12,157 |
| Local | Prometheus | 10,496,620; 10,767,047; 10,283,402 | 455,108; 446,446; 425,502 |
| Local | VictoriaMetrics | 7,134,964; 7,123,819; 6,968,012 | 12,482; 12,710; 12,875 |
| Cloud | TinyStore | 407,259; 423,826; 401,755 | 6,061; 6,012; 5,975 |
| Cloud | modernc baseline | 351,903; 357,158; 362,983 | 4,739; 4,798; 4,778 |
| Cloud | Prometheus | 4,165,253; 4,102,067; 4,064,088 | 137,388; 140,286; 143,370 |
| Cloud | VictoriaMetrics | 2,954,890; 2,965,550; 2,937,709 | 5,454; 5,551; 5,465 |

The successful wide read matches 303 series over the whole corpus and
checks that all 763,560 samples were returned. Cloud medians are 21/s for
TinyStore, 20/s for its baseline, 36/s for Prometheus and 5/s for Victoria.
There are no errors in any corrected metrics stage. WSL2 wide-read figures
remain raw throughput data, not bare-Linux one-in-flight latency claims.

Ingest plus the separately timed settle stage, final directory footprint,
and composite process memory, medians:

| Machine | Contender | Samples/s, ingest + settle | Disk MiB | RAM MiB |
|---|---|---:|---:|---:|
| Local | TinyStore | 586,718 | 6.45 | 240.6 |
| Local | modernc baseline | 512,489 | 6.45 | 237.3 |
| Local | Prometheus | 9,622,107 | 71.61 | 284.6 |
| Local | VictoriaMetrics | 5,775,397 | 9.03 | 687.8 |
| Cloud | TinyStore | 256,701 | 6.45 | 226.8 |
| Cloud | modernc baseline | 234,231 | 6.45 | 226.6 |
| Cloud | Prometheus | 4,070,223 | 71.60 | 278.0 |
| Cloud | VictoriaMetrics | 2,528,908 | 9.03 | 593.4 |

TinyStore's format did not change in the driver port: both revisions keep
the same footprint. Its input is committed durably at each ingest batch;
Prometheus syncs its WAL on segment boundaries, and Victoria's HTTP import
acknowledgement is not an equivalent per-batch disk barrier. Settle brings
each contender to the state its harness specifies, not an identical
compaction policy. Prometheus has both far higher ingest and query rates
on this fixture. Smaller files and process memory do not imply faster
ingest or queries. Corpus allocations are included in each client's RSS.

To repeat only the corrected round, use the full-round Docker command with
the harness pinned to `127c3a0`, `ENGINES=metrics`, `DEEP_ROUNDS=0` and a
new `OUT`; keep the same saved source pair, image and prepared corpus.

### Where the file bytes live

The saved `127c3a0` harness binaries were also run through `child`, keeping
their closed directories instead of the parent's automatic cleanup. This
is a footprint check, not another throughput comparison: reads are shortened
to one second. The `b4d12ec` helper reads the closed pages with the checked
production `internal/dbstat` parser. Its [raw page ownership](data/runtime-2026-10-01/wsl2/physical/pages.jsonl)
lists every b-tree and the remaining pages, in candidate-metrics,
baseline-metrics, candidate-records order.

The metrics file is **6,758,400 bytes**, or **1.327676 bytes per input sample**.
Every object's byte count is identical in the candidate and baseline:

| Object | Bytes, each revision |
|---|---:|
| `sqlite_schema` | 4,096 |
| `_tinystore_migrations` | 4,096 |
| `store_state` | 4,096 |
| `series` | 155,648 |
| `sqlite_autoindex_series_1` | 118,784 |
| `label_values` | 16,384 |
| `sqlite_autoindex_label_values_1` | 12,288 |
| `postings` | 253,952 |
| `series_state` | 2,637,824 |
| `series_ready` | 4,096 |
| `series_due` | 40,960 |
| `series_failed` | 4,096 |
| `series_failed_id` | 4,096 |
| `clocks` | 4,096 |
| `sqlite_autoindex_clocks_1` | 4,096 |
| `groups` | 413,696 |
| `payloads` | 3,076,096 |
| Freelist, pointer maps and lock-byte pages | 0 |

`payloads` here means that table's whole pages, not the codec body length.
The remaining head in `series_state` and directory pages still belong to
the measured file. No codec representation changed, and no payload-only
size is substituted for the file size.

The private-records check closes at 12,949,504 bytes. `blocks` owns 6,320,128;
the other b-trees own 108,544; **6,520,832 bytes remain outside live b-trees**
(freelist/other SQLite bookkeeping). The whole file, not just its live
blocks, is the README's footprint. Small page-count differences between
this copy and the three full passes are retained, not vacuumed away for
the chart. The record bodies and their private stream names are not copied
into the report data.

From the saved comparison image with the prepared corpus and `/data` volume,
build the saved binaries as above, then:

```sh
mkdir -p /data/physical/candidate /data/physical/baseline /data/physical/records
/tmp/compare-candidate child -engine metrics -contender tinystore \
  -dir /data/physical/candidate -seconds 1 > physical-candidate.json
/tmp/compare-baseline child -engine metrics -contender tinystore \
  -dir /data/physical/baseline -seconds 1 > physical-baseline.json
/tmp/compare-candidate child -engine records -contender tinystore \
  -dir /data/physical/records -seconds 1 > physical-records.json
go run dbstat_report.go /data/physical/candidate/metrics.db \
  /data/physical/baseline/metrics.db /data/physical/records/records.db > pages.jsonl
```

The directories must be new. The helper comes from `b4d12ec`, whose source
submodule remains the candidate. It is not linked into the timed harness.

### Served engines and point-read latency

Cloud Go-client rates at 64 calls in flight, medians:

| Operation | Embedded | Local sidecar | Loopback TCP/token |
|---|---:|---:|---:|
| KV get | 192,449 | 114,983 | 92,741 |
| KV set | 5,420 | 5,393 | 5,422 |
| SQL get | 207,289 | 102,890 | 31,896 |
| SQL insert | 43,158 | 31,675 | 17,579 |
| Jobs enqueue | 29,973 | 27,005 | 26,183 |
| Blobs get, 4 KiB | 102,542 | 71,419 | 62,377 |

Served 1-MiB uploads and downloads complete in every pass, with zero stage
errors. At eight callers, cloud sidecar/TCP downloads are 855/1,415 per
second versus 4,191 embedded; copying and transport remain significant.
The no-error result is the upload regression gate, not a claim of zero
transport overhead.

The cloud sidecar's one-in-flight KV get is 11,346; 11,191; 11,225/s, about
89 microseconds per round trip. This is not a cold-process startup number.
The original 180-microsecond WSL2 measurement is not bare-Linux latency.
Worker dispatch remains: bypassing it blocks CANCEL behind a point read
waiting for memory. No latency speedup is claimed for the rejected inline
prototype. Its remaining optimization requires a cancellation-safe design.

### Crash smoke and binary weight

Each of seven contenders was killed and reopened thirty times on each
machine: embedded TinyStore, its sidecar, modernc SQLite, bbolt, Badger,
Pebble and Redis. Every acknowledged value was recovered, with zero lost
or wrong values in all 420 cycles. SIGKILL tests process failure, not power
loss or dishonest storage hardware.

The Linux/amd64 stripped KV probe is 8.57 MiB for TinyStore, adding 7.33 MiB
over the empty program. Hand-written modernc SQLite is 6.35 MiB, adding
5.10 MiB; mattn is 3.61 MiB and uses cgo. The earlier `task size` figure is
a broader public-API probe, not this KV-only executable. ncruces v0.35.6
uses wasm2go-generated Go for SQLite and its own Go VFS, not wazero.
The port remains cgo-free; a smaller executable is not claimed.

### Language SDKs

Two three-second passes per language and contender, at 64 calls in flight.
Both SDKs use the candidate's real `tinystore serve`, and Redis uses each
language's own client with `appendfsync always`. Values and the 90%-get
mixed distribution are the Go KV workload. Median calls/s:

| Machine | Client and transport | Gets | Sets | Mixed |
|---|---|---:|---:|---:|
| Local | Bun, TinyStore sidecar | 138,429 | 16,416 | 92,363 |
| Local | Bun, TinyStore TCP | 115,386 | 16,476 | 82,420 |
| Local | Bun, Redis Unix | 348,593 | 10,031 | 10,917 |
| Local | Bun, Redis TCP | 162,955 | 9,849 | 11,187 |
| Local | Python, TinyStore sidecar | 57,250 | 14,235 | 51,370 |
| Local | Python, TinyStore TCP | 55,962 | 13,577 | 50,944 |
| Local | Python, Redis Unix | 18,473 | 9,043 | 10,079 |
| Local | Python, Redis TCP | 16,097 | 9,365 | 9,882 |
| Cloud | Bun, TinyStore sidecar | 68,149 | 4,673 | 51,171 |
| Cloud | Bun, TinyStore TCP | 66,366 | 5,777 | 47,679 |
| Cloud | Bun, Redis Unix | 262,014 | 15,019 | 18,285 |
| Cloud | Bun, Redis TCP | 144,122 | 13,326 | 15,821 |
| Cloud | Python, TinyStore sidecar | 22,072 | 6,161 | 20,687 |
| Cloud | Python, TinyStore TCP | 21,889 | 5,540 | 20,477 |
| Cloud | Python, Redis Unix | 18,412 | 12,951 | 14,436 |
| Cloud | Python, Redis TCP | 16,286 | 12,430 | 14,408 |

All 32 runs completed with zero stage errors. These are not interchangeable
with Go-client rates: the SDKs' scheduling, encoding and language clients
are included. Bun + Redis reads faster on both machines. Local TinyStore
sets are faster, cloud Redis sets are faster; mixed calls favor TinyStore
in both languages on both machines. SDK baseline A/B is not part of this
round, so these rows do not isolate a language-SDK driver-port speedup.

Both comparison images use Bun 1.4.2, Python 3.13.5 and redis-py 6.4.0.
The SDK image identities are
`sha256:b0a6b571487fd45ce42e17b047fd3dfc8f8a322cb7f7b4fc951459eefdac8ac2`
locally and
`sha256:ab71a7e116c80db177752efcf617e16224dcd25f1cf5e968fc26ff40071fe216`
in the cloud. The SDK timed source is the full comparison's harness and
candidate, not the later metrics-budget patch. Local SDK started 05:01:54
UTC and completed 05:14:13 UTC; its progress file is retained with the JSON.

```sh
docker run --rm -v <research>:/src -v tinystore-compare-data:/data \
  -e GOWORK=off -e CGO_ENABLED=0 -e GOFLAGS=-buildvcs=false \
  -e OUT=results/final -e REPEATS=2 -e SECONDS_A_STAGE=3 \
  tinystore-compare-sdk sh sdk/sdk.sh
```

### Longer-run checks

The bare-cloud fixed-load round first measures each contender's own
64-client maximum, then offers 25%, 50%, 75% and 90% of that maximum for ten
seconds. Two passes, reversed order. Arrival-to-completion latency includes
time queued before a worker takes the call; it does not hide the queue by
waiting for one request before scheduling the next. The offered rates
differ between contenders, so these are operating points, not equal-load
latency A/B results.

| Contender | Share of own max | Completed requests/s, passes | p99 ms, passes |
|---|---:|---:|---:|
| Candidate, one file | 50% | 20,196; 19,995 | 5.77; 6.82 |
| Candidate, one file | 75% | 30,262; 29,864 | 14.68; 10.49 |
| Candidate, one file | 90% | 36,315; 35,892 | 75.50; 67.11 |
| modernc, one file | 50% | 17,488; 15,931 | 9.44; 12.58 |
| modernc, one file | 75% | 26,228; 23,894 | 15.73; 16.78 |
| modernc, one file | 90% | 31,452; 28,658 | 20.97; 18.87 |
| Services | 50% | 14,659; 15,233 | 7.34; 33.55 |
| Services | 75% | 21,693; 22,850 | 150.99; 11.53 |
| Services | 90% | 26,397; 27,425 | 23.07; 33.55 |

No operation errors or dropped logs occurred. Services missed 3,027 offered
arrivals in the first 75% pass; the second pass missed none. Other operating
points missed none. Candidate jobs were fully handled at every operating
point in both passes. Services left 12 and 37 jobs at two counted boundaries,
and the baseline left 32 during one maximum-rate stage. Tail latency is not
a universal win: the candidate's 90%-load p99 is substantially worse than
the baseline's, at a higher offered rate.

Two bounded three-minute application passes per candidate, baseline and
service stack completed, with one-minute windows. Requests/s:

| Contender | Whole-pass rates | Minute rates, first pass | Minute rates, second pass | RAM MiB, passes |
|---|---:|---:|---:|---:|
| Candidate, one file | 35,151; 35,169 | 36,907; 34,253; 34,289 | 36,846; 34,194; 34,467 | 111.2; 110.8 |
| modernc, one file | 31,493; 31,311 | 32,426; 31,018; 31,036 | 32,394; 30,819; 30,720 | 142.5; 140.3 |
| Services | 24,418; 27,603 | 20,962; 25,171; 27,148 | 28,240; 27,254; 27,312 | 138.0; 168.8 |

The candidate is 1.12 times the baseline and 1.35 times services by median
whole-pass rates. It handles all 632,053 and 632,484 enqueued jobs, with
zero operation errors, zero lost logs and no queue remaining. Services also
drain both passes; the baseline leaves 32 jobs in its second pass. The
candidate settles below its five-second burst rate: the final minute is
about 34.3/34.5k, not 43.2k requests/s. Its minute p99 is 46-50 ms versus
25-29 ms for the baseline and 11-25 ms for services.

These passes test more than a short burst, but do not establish half-hour
steady state. Aggregate timeline stages do
not calculate a combined p99; use the per-minute histogram fields instead
of treating their default zero as measured zero latency.

After the same image and saved binaries are built by `resume.sh`, reproduce
the cloud follow-up commands with those binaries and an empty data volume:

```sh
export COMPARE_BASELINE_BIN=/tmp/compare-baseline
export COMPARE_BASELINE_SERVER=/tmp/tinystore-baseline
export TINYSTORE_BIN=/tmp/tinystore-candidate
export COMPARE_BASELINE_COMMIT=20c71a3 TINYSTORE_COMMIT=ad4f047
/tmp/compare-candidate run -engine stack-latency \
  -contenders tinystore-batch,tinystore-batch-baseline,services \
  -repeats 2 -seconds 10 -dir /data -out results/final/stack-latency.json
/tmp/compare-candidate run -engine stack-steady \
  -contenders tinystore-batch,tinystore-batch-baseline,services \
  -repeats 2 -seconds 180 -timeout 10m -dir /data \
  -out results/final/stack-steady.json
```

## What follows

Retain the checked ncruces port and credit/upload fixes. Keep idle-slot and
inline-reader prototypes out of the final branch. Review and merge the
saved source before publishing this draft or the refreshed README on main.
The source branch, report branch, raw JSON and generated cards stay local;
no remote branch or machine is deleted, and no push is performed.

The port, runtime gates, saved-revision comparisons and README refresh are
complete. This is not completion of every idea in the former agent's
scratchpad: FIFO/AfterFunc slot prototypes, the two-CPU syscall anomaly,
matching a trigger outbox, and grouping declarative server batches remain
separate experiments. Real read-decide-write transactions still take the
writer exclusively by contract. No unmeasured version of those ideas is
merged, and a lower first-get latency is not claimed.
