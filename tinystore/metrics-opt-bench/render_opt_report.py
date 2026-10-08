#!/usr/bin/env python3
"""Render the preserved optimization report in English; never run workloads."""
import json
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parent
REPORTS = ROOT.parent / "reports"
DATA = REPORTS / "data/metrics-optimization-2026-10-08"
rows = json.loads((DATA / "summary.json").read_text())
bycase = {row["case"]: row for row in rows}
memory = [json.loads(line) for line in (DATA / "memory.jsonl").read_text().splitlines()]
env = json.loads((DATA / "environment.json").read_text())
variants = ["go", "rust_base", "rust_serial", "rayon1", "rayon2"]


def rss(case, variant):
    return statistics.median(row["max_rss_kib"] for row in memory
                             if row["case"] == case and row["variant"] == variant) / 1024


def timing_table():
    lines = ["| Workload | Go, ms | Baseline Rust, ms | Optimized Rust, ms | Rayon 1, ms | Rayon 2, ms | Baseline / serial | Serial / Rayon 2 |",
             "|---|---:|---:|---:|---:|---:|---:|---:|"]
    for row in rows:
        medians = row["median_ns"]
        values = " | ".join(f"{medians[variant] / 1e6:.4f}" for variant in variants)
        lines.append(f"| {row['case']} | {values} | {row['serial_vs_base']:.2f}× | {row['rayon2_vs_serial']:.2f}× |")
    return "\n".join(lines)


def ablation_table():
    lines = ["| Workload | Refactoring only, ms | Exact accumulator only, ms | Codec/reservation only, ms | Both optimizations, ms | Rayon 2 on CPUs 0,1, ms |",
             "|---|---:|---:|---:|---:|---:|"]
    for row in rows:
        medians = row["median_ns"]
        if "rust_refactor" in medians:
            values = " | ".join(f"{medians[variant] / 1e6:.4f}" for variant in
                                ["rust_refactor", "rust_exact", "rust_codec", "rust_serial", "rayon2_shared_cache"])
            lines.append(f"| {row['case']} | {values} |")
    return "\n".join(lines)


def memory_table():
    lines = ["| Workload | Go | Baseline Rust | Serial Rust | Rayon 1 | Rayon 2 |",
             "|---|---:|---:|---:|---:|---:|"]
    for case in dict.fromkeys(row["case"] for row in memory):
        values = " | ".join(f"{rss(case, variant):.2f}" for variant in variants)
        lines.append(f"| {case} | {values} |")
    return "\n".join(lines)


