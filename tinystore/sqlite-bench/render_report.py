#!/usr/bin/env python3
"""Render the local SQLite comparison from preserved measurements."""
import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parent
REPORTS = ROOT.parent / "reports"
DATA = REPORTS / "data/sqlite-native-2026-10-07"


def read(name):
    return json.loads((DATA / name).read_text())


def main():
    for name in ["native-build.json", "rust-linkage.txt"]:
        source = ROOT / "generated" / name
        if source.exists():
            shutil.copyfile(source, DATA / name)
    baseline = read("summary.json")
    followup = {r["page_size"]: r for r in read("write-followup-summary.json")}
    inspections = read("inspect.json")
    environment = read("environment.json")
    names = {"point": "Point lookup, 128 B", "point_txn": "Point lookup in read transaction", "range240": "Range, 240 owned rows", "aggregate240": "COUNT + SUM, 240 rows", "update1": "One update + FULL commit", "update64": "64 updates/savepoints + FULL commit"}
    lines = [
        "# SQLite backend comparison — 2026-10-07",
        "",
        "Preliminary local experiment. The harness is uncommitted; source and executable SHA256 hashes are preserved. Production TinyStore is unchanged. This report is a local artifact, not a published research measurement round.",
        "",
        "With 4096-byte pages, Rust/rusqlite with native SQLite completed reads 2.57–3.17× faster than the current TinyStore SQL layer, and 1.58–2.48× faster than direct Go SQLite. The longer single-row FULL commit followup gave a 1.24× median speedup over TinyStore; a transaction containing 64 updates gave 1.44×. These compare complete backends and wrappers; they do not isolate the effect of the Rust language.",
        "",
        "## Measured stacks",
        "",
        "- `go_tinystore`: the real TinyStore `internal/sqlite` package at commit `" + environment["tinystore_commit"] + "`, including `Lookup`, `ViewPrepared`, `UpdatePrepared`, prepared statement caching and the database/sql driver.",
        "- `go_raw`: direct `github.com/ncruces/go-sqlite3` v0.35.6 connections, the same translated Go SQLite backend and owned results. A small statement map holds only the measured programs and never evicts. This also bypasses TinyStore admission and adapter checks, so its difference from TinyStore includes more than database/sql overhead.",
        "- `rust_native`: rusqlite 0.40.1, defaults disabled, `cache`, `limits`, `modern_sqlite`; static custom SQLite 3.53.4 built from the official amalgamation. One owned writer and one owned reader use NO_MUTEX connection handles while the native library retains THREADSAFE=1.",
        "",
        "All three reported SQLite 3.53.4 and identical sqlite_source_id `" + inspections[0]["reader"]["source_id"] + "`. The Go backend translates SQLite through wasm2go ahead of time; it does not run a WASM interpreter during this benchmark.",
        "",
        "## Configuration and input",
        "",
        "The fixture contains 32,768 WITHOUT ROWID rows: 64 series × 512 timestamps, each with a deterministic 128-byte BLOB. Each process receives a fresh byte-for-byte copy of the same closed fixture. Page sizes 1024 and 4096 were measured separately. The files occupy " + str(environment["fixtures"]["1024"]["bytes"]) + " and " + str(environment["fixtures"]["4096"]["bytes"]) + " bytes respectively.",
        "",
        "Reader and writer settings were read back and checked: WAL, synchronous=FULL (2), foreign_keys=1, busy_timeout=5000, fullfsync=1, checkpoint_fullfsync=1, cache_size=-1024, trusted_schema=0, and the intended page size. The reader has query_only=1. The measured connections use a 1 MiB length limit and 32 cached statement slots (raw Go has only its measured programs). DQS is disabled in the builds/configuration. Default wal_autocheckpoint=1000 and mmap_size=0 were recorded on both roles. The fsync flags retain the same setting; fullfsync has platform-specific meaning on Linux.",
        "",
        "Point and range reads produce owned BLOBs; a range also owns its row slice/vector. Aggregate reads return native integer values. Point lookups have implicit single-statement snapshots; point_txn explicitly begins a DEFERRED transaction. Write transactions begin IMMEDIATE. update64 includes 64 SAVEPOINT/UPDATE/RELEASE scopes and one commit. This fixed batch measures amortization, while production concurrent UpdateGrouped scheduling is outside the prototype.",
        "",
        "Before timing, every stack ran 16 operations for each of 12 case/page combinations. Rolling hashes cover all returned bytes and scalar results; a separate digest covers every final database row. All three stacks matched exactly. Verification starts at operation zero without mutating warmup.",
        "",
        "## Time per operation",
        "",
        "Values are microseconds per lookup, range, aggregate or committed transaction. Lower is faster. Main values are medians of three passes; update1 uses the six-pass longer followup. A 64-update transaction is one operation in this table.",
        "",
    ]
    for page in [4096, 1024]:
        lines += [f"### {page}-byte pages", "", "| Workload | TinyStore Go, µs | Direct Go, µs | Rust/native, µs | TinyStore / native | Direct Go / native |", "| --- | ---: | ---: | ---: | ---: | ---: |"]
        for row in baseline:
            if row["page_size"] != page:
                continue
            if row["case"] == "update1":
                row = followup[page]
            m = row["median_ns"]
            lines.append(f"| {names[row['case']]} | {m['go_tinystore']/1000:.3f} | {m['go_raw']/1000:.3f} | {m['rust_native']/1000:.3f} | {row['tinystore_over_native']:.2f}× | {row['raw_go_over_native']:.2f}× |")
        lines.append("")
    lines += [
        "## Measurement method and limits",
        "",
        "The main run contains 108 process-isolated measurements: three balanced Latin-rotation passes × three implementations × two page sizes × six cases. Each process performs 32 warm operations and doubles the iteration count until its final interval is at least 0.15 seconds. Only the final interval is timed; setup/open/fixture copy and full verification hashing are outside it. Normal Go GC and Rust destruction remain inside the measured loops; Rust results are consumed through black_box and Go through an escaping result sink.",
        "",
        "Adaptive calibration carries the operation index and mutated database forward. Thus faster stacks can time different iteration counts, key offsets, WAL lengths and checkpoint phases. Timed runs use the same deterministic workload family, but are not an identical fixed operation trace. Reads become warm sustained scans of a small fixture; these are neither cold-storage latency measurements nor concurrent throughput measurements.",
        "",
        "One initial 1024-byte TinyStore update1 sample was 9073 µs/op versus about 181 and 186 µs/op in its other passes. The raw sample is retained. It triggered a separate balanced six-pass followup of update1, with 0.25-second minimum intervals and unchanged source/binary hashes: 36 additional measurements. Followup 4096-byte ranges were about 169–213 µs TinyStore, 162–270 µs direct Go and 137–188 µs native. Durability/write timing is visibly noisy on this VM; the modest write ratios are preliminary and do not establish disk-independent speedups.",
        "",
        "Native SQLite mirrors the Go build's SQL/planner options, including STAT4, DQS=0, TRUSTED_SCHEMA=0, LIKE_DOESNT_MATCH_BLOBS, STRICT_SUBTYPE and the same feature omissions. Material differences remain: native C compiled by GCC at -O2, native Unix VFS, automatic initialization, THREADSAFE=1/MUTEX_PTHREADS and maximum worker threads=8. Go uses a translated build originally compiled by Clang, custom VFS, explicit initialization, isolated THREADSAFE=0 instances and maximum worker threads=0. No parallel query workers are requested by these cases. Exact options are preserved in inspect.json and native-build.json.",
        "",
        "The prototype exercises TinyStore's SQL layer with a synthetic block table. Public metrics/records ingest/read paths, codecs, grouped-write admission, reader pools under load, cancellation, connection recovery, contention and migration compatibility remain outside this comparison. Per-query improvements cannot be multiplied blindly by the earlier codec results.",
        "",
        "## Process memory snapshot",
        "",
        "Each independent inspection process opened two connections, executed 1000 point lookups and reported current Linux VmRSS. Go also cleared its result sink and ran GC. This is a small warm connection footprint, with one snapshot per case/page, not peak RSS or a loaded-engine memory estimate.",
        "",
        "| Pages | TinyStore Go RSS, MiB | Direct Go RSS, MiB | Rust/native RSS, MiB | Native SQLite allocations, MiB |",
        "| ---: | ---: | ---: | ---: | ---: |",
    ]
    for page in [1024, 4096]:
        by_name = {r["implementation"]: r for r in inspections if r["page_size"] == page}
        values = [by_name[name]["rss_kib"] / 1024 for name in ["go_tinystore", "go_raw", "rust_native"]]
        cm = by_name["rust_native"]["sqlite_memory_used"] / 1048576
        lines.append(f"| {page} | {values[0]:.2f} | {values[1]:.2f} | {values[2]:.2f} | {cm:.2f} |")
    lines += [
        "",
        "SQLite C allocations bypass the Rust global allocator. The current Go backend's linear memory is also mapped outside the Go heap (`sqlite3_wrap/mem_unix.go` uses mmap/mprotect), so Go HeapAlloc does not describe its full SQLite memory. Use process RSS together with SQLite memory/cache counters when evaluating a migration. Direct Go and native reader-cache counters are both around 1 MiB here.",
        "",
        "## Binary and environment",
        "",
        "The Rust executable embeds SQLite: its dynamic dependency list has no libsqlite3.so. libm, libgcc_s, libc and the system loader remain dynamic. A fully static portable executable requires a separate musl/toolchain build and validation; it was not measured here.",
        "",
        f"Environment: {environment['go']}; {environment['rustc']}; AMD EPYC 9V74 VM; CPU affinity {environment['cpu_affinity']}; GOMAXPROCS=1, default GOGC=100; cgroup cpu.max `{environment['cpu_quota']}`; memory limit 8 GiB; Linux amd64; `{environment['filesystem']}` filesystem. No builds or other experiment workloads ran concurrently with the timing loops. Host activity and storage variance are outside the experiment's control.",
        "",
        f"Main round: {environment['started_utc']} to {environment['finished_utc']}. Full environment, compiler flags, fixture/source/binary hashes and followup timestamps are preserved in the data files.",
        "",
        "## Reproduction and evidence",
        "",
        "The source tree and exact commands are in [sqlite-bench/README.md](../sqlite-bench/README.md). Raw outputs:",
        "",
        "- [Environment and hashes](data/sqlite-native-2026-10-07/environment.json)",
        "- [Read-back connection settings and memory](data/sqlite-native-2026-10-07/inspect.json)",
        "- [Result/digest verification](data/sqlite-native-2026-10-07/verification.json)",
        "- [108 raw timings](data/sqlite-native-2026-10-07/timings.jsonl) and [summary](data/sqlite-native-2026-10-07/summary.json)",
        "- [36 write followup timings](data/sqlite-native-2026-10-07/write-followup.jsonl), [summary](data/sqlite-native-2026-10-07/write-followup-summary.json) and [followup environment](data/sqlite-native-2026-10-07/write-followup-environment.json)",
        "- [Native build proof](data/sqlite-native-2026-10-07/native-build.json) and [dynamic dependencies](data/sqlite-native-2026-10-07/rust-linkage.txt)",
        "",
        "The evidence supports a native SQLite prototype for the full metrics/records path and also identifies room to reduce the current Go wrapper's read costs. The migration decision needs real engine workloads and concurrency/cancellation compatibility checks.",
        "",
    ]
    (REPORTS / "sqlite-native-2026-10-07.md").write_text("\n".join(lines))


if __name__ == "__main__":
    main()
