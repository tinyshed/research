#!/usr/bin/env python3
"""Build once, compare owned outputs, then run isolated interleaved passes."""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import shutil

ROOT = Path(__file__).resolve().parent


def command(args, **kwargs):
    return subprocess.run(args, check=True, text=True, capture_output=True, **kwargs).stdout


def build():
    (ROOT / "bin").mkdir(exist_ok=True)
    command(["go", "build", "-trimpath", "-o", str(ROOT / "bin/go-spike"), "."], cwd=ROOT / "go")
    command(["cargo", "build", "--release", "--locked"], cwd=ROOT / "rust")
    shutil.copy2(ROOT / "rust/target/release/tinystore-rust-spike", ROOT / "bin/rust-spike")
    command(["cargo", "build", "--release", "--locked", "--features", "counting-allocator"], cwd=ROOT / "rust")
    shutil.copy2(ROOT / "rust/target/release/tinystore-rust-spike", ROOT / "bin/rust-alloc-spike")
    command(["go", "test", "./..."], cwd=ROOT / "go")
    command(["cargo", "test", "--release", "--locked"], cwd=ROOT / "rust")


def source_hashes():
    paths = [ROOT / "run.py"]
    paths.extend(sorted((ROOT / "go").glob("*.go")))
    paths.extend(sorted((ROOT / "rust/src").glob("*.rs")))
    paths.extend([ROOT / "go/go.mod", ROOT / "rust/Cargo.toml", ROOT / "rust/Cargo.lock"])
    return {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}


def rows(binary, mode, extra=(), env=None, cpu=None):
    args = [str(binary), "--mode", mode, *extra]
    if cpu is not None:
        args = ["taskset", "-c", str(cpu), *args]
    if mode == "memory":
        measured = subprocess.run(["/usr/bin/time", "-f", "%M", *args], env=env, check=True, text=True, capture_output=True)
        result = [json.loads(line) for line in measured.stdout.splitlines()]
        peak = int(measured.stderr.strip().splitlines()[-1])
        for row in result: row["peak_rss_kib"] = peak
        return result
    return [json.loads(line) for line in command(args, env=env).splitlines()]


def verify(binaries, output):
    results = {lang: {r["case"]: r for r in rows(binary, "verify")} for lang, binary in binaries.items()}
    if results["go"].keys() != results["rust"].keys():
        raise AssertionError(f"different cases: {results['go'].keys() ^ results['rust'].keys()}")
    verified = []
    for name, baseline in results["go"].items():
        if baseline != results["rust"][name]:
            raise AssertionError(f"output or fixture metadata differs: {name}")
        for token in ("/chunked/", "/decode_word", "/typed_sort"):
            original = name.replace(token, "/baseline/" if token == "/chunked/" else "/decode" if token == "/decode_word" else "")
            if original != name and original in results["go"]:
                assert baseline["hex"] == results["go"][original]["hex"], name
        assert int(baseline["input_fingerprint"], 16) != 0, name
        verified.append({"case": name, "units": baseline["units"], "input_bytes": baseline["input_bytes"], "input_fingerprint": baseline["input_fingerprint"], "output_bytes": len(baseline["hex"]) // 2, "output_sha256": hashlib.sha256(bytes.fromhex(baseline["hex"])).hexdigest()})
    (output / "verification.json").write_text(json.dumps(verified, indent=2) + "\n")
    print(f"Verified identical output bytes for {len(verified)} cases", flush=True)
    return verified


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT.parent / "reports/data/rust-kernels-2026-10-07")
    parser.add_argument("--passes", type=int, default=5)
    parser.add_argument("--seconds", type=float, default=0.15)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=True)
    if not args.skip_build: build()
    binaries = {"go": ROOT / "bin/go-spike", "rust": ROOT / "bin/rust-spike"}
    verified = verify(binaries, output)
    if args.verify_only: return
    cpu = min(os.sched_getaffinity(0))
    env = os.environ.copy(); env.update({"GOMAXPROCS": "1", "GOWORK": "off", "CGO_ENABLED": "0"}); env.pop("GOGC", None)
    metadata = {
        "started_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "tinystore_commit": command(["git", "rev-parse", "HEAD"], cwd=ROOT.parent / "source").strip(),
        "go": command(["go", "version"]).strip(), "rustc": command(["rustc", "--version"]).strip(),
        "cargo": command(["cargo", "--version"]).strip(), "platform": platform.platform(),
        "cpu": command(["lscpu"]).strip(), "cpu_affinity": cpu, "cpu_quota": Path("/sys/fs/cgroup/cpu.max").read_text().strip(),
        "memory_limit": Path("/sys/fs/cgroup/memory.max").read_text().strip(),
        "passes": args.passes, "minimum_final_seconds": args.seconds, "gomaxprocs": 1, "gogc": "default (100)",
        "harness_status": "uncommitted local prototype; content hashes pin the measured files", "source_sha256": source_hashes(),
    }
    (output / "environment.json").write_text(json.dumps(metadata, indent=2) + "\n")
    collected = []
    with (output / "timings.jsonl").open("w") as raw:
        for pass_id in range(1, args.passes + 1):
            # Reverse every other pair to balance which runtime runs first.
            for lang in (["go", "rust"] if pass_id % 2 else ["rust", "go"]):
                print(f"Timing pass {pass_id}/{args.passes}: {lang}", flush=True)
                for row in rows(binaries[lang], "bench", ["--seconds", str(args.seconds)], env, cpu):
                    row.update({"pass": pass_id, "utc": dt.datetime.now(dt.timezone.utc).isoformat()})
                    collected.append(row); raw.write(json.dumps(row) + "\n"); raw.flush()
    with (output / "allocations.jsonl").open("w") as raw:
        for lang, binary in binaries.items():
            print(f"Allocation counters: {lang}", flush=True)
            if lang == "rust": binary = ROOT / "bin/rust-alloc-spike"
            for row in rows(binary, "alloc", ["--iterations", "256"], env, cpu): raw.write(json.dumps(row) + "\n")
    memory_cases = ["metrics/change/many/decode", "records/width_17/decode", "ingest/ordered/4096", "ingest/shuffled/4096"]
    available = {r["case"] for r in verified}
    with (output / "memory.jsonl").open("w") as raw:
        for name in memory_cases:
            if name not in available: raise AssertionError(f"missing memory case: {name}")
            size = next(r["output_bytes"] for r in verified if r["case"] == name)
            retained = max(1, (64 << 20) // max(1, size))
            for pass_id in range(1, 4):
                for lang in (["go", "rust"] if pass_id % 2 else ["rust", "go"]):
                    print(f"Retain 64 MiB: {name}, {lang}, pass {pass_id}", flush=True)
                    for row in rows(binaries[lang], "memory", ["--case", name, "--iterations", str(retained)], env, cpu):
                        row["pass"] = pass_id; raw.write(json.dumps(row) + "\n"); raw.flush()
    summary = []
    for name in sorted(available):
        samples = {lang: [r["ns_per_op"] for r in collected if r["language"] == lang and r["case"] == name] for lang in binaries}
        medians = {lang: statistics.median(v) for lang, v in samples.items()}
        summary.append({"case": name, "go_ns": medians["go"], "rust_ns": medians["rust"], "speedup_go_over_rust": medians["go"] / medians["rust"], "passes_ns": samples})
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    assert source_hashes() == metadata["source_sha256"], "harness changed while benchmarking"
    metadata["finished_utc"] = dt.datetime.now(dt.timezone.utc).isoformat()
    (output / "environment.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(f"Complete: {len(summary)} cases, {args.passes} paired timing passes", flush=True)


if __name__ == "__main__": main()