REPORT_TEMPLATE = """# TinyStore: serial Rust optimizations and Rayon

On this test setup, reducing allocations and using an exact accumulator delivered the main additional gains: `cut_sum`/`cut_avg` became 2.39–2.45 times faster than baseline Rust, and wide aggregates became 2.27–2.43 times faster. Two Rayon threads made a 16-series Read a further 1.30 times faster than optimized serial Rust. Rayon increased latency for wide Stream and several aggregates. These results do not support enabling parallelism for every query.

The measured run used a local, uncommitted research prototype. Production TinyStore was unchanged. The language comparison uses the preceding synchronous Rust prototype; Go calls the real public metrics API. The new run measured both baseline implementations again, so its ratios do not combine results from different sessions. It measures single-request latency, not server throughput under concurrent load. Publishing the preserved report does not turn that preliminary run into a formal measurement round.

Prototype changes:

- Rayon 1.11.0 processes independent series after the SQLite snapshot closes. The Connection remains on the calling thread. Time order and counter state are preserved within each series. Groups are merged exactly, with rounding after the merge.
- A bounded global pool is created before timing. Batches contain at most 8 series on two threads or 4 on one. Results and Stream callbacks are delivered in their original order; the queue does not collect the entire Stream. Normal mode selects Rayon for ≥2 series and ≥4096 samples to decode. Queries that use only summaries remain serial.
- `sum`/`avg` use 35 64-bit words, a 280-byte exact signed accumulator in units of 2^-1074. No BigInt is created for each decoded sample. Summaries continue to use their existing BigInts; the two parts are added exactly at output and use the preceding rounding rules. Count/min/max retain finite-value validation without creating a BigInt.
- Regular clocks fill Sample.at directly, without temporary delta/timestamp arrays. Uncompressed codec payloads are borrowed during decoding. Read reserves bounded result capacity in advance. Compressed payloads and irregular clocks retain the preceding path.

Measurements and correctness:

- 56 scenarios × 9 variants matched FNV hashes, float64 bits, timestamps, buckets, counts/resets/overflow/partial fields and the logical digest. Verification forces Rayon even for short queries. Selection plans and resource limits matched. After every timed write, the file was closed and reread by production Go.
- 21 Rust tests passed: every codec mode, extreme IEEE-754 values, every finite exponent, exact cancellation, signed zero, subnormals, overflow, summary/group merges, callback ordering, consumer stops and the precedence of a global limit over a later decode error. Go vet and cargo fmt --check passed.
- 39 workloads, five passes, 1135 timings with alternating variant order, plus 75 separate RSS processes. Variants used the same immutable fixtures, 32 warm operations and the same fixed operation count in each comparison. Maintain/expiry ran once without warming. Source and binary SHA-256 hashes were checked before and after measurement.
- Interval: 2026-10-08T14:38:21.649283+00:00 to 2026-10-08T14:41:47.773542+00:00. Go 1.27.1, Rust 1.99.0, AMD EPYC 9V74 VM, 3 visible vCPUs, a cgroup quota of 2 CPUs, 8 GiB RAM, overlayfs. Serial variants were pinned to CPU 0; the main Rayon 2 variant used CPUs 0,2. GOMAXPROCS=1. The VM does not reliably establish the host's physical-core topology.
- The kernel reports shared L1/L2 caches for CPUs 0,1 and separate caches for CPU 2. An additional Rayon 2 series on CPUs 0,1 is preserved separately. Differences between the CPU pairs are sometimes visible, limiting conclusions about physical-core scaling. Small differences, including serial write-path differences, cannot be attributed to Rayon: the writer is not parallelized here.
- SQLite 3.53.4, matching source_id and the preceding compile flags/PRAGMAs: 4096-byte pages, WAL/FULL, a 1 MiB cache, mmap=0, fullfsync and checkpoint_fullfsync enabled, foreign_keys=1, busy_timeout=5000, trusted_schema=0, wal_autocheckpoint=1000. Native SQLite retains THREADSAFE=1/Unix VFS, while the translated Go backend uses THREADSAFE=0/custom VFS. That is a preceding implementation difference, not an effect of these optimizations. SQLite/zstd are statically embedded; libm/libgcc_s/libc/the loader remain system libraries. Rayon added no dynamic SQLite dependency.
- The data fit in cache: the sealed fixture is 132 KiB and the wide fixture is 140 KiB. Wide contains 64 gauge series × 1441 samples = 92224 points, with 384 sealed blocks. The original scenarios and 17 floating-point edge scenarios were retained from the full comparison.

The pilot targeted approximately 120 ms of baseline Rust work. Timed registration is capped at 16 operations, scrape at 512, other writes at 4096 and reads/aggregates at 16384. Registration's warm+timed output must stay within the unchanged 100,000-point query ceiling. Both compared variants use the same retained count. One-shot and capped cases can have shorter measurement intervals.

All times below are medians of five passes. A ratio greater than 1 indicates a speedup. Individual pass ranges are available in summary.json; small differences on this VM should be interpreted cautiously.

<!-- TIMING TABLE -->

Disabling optimizations separately helps estimate each component's contribution:

<!-- ABLATION TABLE -->

For `cut_sum`, the exact accumulator alone reduced latency from 2.150 to 0.877 ms relative to the same refactoring with optimizations disabled. Codec changes alone had little effect on this aggregate. For a 16-series Read, codec/reservation changes reduced latency from 2.569 to 2.188 ms. Switching the exact accumulator does not change the Read path; differences between those medians reflect variation.

Peak RSS, median of three processes, MiB:

<!-- MEMORY TABLE -->

Read/aggregate retain 16 results. Stream consumes one series at a time and retains only the last. For Read wide64, reservation reduced peak RSS from 31.32 to 27.51 MiB; Rayon 2 used 27.77 MiB. For Stream wide64, peak RSS grew from 4.39 to 4.79 MiB when two threads were enabled. This is process RSS, including SQLite/zstd/allocators/stacks, not just the Rust heap. GNU time and sampled VmRSS on Linux can differ slightly; the original values are preserved. Parallel queues are bounded, but production shared reservations and a shared concurrent memory budget are not implemented in the prototype.

The practical next step is to carry forward the exact accumulator and remove unnecessary buffers first. Select Rayon according to the amount of actual decoding and the query type; the current threshold is experimental and does not prevent every regression. Production work also requires measurements of concurrent load and a shared bounded pool, so requests do not create competing CPU pools. A serialized SQLite writer does not become faster merely by using par_iter; parallel data preparation before the transaction is a separate experiment.

The preceding Rust prototype's omissions remain: reader pooling/admission, shared reservations, caller cancellation, instruments, runtime directory locking/lifecycle and background tasks were not ported. These results make no claim about cold disk, power-loss durability or linear scaling on 8–16 cores.

Reproduce from `<repo>/tinystore/metrics-opt-bench` following the [harness instructions](../metrics-opt-bench/README.md). [Raw data](data/metrics-optimization-2026-10-08/summary.json) · [Preceding full comparison](metrics-native-2026-10-08.md)

![Optimization comparison](data/metrics-optimization-2026-10-08/optimization.svg)
"""

