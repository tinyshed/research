#!/usr/bin/env python3
"""Compare TinyStore's SQL layer, direct translated SQLite, and native SQLite."""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
CASES = ["point", "point_txn", "range240", "aggregate240", "update1", "update64"]
IMPLEMENTATIONS = ["go_tinystore", "go_raw", "rust_native"]


def call(args, **kwargs):
    return subprocess.run(args, check=True, text=True, capture_output=True, **kwargs).stdout


def source_hashes():
    paths = [ROOT / "run.py", ROOT / "build_native.py"]
    paths += sorted((ROOT / "go").glob("*.go"))
    paths += sorted((ROOT / "rust/src").glob("*.rs"))
    paths += [ROOT / "go/go.mod", ROOT / "go/go.sum", ROOT / "rust/Cargo.toml", ROOT / "rust/Cargo.lock", ROOT / "rust/build.rs"]
    return {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}


def build():
    (ROOT / "bin").mkdir(exist_ok=True)
    call(["go", "build", "-trimpath", "-o", str(ROOT / "bin/go-sqlite-bench"), "."], cwd=ROOT / "go")
    call(["python3", str(ROOT / "build_native.py"), "--cargo"])


def run_one(implementation, fixture, case, mode, env, cpu, extra=()):
    with tempfile.TemporaryDirectory(prefix="run-", dir=ROOT / "stores") as directory:
        db = Path(directory) / "bench.db"
        shutil.copyfile(fixture, db)
        if implementation.startswith("go_"):
            args = [str(ROOT / "bin/go-sqlite-bench"), "--implementation", implementation]
        else:
            args = [str(ROOT / "rust/target/release/tinystore-sqlite-native-bench")]
        args += ["--db", str(db), "--case", case, "--mode", mode, *extra]
        return json.loads(call(["taskset", "-c", str(cpu), *args], env=env))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--verify-only", action="store_true")
    parser.add_argument("--passes", type=int, default=3)
    parser.add_argument("--seconds", type=float, default=0.15)
    args = parser.parse_args()
    output = ROOT.parent / "reports/data/sqlite-native-2026-10-07"
    output.mkdir(parents=True, exist_ok=True); (ROOT / "stores").mkdir(exist_ok=True)
    if not args.skip_build: build()
    env = os.environ.copy(); env.update(GOMAXPROCS="1", CGO_ENABLED="0", GOWORK="off"); env.pop("GOGC", None)
    cpu = min(os.sched_getaffinity(0))
    fixtures = {}
    for page in [1024, 4096]:
        db = ROOT / "stores" / f"fixture-{page}.db"
        if not db.exists():
            print(f"Prepare {page}-byte page fixture", flush=True)
            call([str(ROOT / "bin/go-sqlite-bench"), "--mode", "fixture", "--db", str(db), "--page-size", str(page)], env=env)
        fixtures[page] = db
    inspections = []
    for page, fixture in fixtures.items():
        for implementation in IMPLEMENTATIONS:
            print(f"Inspect {implementation}, page {page}", flush=True)
            row = run_one(implementation, fixture, "point", "inspect", env, cpu)
            row["page_size"] = page; inspections.append(row)
            for role in ["reader", "writer"]:
                c = row[role]
                expected = {"version": "3.53.4", "journal_mode": "wal", "page_size": page, "foreign_keys": 1,
                            "busy_timeout": 5000, "synchronous": 2, "fullfsync": 1, "checkpoint_fullfsync": 1,
                            "cache_size": -1024, "query_only": int(role == "reader"), "trusted_schema": 0}
                for key, value in expected.items():
                    assert c[key] == value, (implementation, page, role, key, c.get(key), value)
    (output / "inspect.json").write_text(json.dumps(inspections, indent=2) + "\n")
    verification = []
    for page, fixture in fixtures.items():
        for case in CASES:
            results = [run_one(implementation, fixture, case, "verify", env, cpu, ["--iterations", "16"]) for implementation in IMPLEMENTATIONS]
            normalized = [{k: v for k, v in row.items() if k != "implementation"} for row in results]
            assert all(row == normalized[0] for row in normalized), (page, case, results)
            verification.append({"page_size": page, **normalized[0]})
    (output / "verification.json").write_text(json.dumps(verification, indent=2) + "\n")
    print("Verified 12 scenarios: same returned bytes and complete final database digest across all 3 stacks", flush=True)
    if args.verify_only: return
    binaries = {"go": ROOT / "bin/go-sqlite-bench", "rust": ROOT / "rust/target/release/tinystore-sqlite-native-bench"}
    metadata = {
        "started_utc": dt.datetime.now(dt.timezone.utc).isoformat(), "source_sha256": source_hashes(),
        "tinystore_commit": call(["git", "rev-parse", "HEAD"], cwd=ROOT.parent / "source").strip(),
        "go": call(["go", "version"]).strip(), "rustc": call(["rustc", "--version"]).strip(),
        "platform": platform.platform(), "cpu": call(["lscpu"]).strip(), "cpu_affinity": cpu,
        "cpu_quota": Path("/sys/fs/cgroup/cpu.max").read_text().strip(), "memory_limit": Path("/sys/fs/cgroup/memory.max").read_text().strip(),
        "passes": args.passes, "minimum_final_seconds": args.seconds, "gomaxprocs": 1, "gogc": "default100",
        "filesystem": call(["stat", "-f", "-c", "%T", str(ROOT / "stores")]).strip(),
        "native_build": json.loads((ROOT / "generated/native-build.json").read_text()),
        "fixtures": {str(page): {"bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for page, path in fixtures.items()},
        "binary_sha256": {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in binaries.items()},
        "harness_status": "uncommitted local prototype; source hashes recorded", "data_rows": 32768, "body_bytes": 128,
    }
    (output / "environment.json").write_text(json.dumps(metadata, indent=2) + "\n")
    timings = []
    with (output / "timings.jsonl").open("w") as raw:
        for p in range(args.passes):
            # Latin rotation balances first/middle/last position over three passes.
            order = IMPLEMENTATIONS[p % 3:] + IMPLEMENTATIONS[:p % 3]
            for implementation in order:
                print(f"Timing pass {p+1}/{args.passes}: {implementation}", flush=True)
                for page, fixture in fixtures.items():
                    for case in CASES:
                        row = run_one(implementation, fixture, case, "bench", env, cpu, ["--seconds", str(args.seconds)])
                        row.update(page_size=page, pass_id=p+1, utc=dt.datetime.now(dt.timezone.utc).isoformat())
                        timings.append(row); raw.write(json.dumps(row) + "\n"); raw.flush()
    summary = []
    for page in fixtures:
        for case in CASES:
            samples = {implementation: [r["ns_per_op"] for r in timings if r["implementation"] == implementation and r["page_size"] == page and r["case"] == case] for implementation in IMPLEMENTATIONS}
            median = {name: statistics.median(values) for name, values in samples.items()}
            summary.append({"page_size": page, "case": case, "median_ns": median, "passes_ns": samples,
                            "tinystore_over_native": median["go_tinystore"] / median["rust_native"],
                            "raw_go_over_native": median["go_raw"] / median["rust_native"],
                            "tinystore_over_raw_go": median["go_tinystore"] / median["go_raw"]})
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    assert source_hashes() == metadata["source_sha256"], "source changed during benchmark"
    metadata["finished_utc"] = dt.datetime.now(dt.timezone.utc).isoformat()
    (output / "environment.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(f"Complete: {len(timings)} measurements, SQLite3.53.4, 3 stacks, 2 page sizes, 6 workloads", flush=True)


if __name__ == "__main__": main()
