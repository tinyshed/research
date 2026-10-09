#!/usr/bin/env python3
"""Bounded, phased density/performance/retention study. Run only in a quiet window."""
import argparse
from collections import defaultdict
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import sqlite3
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parent
VARIANTS = ["baseline", "count1", "count2", "count4", "count8", "count16", "count32", "bytes1024", "bytes2048", "bytes4096", "bytes8192", "bytes16384"]
CASES = ["point", "sparse", "range", "full", "summary"]


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(1 << 20):
            h.update(chunk)
    return h.hexdigest()


def run(binary, *args):
    result = subprocess.run(["taskset", "-c", "0", str(binary), *map(str, args)], check=True, capture_output=True, text=True)
    return [json.loads(line) for line in result.stdout.splitlines() if line]


def physical(path):
    # This reader is deliberately outside measured native operations. It uses
    # system SQLite only for the dbstat virtual table omitted by the matched
    # measured archive; it never mutates/checkpoints the benchmark files.
    with sqlite3.connect(path.as_uri() + "?immutable=1", uri=True) as db:
        names = ["name", "pagetype", "pages", "bytes", "cell_payload", "unused", "cells", "max_payload"]
        rows = db.execute("SELECT name,pagetype,count(*),sum(pgsize),sum(payload),sum(unused),sum(ncell),max(mx_payload) FROM dbstat GROUP BY name,pagetype ORDER BY name,pagetype")
        objects = [dict(zip(names, row)) for row in rows]
        return {"reader_sqlite_version": sqlite3.sqlite_version, "objects": objects, "owned_pages": sum(o["pages"] for o in objects)}


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def append(path, row):
    with path.open("a") as f:
        f.write(json.dumps(row) + "\n")


