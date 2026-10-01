# Small Consumers, Idle Memory and Raw Metrics

Draft on `research/runtime-benchmarks`, with TinyStore's measured source still
off main. Source is `ad4f0477f4ba96cf59b0cecbb3e5134f6e064d8a`; harness is
`f353a11`. No production code is changed in this round.

| Question | Answer | What follows |
|---|---|---|
| Does a small consumer need the comparison's 30 MiB at startup? | No. A single engine opens at 8.5-9.3 MiB on the VM; all six at 14.9. | Do not present the large comparison binary's open RSS as a minimal consumer's cost. |
| Does explicit GC make idle RSS smaller? | Not here: it lowers some live allocations but starts GC work and touches more resident pages. | Keep natural-open, GC and forced OS release separate. |
| Can one memory card show both idle and load peak? | Yes, if both include the same client's and services' processes in one run. | Measure service idle, which the former harness omitted. |
| What would the TSBS samples cost without compression? | 82,039,788 bytes including a series index; every timestamp and value rereads unchanged. | Add the actual raw representation, not just a theoretical 16-byte/sample calculation. |

## Environment and Inputs

Local Linux container: Ryzen 7 7700, 16 visible CPUs, WSL2 kernel 6.18.33.2.
Cloud: existing 8-vCPU Ice Lake Yandex VM, Ubuntu 24.04 kernel 6.8.0-142.
Both build with Go 1.27.1, `CGO_ENABLED=0`, `-trimpath -ldflags='-s -w'`.
The [raw data](data/consumer-memory-2026-10-01/) retains each host's printed
environment, five passes and phase samples. No measurement and build/test
work overlap on the same host.

Each consumer links only the root and the engine it opens; `all` links all
six, and `empty` only the shared standard-library probe. A dependency test
checks the boundary. SQL creates one empty `note(id)` table; KV, jobs and
blobs create one empty bucket or queue. Default engine options and background
work remain enabled. No corpus or application rows are loaded.

## Natural Startup

RSS in MiB, median of five passes. `Linked` means package initialization
without `Open`; `Open` is sampled 250 ms after opening. Case order reverses
on every second pass. The empty program includes the same sampling protocol;
it is not an absolute minimum Go executable.

| Case | Local linked | Local open | VM linked | VM open |
|---|---:|---:|---:|---:|
| Empty probe | 3.39 | 3.46 | 3.31 | 3.37 |
| Root only | 3.70 | 3.77 | 3.55 | 3.65 |
| KV | 5.59 | 8.78 | 5.46 | 8.61 |
| SQL | 5.75 | 9.33 | 5.63 | 9.25 |
| Jobs | 5.33 | 8.61 | 5.23 | 8.49 |
| Blobs | 5.52 | 8.70 | 5.36 | 8.62 |
| Records | 5.70 | 9.30 | 5.55 | 9.29 |
| Metrics | 5.46 | 9.17 | 5.36 | 8.96 |
| All six | 6.86 | 14.99 | 6.67 | 14.93 |

This directly separates linking/initialization from opening within the same
binary. It does not assign every resident page to a particular dependency.
Go's heap counters are only part of RSS: code, mapped memory and stacks also
matter. The comparison's files-only contender opened at 27.7 MiB, while its
TinyStore KV contender opened at 31.3; neither was a minimal executable.

## GC and OS Release

The same processes then run `runtime.GC`, followed by `debug.FreeOSMemory`.
The second is artificial scavenging, not normal idle. VM median RSS:

| Case | Natural open | After GC | After forced release | Live Go heap after GC |
|---|---:|---:|---:|---:|
| Empty | 3.37 | 3.78 | 3.69 | 0.18 |
| Root | 3.65 | 4.03 | 3.97 | 0.18 |
| KV | 8.61 | 9.19 | 8.91 | 0.30 |
| SQL | 9.25 | 9.88 | 9.59 | 0.29 |
| Jobs | 8.49 | 9.13 | 8.95 | 0.28 |
| Blobs | 8.62 | 9.24 | 8.98 | 0.29 |
| Records | 9.29 | 9.78 | 9.64 | 0.42 |
| Metrics | 8.96 | 9.55 | 9.43 | 0.28 |
| All six | 14.93 | 15.67 | 15.33 | 0.78 |

The low live heap is not a substitute for process RSS. More reader connections
are opened under concurrent load and retained by design; that warmed idle
state is not measured by this empty-start experiment. The startup high-water
counter is retained, but is not labeled as peak under an application workload.

## Paired Application Memory

The full comparison binary separately ran three three-second application
passes at 8, 64 and 256 callers. Every startup sample now includes service
parent high-water RSS and service-tree PSS as well as the client RSS, with
the same max(HWM, PSS) rule as the load measurement. Medians and passes:

| Configuration | Idle MiB | Load composite MiB |
|---|---:|---:|
| SQL + jobs, joined file | 41.2; 41.0; 40.4 | 112.3; 110.8; 111.1 |
| Jobs separate | 42.9; 43.2; 41.8 | 110.6; 115.7; 112.6 |
| Services | 75.3; 75.7; 75.3 | 142.6; 141.8; 159.0 |

