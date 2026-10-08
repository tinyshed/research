#!/usr/bin/env python3
"""Render the completed deep round; this helper was not a measured harness file."""
import argparse
import csv
import json
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parent
REPORTS = ROOT.parent / "reports"
DATA = REPORTS / "data/metrics-deep-2026-10-08"
MAIN = ["go", "rust_previous", "rust_control", "rust_new", "rayon2",
        "rust_lto", "rust_native", "rust_pgo", "pgo_rayon2"]
LABELS = {"go": "Go", "rust_previous": "Previous", "rust_control": "Control",
          "rust_new": "New", "rayon2": "Rayon 2", "rust_lto": "LTO",
          "rust_native": "Native", "rust_pgo": "PGO", "pgo_rayon2": "PGO + Rayon 2"}
ABLATIONS = ["only_bits", "only_buffers", "only_fused", "only_specialized"]
MEMORY_VARIANTS = ["go", "rust_previous", "rust_new", "rayon2", "rust_pgo"]


def jsonl(name):
    return [json.loads(line) for line in (DATA / name).read_text().splitlines()]


def table(headers, rows):
    return "\n".join(["| " + " | ".join(headers) + " |",
                      "| " + " | ".join(["---"] + ["---:"] * (len(headers) - 1)) + " |"]
                     + ["| " + " | ".join(map(str, row)) + " |" for row in rows])