report = REPORT_TEMPLATE.replace("<!-- TIMING TABLE -->", timing_table())
report = report.replace("<!-- ABLATION TABLE -->", ablation_table())
report = report.replace("<!-- MEMORY TABLE -->", memory_table())
report = report.replace("2026-10-08T14:38:21.649283+00:00", env["started_utc"])
report = report.replace("2026-10-08T14:41:47.773542+00:00", env["finished_utc"])
(REPORTS / "metrics-optimization-2026-10-08.md").write_text(report)

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np
cases=['read_head_full8','read_sealed_full16','read_wide64','aggregate_cut_sum','aggregate_wide_cut_sum','stream_wide64']
labels=['Head / 8 series','Read / 16 series','Read / 64 series','Cut sum / 8 series','Cut sum / 64 series','Stream / 64 series']
fig,ax=plt.subplots(figsize=(10,5.2));x=np.arange(len(cases));width=.25
for j,(v,label,color) in enumerate([('rust_base','Baseline Rust','#a8b2c0'),('rust_serial','Optimized serial','#3279ab'),('rayon2','Optimized + Rayon 2','#35a68a')]):
 values=[bycase[c]['median_ns'][v]/1e6 for c in cases]
 bars=ax.bar(x+(j-1)*width,values,width,label=label,color=color)
 ax.bar_label(bars,fmt='%.2f',fontsize=8,padding=2)
ax.set_xticks(x,labels,rotation=15,ha='right');ax.set_ylabel('Median request time, ms (lower is better)')
ax.set_title('TinyStore metrics: fewer allocations and bounded Rayon')
ax.spines[['top','right']].set_visible(False);ax.legend(frameon=False);ax.grid(axis='y',alpha=.18);ax.set_axisbelow(True)
fig.text(.01,.012,'5 alternating passes • warm native SQLite 3.53.4 • serial: CPU0; Rayon2: CPU0,2 • VM quota: 2 CPUs',fontsize=8,color='#505968')
fig.tight_layout(rect=[0,.055,1,1]);fig.savefig(DATA/'optimization.svg');fig.savefig(DATA/'optimization.png',dpi=180)
print(REPORTS/'metrics-optimization-2026-10-08.md')
