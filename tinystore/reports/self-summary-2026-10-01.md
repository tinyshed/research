# Opt-In Self Reports and Exact Aggregate Summaries

Draft on `research/self-summary-measurements`; the production experiment is
on `research/self-metrics-summary`, branched from the checked runtime, not main.
No cloud machine was recreated. The measured candidate is
`8bf3824164cea4c8758aaabdce1c863f27828c34`; baseline is
`48cdcd1085d4c09350081d8900fc7a6c85cab276`. Harness is `d4303d5`.

| Question | Answer | What follows |
|---|---|---|
| Can self-metrics ship without importing engines into root? | Optional Reporter/SelfWriter interfaces collect bounded reports into the metrics engine. | Enable explicitly; report memory reservations, not RSS. |
| Can whole-block sums preserve exact answers? | Canonical integer summaries in directory v4 agree with the raw path; boundaries and old blocks still decode. | Use summaries only after range/retention/bucket eligibility. |
| What is the query gain? | Whole sum 9.38x and increase 10.29x on the named fixture, eight interleaved runs each. | Do not attribute it to raw reads or every aggregate shape. |
| What is the file cost? | TSBS file +126,976 bytes (+2.79%), all in the groups b-tree. | Preserve the per-object accounting and the raw payload. |

## Environment and Reproduction

Local Linux amd64 container on Ryzen 7 7700, 16 visible CPUs, Docker Desktop
29.6.2, WSL2 kernel 6.18.33.2, Go 1.27.1, `CGO_ENABLED=0`.
All builds finish before measurement. Both source states come from Git
archives and use the candidate's identical `shortcut_bench_test.go`; binaries
are compiled once. Each benchmark creates a fresh database on the Docker
named data volume. No build, test or lint runs on the host during timing.

The fixture has eight series of 4,801 samples each, at 1-ms intervals.
Values are `((i*17+series_id)%997)/10`. Sum uses gauges; increase uses counters
with resets. The whole-range bucket is 5 seconds. The cut test moves From by
1 ms and uses 481-ms buckets. One-series heads remain queryable.
There are eight one-second benchmark runs per revision and shape. Order is
base/candidate, candidate/base, repeated four times. All output is retained
in [data/self-summary-2026-10-01](data/self-summary-2026-10-01/).

```sh
docker run --rm -v <baseline>:/base -v <candidate>:/candidate:ro \
  -v <research>:/src -v <corpus>:/corpus:ro \
  -v summary-go:/go -v summary-cache:/root/.cache/go-build \
  -v summary-data:/data -e GOWORK=off -e CGO_ENABLED=0 \
  -e GOFLAGS=-buildvcs=false -e BASE_COMMIT=48cdcd1 \
  -e CANDIDATE_COMMIT=8bf3824 -e HARNESS_COMMIT=d4303d5 \
  -e OUT=results/<round> -w /src/tinystore/bench/aggregate-summary \
  golang:1.27 sh run.sh
```

The density directories must be new. Corpora are not committed.

## Aggregate Measurements

Medians of eight runs per cell; allocations are Go allocations, not RSS.

| Query | Baseline us/op | Candidate us/op | Speed ratio | Baseline bytes/op | Candidate bytes/op |
|---|---:|---:|---:|---:|---:|
| Whole sum | 2,408.35 | 256.82 | 9.38x | 1,809,123 | 386,532 |
| Whole counter increase | 2,945.09 | 286.12 | 10.29x | 1,814,541 | 454,874 |
| Cut buckets, sum | 2,385.89 | 1,392.99 | 1.71x | 1,859,533 | 1,218,764 |

Whole-sum allocation count falls from 5,401 to 2,201; counter increase from
5,418 to 3,003. Cut queries still decode boundary blocks; their mix contains
eligible complete blocks too. No silent downsampling is involved. A whole
block contributes only when all its samples fit the clipped range and one
output bucket. A summary spends directory/clock/block budgets but not raw
payload or decoded-sample budgets. Output still limits buckets.

## Real File Cost

TSBS input SHA-256 is
`e4f502af7b0b2ff2c4dba92057a8f2b95e636882f3cb9900986e013d189572cf`:
2,020 series, 5,090,400 samples. The public-corpus gate ingests and maintains
whole series, not the earlier comparison's ten-minute windows. Its absolute
file size must not replace that earlier state in the README.

| Object | Baseline bytes | Candidate bytes |
|---|---:|---:|
| Entire closed file | 4,554,752 | 4,681,728 |
| `groups` | 409,600 | 536,576 |
| `payloads` | 3,076,096 | 3,076,096 |
| `series_state` | 446,464 | 446,464 |
| Other b-trees, individually retained in raw output | unchanged | unchanged |

The file grows 2.79%; bytes/sample move from 0.894773 to 0.919717. The entire
delta belongs to directory pages, not a hidden new index. Values, clocks,
head count (242,400) and payload pages do not change. Both reopened corpus
reads preserve every sample bit and check 64 exact aggregates.

The single corpus passes take 12.34/12.73 seconds for ingest+maintenance,
and 1.30/1.29 seconds for reopened bitwise reads. These are checks and one
pass per revision, not a repeated estimate of ingest/read speed. The new
summary computation has work and storage cost; only aggregate timings above
have the repeated A/B comparison.

## Self Reports and Correctness

`Options.SelfMetrics` is off by default. When enabled, root schedules a
15-second collection only through its own background lifecycle. Manual
stores call `FlushSelfMetrics`. Available metrics/records counters and the
root memory budget are saved as ordinary fixed-label series; other engines
do not yet report. At most 256 measurements/engines and 64-character names
are accepted, duplicate series are refused, and integer values above 2^53
are never silently rounded. A final collection precedes engine shutdown and
is bounded to five seconds. User ingest/rejection counts exclude self-writes.

The final records-stats follow-up counts damaged rows under its existing
lock without copying the whole diagnostic list. Its test proves that Stats
allocates nothing. That follow-up does not touch the measured aggregation
or format code; the timed revision above remains unchanged.

Version four adds exact integer sum/increase while retaining explicit payload
addresses. It has canonical bounded encoding, a golden vector, corrupt-field
tests, and raw/summary differential cases for all five operations. Versions
two/three remain readable through raw fallback. Extreme exponent mixtures
split a group before its expanded directory could exceed the bound. Unused
raw rows are not fetched; raw and boundary reads still check their payloads.

`task check` passes on Windows. All root/server/tool tests ran on Linux with
`-race -count=3 -shuffle=on`; focused self/summary gates also ran thirty times
with race. Linux/Darwin root lint passes; Darwin was not executed. SDK checks
pass 217 Bun and 188 Python tests, Python coverage 85.54%. Exact-summary fuzz
ran twenty seconds, 1,676,015 executions, without a failure.

The new behavior's tests were proven: disabling summary selection makes the
decoded-budget gate fail, and counting self-writes makes its feedback gate
fail. The code was restored and those tests rerun. The size probe adds 40 KiB:
13,080 KiB total, import delta 11,608 KiB.

## What Follows

The experiment provides both features on the new branch. Review the public
telemetry API and directory-v4 change before merging it into the runtime
branch. Do not present its eight-series query gain as an application-wide
speedup. More self-reporting engines, an export endpoint and long steady
state remain separate work, not claims made by this implementation.