def write_csv(name, headers, rows):
    with (DATA / name).open("w", newline="") as output:
        writer = csv.writer(output, lineterminator="\n")
        writer.writerow(headers)
        writer.writerows(rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--no-plot", action="store_true")
    args = parser.parse_args()
    env = json.loads((DATA / "environment.json").read_text())
    summary = json.loads((DATA / "summary.json").read_text())
    bycase = {row["case"]: row for row in summary}
    timings = jsonl("timings.jsonl")
    memory = jsonl("memory.jsonl")
    profiles = jsonl("profiles.jsonl")
    kernels = jsonl("kernels.jsonl")
    verification = json.loads((DATA / "verification.json").read_text())
    plans = json.loads((DATA / "plans.json").read_text())
    assert (len(summary), len(timings), len(memory), len(profiles), len(kernels),
            len(verification), len(plans)) == (42, 2070, 90, 66, 20, 56, 29)
    assert env["passes"] == 5
    for row in summary:
        for variant, values in row["passes_ns"].items():
            actual = sorted((item for item in timings if item["case"] == row["case"]
                             and item["variant"] == variant), key=lambda item: item["pass_id"])
            assert [item["pass_id"] for item in actual] == [1, 2, 3, 4, 5]
            assert [item["ns_per_op"] for item in actual] == values
            assert statistics.median(values) == row["median_ns"][variant]
            assert all(item["iterations"] == row["iterations"] for item in actual)

    def ms(case, variant):
        return bycase[case]["median_ns"][variant] / 1e6

    def ratio(case, baseline, candidate):
        return ms(case, baseline) / ms(case, candidate)

    def profile_median(case, mask, field, stage=None):
        records = [row for row in profiles if row["case"] == case and row["mask"] == mask]
        assert sorted(row["pass_id"] for row in records) == [1, 2, 3]
        return statistics.median((row["stages"][stage][field] if stage else row[field])
                                 / row["iterations"] for row in records)

    def rss(case, variant, field="max_rss_kib"):
        records = [row for row in memory if row["case"] == case and row["variant"] == variant]
        assert sorted(row["pass_id"] for row in records) == [1, 2, 3]
        return statistics.median(row[field] for row in records) / 1024

    main_table = table(["Workload", "Timed ops"] + [LABELS[v] + ", ms" for v in MAIN],
                       [[row["case"], row["iterations"]]
                        + [f"{row['median_ns'][variant] / 1e6:.4f}" for variant in MAIN]
                        for row in summary])
    write_csv("medians.csv", ["case", "iterations"] + [v + "_ms" for v in MAIN],
              [[row["case"], row["iterations"]]
               + [row["median_ns"][v] / 1e6 for v in MAIN] for row in summary])
    write_csv("timings.csv", ["case", "variant", "pass_id", "iterations", "warm",
                              "ns_per_op", "fixture", "utc", "go_crossread"],
              [[row.get(key, "") for key in ["case", "variant", "pass_id", "iterations",
                "warm", "ns_per_op", "fixture", "utc", "go_crossread"]] for row in timings])
    compiler_cases = ["read_head_full8", "read_sealed_full16", "read_wide64", "stream_wide64",
                      "aggregate_cut_sum", "aggregate_cut_avg", "aggregate_wide_cut_sum",
                      "aggregate_wide_cut_count", "aggregate_wide_grouped_cut_sum"]
    compiler_table = table(["Workload", "New / LTO", "LTO / native", "LTO / PGO", "PGO / PGO+Rayon 2"],
                           [[case] + [f"{ratio(case, a, b):.2f}×" for a, b in
                                      [("rust_new", "rust_lto"), ("rust_lto", "rust_native"),
                                       ("rust_lto", "rust_pgo"), ("rust_pgo", "pgo_rayon2")]]
                            for case in compiler_cases])
    write_csv("compiler-ratios.csv", ["case", "new_over_lto", "lto_over_native", "lto_over_pgo",
                                      "pgo_over_pgo_rayon2"],
              [[row["case"]] + [ratio(row["case"], a, b) for a, b in
               [("rust_new", "rust_lto"), ("rust_lto", "rust_native"),
                ("rust_lto", "rust_pgo"), ("rust_pgo", "pgo_rayon2")]] for row in summary])

    ablation_rows = [row for row in summary if "only_bits" in row["median_ns"]]
    assert len(ablation_rows) == 9
    ablation_variants = ["rust_control"] + ABLATIONS + ["rust_new"]
    ablation_table = table(["Workload", "Control, ms", "Bits only, ms", "Buffers only, ms",
                            "Fused only, ms", "Specialized only, ms", "All four, ms"],
                           [[row["case"]] + [f"{ms(row['case'], v):.4f}" for v in ablation_variants]
                            for row in ablation_rows])
    ablation_ratios = table(["Workload", "Bits only", "Buffers only", "Fused only",
                             "Specialized only", "All four"],
                            [[row["case"]] + [f"{ratio(row['case'], 'rust_control', v):.2f}×"
                             for v in ABLATIONS + ["rust_new"]] for row in ablation_rows])
    write_csv("ablations.csv", ["case", "variant", "median_ns", "control_over_variant"],
              [[row["case"], v, row["median_ns"][v], ratio(row["case"], "rust_control", v)]
               for row in ablation_rows for v in ablation_variants])

    profile_cases = list(dict.fromkeys(row["case"] for row in profiles))
    allocation_table = table(["Workload", "Control calls/op", "New calls/op", "Control requested KiB/op",
                               "New requested KiB/op"],
                              [[case] + [f"{profile_median(case, mask, 'rust_allocation_calls'):.0f}"
                               for mask in [0, 15]]
                               + [f"{profile_median(case, mask, 'rust_requested_bytes') / 1024:.2f}"
                               for mask in [0, 15]] for case in profile_cases])
    stage_table = table(["Workload", "Control total, ms", "New total, ms", "Control snapshot, ms",
                         "New snapshot, ms", "Control process, ms", "New process, ms"],
                        [[case] + [f"{profile_median(case, mask, 'inclusive_ns', stage) / 1e6:.4f}"
                         for stage in ["operation", "snapshot",
                                       "aggregate_process" if case.startswith("aggregate_") else "read_process"]
                         for mask in [0, 15]] for case in profile_cases])
    write_csv("profiles.csv", ["case", "mask", "pass_id", "iterations", "stage", "calls",
                               "inclusive_ns", "rust_allocation_calls", "rust_requested_bytes"],
              [[row["case"], row["mask"], row["pass_id"], row["iterations"], name,
                stage["calls"], stage["inclusive_ns"], stage["rust_allocation_calls"],
                stage["rust_requested_bytes"]] for row in profiles
               for name, stage in row["stages"].items()])
    memory_cases = list(dict.fromkeys(row["case"] for row in memory))
    memory_table = table(["Workload", "Retained sample points"] + [LABELS[v] + ", MiB" for v in MEMORY_VARIANTS],
                        [[case, next(row["retained_samples"] for row in memory if row["case"] == case)]
                         + [f"{rss(case, v):.2f}" for v in MEMORY_VARIANTS] for case in memory_cases])
    write_csv("memory.csv", ["case", "variant", "pass_id", "iterations", "retained_samples",
                             "rss_kib", "max_rss_kib", "heap_alloc_bytes", "heap_sys_bytes", "sqlite_memory_used"],
              [[row.get(key, "") for key in ["case", "variant", "pass_id", "iterations", "retained_samples",
                "rss_kib", "max_rss_kib", "heap_alloc_bytes", "heap_sys_bytes", "sqlite_memory_used"]]
               for row in memory])

    kernel_records = []
    for row in kernels:
        for item in row.get("cases", [row]):
            kernel_records.append({**item, "kernel": row["kernel"], "variant": row["variant"],
                                   "pass_id": row["pass_id"], "iterations": row["iterations"],
                                   "warm": row["warm"]})
    kernel_groups = list(dict.fromkeys((row["kernel"], row.get("width", "Huffman")) for row in kernel_records))
    kernel_table_rows = []
    for kernel, width in kernel_groups:
        records = [row for row in kernel_records if row["kernel"] == kernel and row.get("width", "Huffman") == width]
        assert len(records) == 10
        assert len({(row["input_hash"], row["output_hash"], row["input_bytes"], row["values"]) for row in records}) == 1
        medians = {v: statistics.median(row["ns_per_op"] for row in records if row["variant"] == v)
                   for v in ["scalar", "words"]}
        kernel_table_rows.append(["Huffman" if width == "Huffman" else f"Residual width {width}",
                                  records[0]["values"], records[0]["input_bytes"],
                                  f"{medians['scalar']:.2f}", f"{medians['words']:.2f}",
                                  f"{medians['scalar'] / medians['words']:.2f}×"])
    kernel_table = table(["Kernel", "Output values", "Input bytes", "Scalar, ns/op", "Words, ns/op", "Scalar / words"],
                         kernel_table_rows)
    write_csv("kernels.csv", ["kernel", "width", "variant", "pass_id", "iterations", "warm",
                              "values", "input_bytes", "input_hash", "output_hash", "ns_per_op", "samples"],
              [[row.get(key, "") for key in ["kernel", "width", "variant", "pass_id", "iterations", "warm",
                "values", "input_bytes", "input_hash", "output_hash", "ns_per_op", "samples"]]
               for row in kernel_records])

    passes = ["# Deep metrics round: complete retained passes", "",
              "Generated from the preserved raw output by the unmeasured render_deep_report.py helper.", "",
              "## Public-operation latency", "",
              "Milliseconds per operation. All five passes are shown in pass order; the median is descriptive.", "",
              table(["Workload", "Variant", "Pass 1", "Pass 2", "Pass 3", "Pass 4", "Pass 5", "Median"],
                    [[row["case"], variant] + [f"{value / 1e6:.6f}" for value in values]
                     + [f"{row['median_ns'][variant] / 1e6:.6f}"] for row in summary
                     for variant, values in row["passes_ns"].items()]), "",
              "## Kernel latency", "", "Nanoseconds per complete decoded buffer, five passes per variant.", ""]
    passes.append(table(["Kernel", "Width", "Variant", "Pass 1", "Pass 2", "Pass 3", "Pass 4", "Pass 5"],
                        [[kernel, width, v] + [f"{row['ns_per_op']:.6f}" for row in sorted(kernel_records,
                          key=lambda row: row["pass_id"]) if row["kernel"] == kernel
                          and row.get("width", "Huffman") == width and row["variant"] == v]
                         for kernel, width in kernel_groups for v in ["scalar", "words"]]))
    passes += ["", "## Process memory", "",
               "Three independent processes per variant; peak and sampled RSS are both MiB.", "",
               table(["Workload", "Variant", "Peak 1", "Peak 2", "Peak 3", "Sampled 1", "Sampled 2", "Sampled 3"],
                     [[case, v] + [f"{row[field] / 1024:.6f}" for field in ["max_rss_kib", "rss_kib"]
                      for row in sorted(memory, key=lambda row: row["pass_id"])
                      if row["case"] == case and row["variant"] == v]
                      for case in memory_cases for v in MEMORY_VARIANTS]), "",
               "## Instrumented Rust profiles", "",
               "Three independent 32-operation processes. Calls, requested bytes and instrumented total time are per operation.",
               "Mask 0 is Control and mask 15 is New. All stage values are retained in profiles.csv and profiles.jsonl.", "",
               table(["Workload", "Mask", "Calls 1; 2; 3", "Requested bytes 1; 2; 3", "Instrumented ms 1; 2; 3"],
                     [[case, mask] + ["; ".join(f"{row[key] / row['iterations']:.3f}"
                      for row in sorted(profiles, key=lambda row: row["pass_id"])
                      if row["case"] == case and row["mask"] == mask)
                      for key in ["rust_allocation_calls", "rust_requested_bytes"]]
                      + ["; ".join(f"{row['stages']['operation']['inclusive_ns'] / row['iterations'] / 1e6:.6f}"
                      for row in sorted(profiles, key=lambda row: row["pass_id"])
                      if row["case"] == case and row["mask"] == mask)]
                      for case in profile_cases for mask in [0, 15]]), ""]
    (DATA / "passes.md").write_text("\n".join(passes))

    fixture_table = table(["Fixture", "File bytes", "SHA-256"],
                          [[name, item["bytes"], item["sha256"]] for name, item in env["fixtures"].items()])
    report = f'''# TinyStore: deeper Rust metrics optimizations, compiler variants and bounded Rayon — 2026-10-08

The new portable serial path reduces a 16-series sealed Read from {ms('read_sealed_full16', 'rust_previous'):.3f} to {ms('read_sealed_full16', 'rust_new'):.3f} ms relative to the preceding optimized Rust port remeasured in this session. Portable PGO reaches {ms('read_sealed_full16', 'rust_pgo'):.4f} ms, and PGO with two Rayon workers reaches {ms('read_sealed_full16', 'pgo_rayon2'):.3f} ms; Go takes {ms('read_sealed_full16', 'go'):.3f} ms. Cut average moves from {ms('aggregate_cut_avg', 'rust_previous'):.3f} to {ms('aggregate_cut_avg', 'rust_new'):.3f} to {ms('aggregate_cut_avg', 'rust_pgo'):.3f} ms, versus {ms('aggregate_cut_avg', 'go'):.3f} ms for Go. These are cache-resident single-request measurements on this VM. Production TinyStore is unchanged.

This round measured a committed research harness, [16df44bff8bc7f18313d0806492a7c8304fbc9b6](https://github.com/tinyshed/research/commit/16df44bff8bc7f18313d0806492a7c8304fbc9b6), against TinyStore [e307c48a40126aad0e2873b6bf3aaedef8115483](https://github.com/tinyshed/tinystore/commit/e307c48a40126aad0e2873b6bf3aaedef8115483). The earlier [native-path](metrics-native-2026-10-08.md) and [allocation/Rayon](metrics-optimization-2026-10-08.md) reports preserve preliminary runs of uncommitted prototypes. Their numbers are not combined with this round. The preceding optimized Rust baseline and Go were measured again here.

![Deep metrics comparison](data/metrics-deep-2026-10-08/deep-optimizations.svg)

## What the comparisons isolate

| Variant | Source and switches | Compiler and workers |
| --- | --- | --- |
| Go | Real public metrics API at the pinned TinyStore commit | Go 1.27.1, GOMAXPROCS=1 |
| Previous | The preceding optimized Rust port, preserved in reference-rust; fast exact/codec enabled | Ordinary release, serial |
| Control | New source, all four deep switches disabled | Ordinary release, serial |
| New | New source, all four deep switches enabled | Ordinary release, serial |
| Rayon 2 | New algorithms | Ordinary release, bounded two-worker pool |
| LTO | New algorithms | Portable thin LTO, one codegen unit, serial |
| Native | New algorithms | The same LTO flags plus target-cpu=native, serial |
| PGO | New algorithms | The same portable LTO flags plus profile-use, serial |
| PGO + Rayon 2 | New algorithms | Portable PGO and the bounded two-worker pool |

All Rust variants enable the previous fast exact sum/average and codec options. **New versus Control isolates the four new switches in the same source and executable.** New versus Previous also includes the intervening source and ownership/layout changes. Compiler comparisons use LTO as the baseline for native CPU generation and PGO; comparing those builds only with the ordinary release would mix compiler effects. The telemetry executable is separate and never supplies latency timings.

The independent tuning bits are 1 for word-based packed residual/Huffman readers, 2 for shared packed-head storage and bounded reusable decode scratch, 4 for decoding into validated reusable buffers consumed by query folding, and 8 for operation-specialized aggregate arithmetic. Dispatch occurs before the per-point arithmetic loop; count/min/max avoid unnecessary exact integer construction, while sum/average/delta/increase retain their exact result contracts. Complete selected block or mutable-head validation preserves corruption and output-limit error precedence. Mutable-head body/stored slices share packed storage; worker-local scratch has an explicit cache bound and discards oversized capacity.

Rayon receives owned snapshot data after the SQLite transaction closes. The Connection stays on the calling thread. One global pool has at most two workers, batches contain at most eight series, and indexed consumption preserves series/Stream order, consumer stops and global limits. Counter state remains ordered within each series, and grouped values merge exactly before rounding. Normal timing uses the existing ≥2-series/≥4096-decoded-sample heuristic, not forced parallelism. Verification forces worker execution even on small queries.

## Correctness and traces

Before timing, 39 Rust tests passed, including scalar/word equivalence and malformed-input guards, immutable/shared head storage and scratch rollback/reentrancy, exact arithmetic, every aggregate operation, signed zero, finite extremes, summary/group merges, bucket boundaries, ordered Stream callbacks and error precedence. The harness compared 56 traces across 13 configurations, or 728 configuration-trace checks. Canonical series/labels, timestamps and float bits, all aggregate bucket fields, maintenance outcomes and final logical digests matched. Twenty-nine selection plans and both guard suites matched; guard coverage includes TooOld/TooNew, resource limits, nonfinite aggregates and atomic rollback.

Every process starts from its own byte-identical copy of a closed Go-created fixture named metrics.db. Each timed write closes its file and production Go reopens it outside the timer; final logical digests match across the compared variants in every retained pass. This checks logical format compatibility. It does not claim compressed bytes or whole database files are identical, and this deeper round introduces no storage-size saving claim.

There are 42 public-operation workloads × nine main variants × five passes, plus nine workloads × four independent switch variants × five passes: **2070 retained timings**. A fixed 16-operation pilot on Previous selects a common operation count targeting a 120-ms timed trace; the pilot itself is not a fixed-duration 120-ms measurement. Registration is capped at 16 timed calls, scrape at 512, other writes at 4096 and reads/aggregates at 16384. Registration's 32 warm calls plus timed output must fit the unchanged 100,000-point query ceiling. Maintain/full expiry are fresh-copy, one-call cases without warming; other traces run the same 32 warm calls and operation indices. No adaptive mutating state is transferred between stacks or passes.

The variant order rotates by workload and pass and reverses on alternating passes. Timers include request/input construction, public validation, SQLite/query work, result materialization, Rust destruction or normal Go GC, and durable commit work. Open/migration, fixture copying, verification/full hashes, cross-reading and later inspection are outside timing. Go retains its query sink output through the loop; Rust consumes and destroys each owning Read/aggregate output immediately after black_box. Both retain the final Stream callback result. Go-versus-Rust ratios include these consumer-lifetime differences as well as native/backend and engine differences; they are not an isolated language comparison. Rust-versus-Rust variants use the same output lifetime. No builds, tests or other experiments ran during the retained timing interval; host/storage activity remains outside the harness's control.

## Corpus and SQLite configuration

The synthetic decimal values are float64((sample_index*17+series_id)%997)/10. Timestamp origin is 1700000000000 and frozen normal Now is origin+6000. The sealed/ready fixtures have eight gauge and eight counter series × 4801 points. Maintain seals 4800 points per series into 320 blocks and leaves 16 mutable points; series share compact clocks. The head fixture has eight × 2001 points, scrape has 100 × 240, and empty contains the migrated schema. Wide has 64 gauge series × 1441 points = 92,224 points, with 384 sealed blocks. The separate 12-series × 481-point edge corpus covers constant/change/decimal/XOR values, irregular times, signed zero, subnormals, NaN payloads/infinities, exact cancellation/overflow and counter resets. Edge Read and two valid edge aggregates are timed as well as verified.

The main sealed file is 132 KiB and wide is 140 KiB, inside the 1 MiB SQLite cache. Common clocks and unusually compact decimal bodies favor CPU/backend testing. This is neither a cold-storage test nor a production/high-cardinality corpus. Fixtures are generated locally, never committed; their closed-file hashes are:

{fixture_table}

Both backends report SQLite **3.53.4**, with source_id `2026-07-24 19:02:57 bf7c7f30031888f4e796e429ab3978879485813aaca6f641c7b33e4e09459bcc`. Rust uses rusqlite 0.40.1 and the same pinned custom native SQLite archive compiled with -O2, -fPIC and -pthread; no SQLite optimization-level change is attributed to the Rust compiler variants. The [native builder](../sqlite-bench/build_native.py) and [recorded native build](data/metrics-deep-2026-10-08/native-build.json) retain all custom flags and archive/source/header hashes. Go runs ncruces/go-sqlite3 translated ahead of time through wasm2go, with no interpreted WASM in the measured loops.

Readbacks retain 4096-byte pages, WAL, synchronous=FULL, foreign_keys=1, busy_timeout=5000, fullfsync=1, checkpoint_fullfsync=1, cache_size=-1024, trusted_schema=0, mmap_size=0 and wal_autocheckpoint=1000. Readers are query-only and statement capacity is 32. Metrics keeps SQLite's default length limit, rather than the earlier SQL microbenchmark's artificial 1 MiB limit. Head capacity is raised equally to 1,048,576 samples/16 MiB and input capacity to 100,000 samples/64 MiB; query ceilings remain the production defaults. Retention is 30 days, lateness zero, maximum block span one day and maintenance capacity 64 series. Each stack has one reader and one writer.

Native SQLite retains THREADSAFE=1/global mutexes, automatic initialization and Unix VFS. Go isolates translated THREADSAFE=0 instances and uses its custom VFS. Those are existing backend differences, so the Go/native ratio is not an isolated language comparison. SQLite and zstd are statically embedded; [readelf evidence](data/metrics-deep-2026-10-08/linkage.txt) shows no dynamic libsqlite3.so or libzstd.so dependency. System libm/libgcc_s/libc/the loader remain dynamic. Native Huffman uses private symbols from the pinned zstd build and needs production ABI/build review.

Plans explain why aggregate gains differ: whole sum uses 160 summaries and decodes only eight head points; cut sum/average use 80 summaries and decode 19,208 points. Read16 decodes 76,816 points. Wide cut aggregates use 192 summaries and decode 46,144 points. Narrow reads still fetch/validate/decode the required complete block or head chunk. The [29 plans](data/metrics-deep-2026-10-08/plans.json) preserve all selected series, blocks, summary shortcuts, fetched bytes and decoded counts.

## All public-operation medians

Milliseconds per public call, median of five passes, lower is faster. A scrape call contains 100 input points; registration creates eight series × 240 points per call. All nine main variants are shown. [medians.csv](data/metrics-deep-2026-10-08/medians.csv) preserves unrounded values; [passes.md](data/metrics-deep-2026-10-08/passes.md) shows every retained pass, including ablations. [timings.csv](data/metrics-deep-2026-10-08/timings.csv) and the original [JSONL](data/metrics-deep-2026-10-08/timings.jsonl) retain counts, order, UTC and write cross-reader digests.

{main_table}

For Read16, Control/New is {ratio('read_sealed_full16', 'rust_control', 'rust_new'):.2f}×; Previous/New is {ratio('read_sealed_full16', 'rust_previous', 'rust_new'):.2f}×. For cut average, those ratios are {ratio('aggregate_cut_avg', 'rust_control', 'rust_new'):.2f}× and {ratio('aggregate_cut_avg', 'rust_previous', 'rust_new'):.2f}×. The new switches give additional decoded-path gains, while whole-block aggregates have very little point work to remove and show no uniform portable-release improvement.

Durable writes are small and noisy here. Scrape changes from {ms('ingest_scrape100', 'rust_previous'):.3f} ms on Previous to {ms('ingest_scrape100', 'rust_new'):.3f} ms on New; the deeper switches establish no added scrape gain. Rayon does not parallelize the writer, so variation between its write rows cannot be credited to workers. For example, Control append spans {min(bycase['ingest_append1']['passes_ns']['rust_control'])/1e6:.3f}–{max(bycase['ingest_append1']['passes_ns']['rust_control'])/1e6:.3f} ms and New spans {min(bycase['ingest_append1']['passes_ns']['rust_new'])/1e6:.3f}–{max(bycase['ingest_append1']['passes_ns']['rust_new'])/1e6:.3f} ms. Retained FULL fsync/checkpoint behavior and the shared VM limit causal conclusions from small write differences.

## Compiler effects and held-out PGO work

LTO flags are `-C lto=thin -C embed-bitcode=yes -C codegen-units=1`. Native adds `-C target-cpu=native`. PGO adds profile-use to that portable LTO configuration, with no target-cpu=native. The training executable is a separate instrumented build. Training runs head8, sealed8, cut sum and count for 256 calls each, scrape for 64, and Maintain once; other than Maintain, training has 32 warm calls. Wide64, full16, IEEE edge traces, prefix/NoneOf and grouped queries are held out. They still use the same small synthetic value distributions and code families, so held-out improvement does not establish generalization to unrelated production workloads.

The merged training profile SHA-256 is `{env['build']['profile_sha256']}`. Compiler flags, training traces, profile/tool versions and executable hashes are in [environment.json](data/metrics-deep-2026-10-08/environment.json). Ratios below divide the named baseline by its candidate; greater than one is faster. The native and PGO comparisons therefore use LTO, not the default build.

{compiler_table}

PGO improves Read16 {ratio('read_sealed_full16', 'rust_lto', 'rust_pgo'):.2f}× and cut average {ratio('aggregate_cut_avg', 'rust_lto', 'rust_pgo'):.2f}× relative to LTO. Host-specific generation is uneven: Read16 is {ratio('read_sealed_full16', 'rust_lto', 'rust_native'):.2f}×, while cut average is {ratio('aggregate_cut_avg', 'rust_lto', 'rust_native'):.2f}×. Thin LTO alone also has mixed results. These are separate deployment/build choices, not automatic additions to a portable default. [compiler-ratios.csv](data/metrics-deep-2026-10-08/compiler-ratios.csv) covers every workload.

Two workers help PGO Read16 a further {ratio('read_sealed_full16', 'rust_pgo', 'pgo_rayon2'):.2f}×, but make PGO wide cut sum slower ({ratio('aggregate_wide_cut_sum', 'rust_pgo', 'pgo_rayon2'):.2f}×) and wide Stream slightly slower ({ratio('stream_wide64', 'rust_pgo', 'pgo_rayon2'):.2f}×). The current threshold does not prevent those regressions. A two-CPU quota and virtual topology cannot establish scaling to 8–16 physical cores or concurrent server throughput.

## Independent switch ablations

These four variants enable only the named deep switch; the previous fast exact/codec options remain on. They share Control's ordinary release executable. All-four effects need not equal a product of isolated effects.

{ablation_table}

Control divided by each candidate:

{ablation_ratios}

Bits alone helps Read16 {ratio('read_sealed_full16', 'rust_control', 'only_bits'):.2f}× in this run; buffers alone helps {ratio('read_sealed_full16', 'rust_control', 'only_buffers'):.2f}× and all four help {ratio('read_sealed_full16', 'rust_control', 'rust_new'):.2f}×. Specialization alone helps wide cut sum {ratio('aggregate_wide_cut_sum', 'rust_control', 'only_specialized'):.2f}×. Several isolated effects are weak or negative. A useful noise check is specialization-only on Read16: its arithmetic specialization is unused by Read, yet the median moves by about 8%. Small differences between switches therefore need the retained ranges, not a universal mechanism claim. [ablations.csv](data/metrics-deep-2026-10-08/ablations.csv) and full pass values preserve those comparisons.

## Residual and Huffman kernels

The microbenchmarks run the same ordinary-release executable on CPU 0 with scalar/word switching only. Each mode has five scalar and five word processes, 2048 timed decodes and 32 warm decodes per process. Residual inputs contain 239 fixed-width values; Huffman reconstructs 4096 bytes from a 1406-byte native Huffman message. Fixture generation, compression and validation precede the timer. Timed decoding includes fresh output ownership and destruction. The renderer asserts matching input/output hashes for scalar and word variants in every case; [kernels.jsonl](data/metrics-deep-2026-10-08/kernels.jsonl) and [kernels.csv](data/metrics-deep-2026-10-08/kernels.csv) retain the hashes, counts and all five passes.

{kernel_table}

Residual decode gains range from 1.30× at one bit to 17.65× at 64 bits; the Huffman case gains 3.31×. These isolated gains do not multiply into request speedups. End-to-end requests also match/fetch labels, parse directories, process summaries and allocate/materialize output, and this compact decimal corpus does not exercise each residual width equally.

## Instrumented time and Rust allocation requests

Eleven workloads have three Control and three New profiling processes, 32 warm operations and 32 instrumented operations each: 66 processes. Medians below divide counters by the 32 operations. The separate telemetry build counts successful Rust alloc/alloc_zeroed/realloc requests, and requested bytes count every reallocation's new size in full. These are allocation traffic, not live heap, retained bytes or process RSS. Native SQLite/zstd/Huffman C allocations bypass this Rust allocator counter.

{allocation_table}

The requested bytes for Read16 fall from {profile_median('read_sealed_full16', 0, 'rust_requested_bytes')/1024:.2f} to {profile_median('read_sealed_full16', 15, 'rust_requested_bytes')/1024:.2f} KiB/op, while allocation calls move from {profile_median('read_sealed_full16', 0, 'rust_allocation_calls'):.0f} to {profile_median('read_sealed_full16', 15, 'rust_allocation_calls'):.0f}. Cut sum requests fall from {profile_median('aggregate_cut_sum', 0, 'rust_requested_bytes')/1024:.2f} to {profile_median('aggregate_cut_sum', 15, 'rust_requested_bytes')/1024:.2f} KiB/op. Those reductions establish less Rust allocation traffic on these paths; they do not establish a corresponding RSS reduction.

Stage times are **inclusive, instrumented and overlapping**. Snapshot contains registry/head/group work; process contains decode/folding; fold_series contains decoded blocks as well as arithmetic. They are neither additive CPU samples nor estimates of exclusive functions. Instrumentation and allocator bookkeeping change timing; use the separate uninstrumented table for latency claims. Process below means read_process or aggregate_process as appropriate.

{stage_table}

For instrumented cut sum, snapshot is nearly unchanged at {profile_median('aggregate_cut_sum', 0, 'inclusive_ns', 'snapshot')/1e6:.3f}/{profile_median('aggregate_cut_sum', 15, 'inclusive_ns', 'snapshot')/1e6:.3f} ms, while aggregate processing falls from {profile_median('aggregate_cut_sum', 0, 'inclusive_ns', 'aggregate_process')/1e6:.3f} to {profile_median('aggregate_cut_sum', 15, 'inclusive_ns', 'aggregate_process')/1e6:.3f} ms. Instrumented whole count is dominated by snapshot work and its total even increases between those profile medians; less folding time does not imply a universal total-query win. [profiles.csv](data/metrics-deep-2026-10-08/profiles.csv) preserves all nested stage counts, time and allocation requests, with three-process totals in [passes.md](data/metrics-deep-2026-10-08/passes.md).

## Process memory with caller-owned output

Six workloads × five variants × three independent processes give 90 RSS runs. Each process retains 16 Read/aggregate results. Go runs GC before sampled RSS; Rust drops temporary results normally. Stream consumes one series at a time and retains only the final callback result. Its zero retained_samples means no accumulated query outputs, not that the final series owns no samples. The table shows median GNU time peak RSS, with sampled VmRSS and every process preserved in [memory.csv](data/metrics-deep-2026-10-08/memory.csv), [JSONL](data/metrics-deep-2026-10-08/memory.jsonl) and full passes.

{memory_table}

Read16 retains 1,229,056 timestamp/value points, about 18.75 MiB of content before containers; wide Read retains 1,475,584 points, about 22.52 MiB. Deep optimizations reduce allocation traffic but do not produce a clear further peak-RSS saving over Previous here. Stream wide64 grows from {rss('stream_wide64', 'rust_previous'):.2f} to {rss('stream_wide64', 'rust_new'):.2f} MiB and to {rss('stream_wide64', 'rayon2'):.2f} MiB with workers. Process RSS includes SQLite/zstd, native and language allocators, reusable pages, output ownership and stacks. Linux sampled VmRSS and ru_maxrss can differ slightly. These explicit retention scenarios establish neither intrinsic language RAM costs nor bounds for an arbitrary application.

## Environment, reproduction and limits

Retained timing/memory/kernel interval: **{env['started_utc']} to {env['finished_utc']} UTC**. Diagnostic profiles preceded that interval. Linux 6.18.44 x86_64, glibc 2.41, AMD EPYC 9V74 KVM VM, three visible vCPUs, cpu.max `200000 100000` (two-CPU quota), memory limit 8 GiB, overlayfs. Serial variants and kernels use CPU 0; Rayon uses CPUs 0,2, selected for different reported L1 cache groups. Guest topology does not reliably establish physical host cores. Go is 1.27.1 with GOMAXPROCS=1/default GOGC=100. Rust is 1.99.0 (b940084d7, 2026-09-28), LLVM 23.1.1, GNU/Linux target. SQLite was compiled by GCC 14.2.0 with the pinned -O2 native flags. Full OS/CPU/compiler output, fixture hashes, measured source hashes and all executable/profile digests are preserved in the [environment](data/metrics-deep-2026-10-08/environment.json) and [verified source/binary hashes](data/metrics-deep-2026-10-08/verified-hashes.json).

From `<repo>/tinystore/metrics-max-bench`, follow the [reproduction README](../metrics-max-bench/README.md) to initialize the measured TinyStore submodule, regenerate fixtures/native SQLite, build the preceding/new/compiler/telemetry variants, run tests, verify and run the quiet fixed traces. The measured harness commit records source before timing. Source and executable hashes were checked unchanged after the completed run. Fixtures, executable files and generated C/PGO outputs remain local and are excluded from publication. Embedded PGO path strings in the published evidence use `<repo>/tinystore/metrics-max-bench/pgo/...`; numeric measurements and embedded digests are unchanged.

This report, CSV/pass tables and standalone SVG/PNG are generated by [render_deep_report.py](../metrics-max-bench/render_deep_report.py), an **unmeasured documentation helper added after the run**. It reads preserved output and never executes benchmark workloads. Regenerate with `python3 render_deep_report.py`; Matplotlib/NumPy are needed for plots. The later publication manifest records original versus sanitized evidence and renderer artifacts.

Publication compatibility: moving the source submodule to the measured e307c48 commit also required adapting the older bench/compare, bench/perf and bench/records modules to the current metrics Series/Range fields and records Scan API, plus dependency tidying in bench/kv, bench/perf, bench/records, server-spike and spike. All 16 legacy and current Go modules compiled successfully against the publication pin. This is repository build compatibility, not a remeasurement of historical reports; production source remains unchanged.

The Rust port implements synchronous storage/query paths, query budgets and snapshot deadlines. It still omits production reader pooling, admission and shared memory reservations, caller cancellation, instruments, runtime directory locks/lifecycle, background maintenance scheduling, snapshots/backups and server APIs. Partial retention/group merge/quarantine code is not a substitute for fault and concurrency verification. No conclusion is made about cold I/O, power-loss durability, production concurrent throughput, wide physical-core scaling or a complete migration.

The next implementation candidate is the measured serial decoding/allocation work, with exact/error contracts retained. PGO merits separate production-distribution training and held-out measurement before choosing it; native CPU code generation remains a host-specific build option. Rayon needs workload-sensitive selection and shared concurrent resource limits: it accelerates several reads and regresses several aggregates even within this small corpus. Durable write preparation and the serial SQLite writer need separate experiments; the deeper query switches establish no additional scrape benefit.
'''
    (REPORTS / "metrics-deep-2026-10-08.md").write_text(report)

    if not args.no_plot:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt

        plt.rcParams.update({"font.size": 8, "svg.hashsalt": "metrics-deep-2026-10-08"})
        selected = ["read_head_full8", "read_sealed_full16", "read_wide64", "stream_wide64",
                    "aggregate_cut_avg", "aggregate_wide_cut_sum"]
        colors = ["#89939f", "#b4bdc8", "#c9ced6", "#397bad", "#58a38a",
                  "#8e78b3", "#b58951", "#c45459", "#7051a1"]
        fig, axes = plt.subplots(2, 3, figsize=(14, 9))
        for axis, case in zip(axes.flat, selected):
            values = [ms(case, v) for v in MAIN]
            bars = axis.barh([LABELS[v] for v in MAIN], values, color=colors)
            axis.invert_yaxis()
            axis.bar_label(bars, labels=[f"{value:.3f}" for value in values], padding=3, fontsize=8)
            axis.set_xlim(0, max(values) * 1.19)
            axis.set_title(case, fontsize=10)
            axis.set_xlabel("Median request time, ms (lower is faster)")
            axis.grid(axis="x", alpha=0.18)
            axis.set_axisbelow(True)
            axis.spines[["top", "right"]].set_visible(False)
        fig.suptitle("TinyStore metrics: deeper serial changes, compiler variants and bounded Rayon", fontsize=14)
        fig.text(0.01, 0.012, "Five interleaved passes · committed harness · warm SQLite 3.53.4 · "
                 "serial CPU 0 / workers CPUs 0,2 · VM quota: 2 CPUs", fontsize=9, color="#505968")
        fig.tight_layout(rect=[0, 0.04, 1, 0.965])
        fig.savefig(DATA / "deep-optimizations.svg", metadata={"Date": None})
        fig.savefig(DATA / "deep-optimizations.png", dpi=180)
        plt.close(fig)
    print(REPORTS / "metrics-deep-2026-10-08.md")
    print("42 median rows; 2070 timing rows; 9 ablations; 11 profiles; 6 RSS workloads; 9 kernel cases")


if __name__ == "__main__":
    main()