The faded lower bar is the load composite, not a simultaneously sampled
whole-tree peak. The solid upper bar is idle from that same run. The card
does not splice the small consumer's idle into the larger comparison's peak.
All operations and logs succeed, but the joined queue remains behind at
256 callers (2,533; 1,150; 2,101 jobs). That is not a sustainable-rate claim.
Rates from this shorter follow-up do not replace the three five-second
throughput passes in the previous report.

## Actual Raw Metrics

TSBS DevOps: 2,020 series and 5,090,400 samples, source JSONL SHA-256
`e4f502af7b0b2ff2c4dba92057a8f2b95e636882f3cb9900986e013d189572cf`.
`cmd/raw` writes one binary file of little-endian int64 millisecond times and
float64 bits, and one JSON series index with labels, byte offsets and counts.
It rereads the entire binary against the input and refuses missing, changed
or extra samples. Its test includes negative time and the bits of negative
zero, and verifies the series index.

| Object | Bytes |
|---|---:|
| `samples.bin` | 81,446,400 |
| `series.json` | 593,388 |
| Total | 82,039,788 (78.24 MiB) |

This is a real uncompressed representation, not a SQLite table or complete
engine. It provides neither TinyStore's query API, retention nor integrity
envelope. The chart includes both files, not just the body bytes. TinyStore's
unchanged comparison state is 6,758,400 bytes (6.45 MiB); the prior report
divides its whole file among every table and index. No hypothetical row-based
SQL file size is inferred from this 16-byte/sample representation.

## Reproduction

Pin the research harness and source above. Build and run the SDK comparison
image as the preceding round specifies. From the `bench/idle` module:

```sh
OUT=results/<machine> TINYSTORE_COMMIT=ad4f047 HARNESS_COMMIT=f353a11 sh run.sh
go build -o /tmp/raw ./cmd/raw
/tmp/raw <corpus>/tsbs-packed/series.jsonl /data/<new-raw-directory> > raw.json
```

For the memory card, run `bench/compare/resume.sh` with
`ENGINES=stack`, `CONTENDERS=tinystore-batch,tinystore,services`,
`DEEP_ROUNDS=0`, `REPEATS=3`, `SECONDS_A_STAGE=3`, the same source pair and
data-volume mounts as the preceding round. The saved `stack-memory.json`
contains the new service-idle fields. Generate cards from the preceding
accepted rounds plus this file and `metrics-raw.json` using `cards.py`.

## Interleaved Client Modes

A follow-up uses harness `5f672d5`, the same saved TinyStore source, local
Ryzen/WSL2 comparison image and data volume. Two three-second passes per mode,
one session: Go modes run before the first SDK pass and after the reversed
second pass. Both language and transport order reverse on pass two. Builds
finish before any calls are measured. Bun is 1.4.2, Python is 3.13.5.

Every mode holds 100,000 keys with 128-byte values. Rates at 64 in flight,
both passes, calls/s:

| Mode | Gets | Sets | Mixed, 90% gets |
|---|---:|---:|---:|
| Go embedded | 373,418; 361,046 | 18,889; 19,157 | 117,529; 119,679 |
| Go sidecar | 267,591; 275,542 | 17,937; 18,088 | 108,423; 108,095 |
| Go TCP server | 137,081; 143,358 | 17,965; 18,346 | 86,169; 88,217 |
| Bun sidecar | 130,465; 134,364 | 16,359; 16,719 | 89,719; 90,100 |
| Bun TCP server | 115,201; 112,626 | 15,985; 15,954 | 78,891; 80,084 |
| Python sidecar | 54,794; 56,383 | 13,985; 13,535 | 50,351; 50,882 |
| Python TCP server | 55,990; 56,032 | 13,306; 13,336 | 50,353; 51,439 |

All fourteen runs have zero stage errors and no failed run. By medians,
Go sidecar gets retain 74% of Go embedded's rate, Bun sidecar 36%, Python
sidecar 15%; writes retain 95%, 87% and 72%. Those language ratios include
the client and its encoding/scheduling, not just transport. Bun and Python
have no embedded mode. Loopback TCP uses a token but not TLS. WSL2's
single-in-flight latency is not quoted as bare-Linux latency.

The single client-mode chart shows reads/writes in separate panels, retaining
all Go, Bun and Python modes so its title describes the entire comparison.
The mobile variant stacks those panels instead of shrinking their labels.
The matched round, not the older separately timed SDK results, supplies
all their numbers. The raw file is `wsl2/sdk-modes.json`; its `started`
field is aggregation time, and actual start/end are in `sdk-progress.txt`.

```sh
COMPARE_GO_MODES=1 SDK_CONTENDERS='tinystore tinystore-server' \
  REPEATS=2 SECONDS_A_STAGE=3 OUT=results/modes sh sdk/sdk.sh
```