def environment(args):
    files = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "build.rs", *sorted((ROOT / "src").rglob("*.rs")), ROOT / "run.py"]
    imported = ROOT.parent / "metrics-bench" / "rust" / "src"
    files += [imported / "codec.rs", imported / "exact.rs", *sorted((imported / "codec").rglob("*.rs"))]
    return {
        "utc": datetime.now(timezone.utc).isoformat(), "uname": platform.uname()._asdict(),
        "binary_sha256": digest(args.binary),
        "source_sha256": {str(p.relative_to(ROOT.parent)): digest(p) for p in files},
        "research_commit": os.environ.get("RESEARCH_COMMIT") or subprocess.check_output(["git", "-C", str(ROOT), "rev-parse", "HEAD"], text=True).strip(),
        "tinystore_commit": os.environ.get("TINYSTORE_COMMIT") or subprocess.check_output(["git", "-C", str(ROOT.parent / "source"), "rev-parse", "HEAD"], text=True).strip(),
        "cpuinfo": next(line for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")),
        "cpu_max": Path("/sys/fs/cgroup/cpu.max").read_text().strip(),
        "memory_max": Path("/sys/fs/cgroup/memory.max").read_text().strip(),
        "native_build": json.loads((ROOT.parent / "sqlite-bench" / "generated" / "native-build.json").read_text()),
        "linkage": subprocess.check_output(["ldd", str(args.binary)], text=True),
        "versions": {name: subprocess.check_output(command, text=True).strip() for name, command in {
            "rustc": ["/work/cargo/bin/rustc", "--version"], "go": ["/usr/local/go/bin/go", "version"], "python": ["python3", "--version"]}.items()},
        "note": "Exploratory uncommitted harness explicitly authorized; content/binary hashes pin the executed sources. Native SQLite stock archive; source/build hashes retained. CPU 0. No DB/VFS/strace instrumentation in retained speed timings.",
    }


def density(args):
    suffix="-format51" if "compact-" in args.variants else ""
    write(args.output / f"environment-density{suffix}.json", environment(args))
    for dataset in args.datasets.split(","):
        original = args.inputs / dataset / "metrics.db"
        manifest = json.loads(original.with_name("manifest.json").read_text())
        write(args.output / f"{dataset}-manifest.json", manifest)
        original_stats = run(args.binary, "inspect", original)[0]
        original_stats["physical"] = physical(original)
        write(args.output / f"{dataset}-original.json", original_stats)
        start = time.monotonic()
        for variant in args.variants.split(","):
            file = args.work / dataset / f"{variant}.db"
            result = run(args.binary, "build", original, file, variant)[0]
            result.update(dataset=dataset, variant=variant)
            result["inspect"]["physical"] = physical(file)
            result["file_sha256"] = digest(file)
            append(args.output / "density.jsonl", result)
            check=run(args.binary, "verify-bytes", original, file)[0]
            check.update(dataset=dataset,variant=variant)
            append(args.output / "verification-bytes.jsonl",check)
            print(json.dumps({"phase": "density", "dataset": dataset, "variant": variant, "file_bytes": result["inspect"]["file_bytes"], "elapsed_s": time.monotonic()-start}), flush=True)
        # Deterministic size control: same source bytes, same fresh row order.
        repeat = args.work / dataset / "baseline-repeat.db"
        if repeat.exists():
            continue
        result = run(args.binary, "build", original, repeat, "baseline")[0]
        result.update(dataset=dataset, variant="baseline-repeat")
        result["inspect"]["physical"] = physical(repeat)
        append(args.output / "density.jsonl", result)


def verify(args):
    for dataset in args.datasets.split(","):
        original = args.inputs / dataset / "metrics.db"
        for variant in args.variants.split(","):
            result = run(args.binary, "verify", original, args.work / dataset / f"{variant}.db")[0]
            result.update(dataset=dataset, variant=variant)
            append(args.output / "verification.jsonl", result)
            print(json.dumps({"phase": "verify", "dataset": dataset, "variant": variant, "samples": result["samples"]}), flush=True)


def cells(args, dataset):
    for variant in args.variants.split(","):
        for mode in (["whole"] if variant.endswith("baseline") or variant.endswith("unpacked") else ["whole", "range"]):
            for cache in ["warm", "cold"]:
                for case in CASES:
                    yield variant, mode, cache, case


def pilot(args):
    plans = {}
    for dataset in args.datasets.split(","):
        timings = defaultdict(list)
        for variant, mode, cache, case in cells(args, dataset):
            result = run(args.binary, "bench", args.work / dataset / f"{variant}.db", mode, case, cache, 16)[0]
            result.update(dataset=dataset, variant=variant, phase="pilot",stage="short")
            append(args.output / "pilot.jsonl", result)
            confirmation_count=max(16,min(100_000,math.ceil(20_000_000/result["ns_per_op"])))
            result=run(args.binary, "bench", args.work / dataset / f"{variant}.db", mode, case, cache, confirmation_count)[0]
            result.update(dataset=dataset,variant=variant,phase="pilot",stage="confirmation")
            append(args.output / "pilot.jsonl",result)
            timings[case, cache].append(result["ns_per_op"])
        for (case, cache), times in timings.items():
            count = max(1, min(math.ceil(150_000_000/min(times)), math.floor(1_000_000_000/max(times))))
            plans[f"{dataset}/{case}/{cache}"] = {"iterations": count, "pilot_min_ns": min(times), "pilot_max_ns": max(times), "projected_total_6passes_s": count * sum(times) * 6 / 1e9}
    write(args.output / "plan.json", plans)
    print(json.dumps({"phase": "pilot_complete", "projected_timed_seconds": sum(p["projected_total_6passes_s"] for p in plans.values()), "plan": plans}), flush=True)


def performance(args):
    write(args.output / "environment-performance.json", environment(args))
    plans = json.loads((args.output / "plan.json").read_text())
    for pass_number in range(6):
        for dataset in args.datasets.split(","):
            # All implementations for a case/cache adjacent; reverse each pass.
            implementations = [(v, m) for v in args.variants.split(",") for m in (["whole"] if v.endswith("baseline") or v.endswith("unpacked") else ["whole", "range"])]
            for cache in ["warm", "cold"]:
                for case in CASES:
                    order = implementations if (pass_number + CASES.index(case)) % 2 == 0 else implementations[::-1]
                    for variant, mode in order:
                        count = plans[f"{dataset}/{case}/{cache}"]["iterations"]
                        result = run(args.binary, "bench", args.work / dataset / f"{variant}.db", mode, case, cache, count)[0]
                        result.update(dataset=dataset, variant=variant, pass_number=pass_number, utc=datetime.now(timezone.utc).isoformat())
                        append(args.output / "timings.jsonl", result)
            print(json.dumps({"phase": "performance", "dataset": dataset, "pass": pass_number}), flush=True)
    summarize(args.output)


def diagnostics(args):
    for dataset in args.datasets.split(","):
        for variant, mode, cache, case in cells(args, dataset):
            path = args.work / dataset / f"{variant}.db"
            result = run(args.binary, "diagnose", path, mode, case, cache)[0]
            result.update(dataset=dataset, variant=variant)
            append(args.output / "diagnostics.jsonl", result)
            if cache == "cold" and case in ["point", "full"]:
                output = args.output / f"strace-{dataset}-{variant}-{mode}-{case}.txt"
                # Includes process startup + selection + 8 warmups + one cold
                # query. It is syscall evidence only, not isolated query I/O.
                subprocess.run(["strace", "-qq", "-c", "-e", "trace=pread64", "-o", str(output), str(args.binary), "diagnose", str(path), mode, case, cache], check=True, stdout=subprocess.DEVNULL)


def lifecycle(args):
    for dataset in args.datasets.split(","):
        for variant in args.variants.split(","):
            target = args.work / dataset / f"lifecycle-{variant}"
            for result in run(args.binary, "lifecycle", args.work / dataset / f"{variant}.db", target):
                result.update(dataset=dataset, variant=variant)
                filename=f"expire{result['fraction']}.db" if "fraction" in result else "alternate-series.db"
                result["inspect"]["physical"] = physical(target / filename)
                append(args.output / "lifecycle.jsonl", result)
            print(json.dumps({"phase": "lifecycle", "dataset": dataset, "variant": variant}), flush=True)


def summarize(output):
    grouped = defaultdict(list)
    for line in (output / "timings.jsonl").read_text().splitlines():
        row = json.loads(line)
        grouped[row["dataset"], row["variant"], row["mode"], row["case"], row["cache"]].append(row)
    rows = []
    for key, values in grouped.items():
        times = [v["ns_per_op"] for v in values]
        if len({v["digest"] for v in values}) != 1:
            raise RuntimeError("logical query digest differs between passes")
        rows.append(dict(zip(["dataset", "variant", "mode", "case", "cache"], key), passes_ns=times, median_ns=statistics.median(times), min_ns=min(times), max_ns=max(times), iterations=values[0]["iterations"], digest=values[0]["digest"]))
    for row in rows:
        base = next(r for r in rows if r["dataset"] == row["dataset"] and r["variant"] == "baseline" and r["case"] == row["case"] and r["cache"] == row["cache"])
        if row["digest"] != base["digest"]:
            raise RuntimeError("logical query digest differs from baseline")
        row["baseline_over_candidate"] = base["median_ns"] / row["median_ns"]
    write(output / "summary.json", rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=["density", "verify", "pilot", "performance", "diagnostics", "lifecycle", "summary"])
    parser.add_argument("--binary", type=Path, default=Path("/work/metrics-payload-target/release/tinystore-metrics-payload-bench"))
    parser.add_argument("--inputs", type=Path, default=Path("/work/metrics-storage-input"))
    parser.add_argument("--work", type=Path, default=Path("/work/metrics-payload"))
    parser.add_argument("--output", type=Path, default=ROOT.parent / "reports" / "data" / "metrics-payload-2026-10-09")
    parser.add_argument("--datasets", default="tsbs,alibaba,regular,irregular,nonsparse,edge")
    parser.add_argument("--variants", default=",".join(VARIANTS))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    if args.phase == "summary":
        summarize(args.output)
    else:
        globals()[args.phase](args)


if __name__ == "__main__":
    main()
