#!/usr/bin/env python3
"""Resume memory collection after timings using a locally installed GNU time."""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess

from run import ROOT, source_hashes


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--time", default="/usr/bin/time")
    parser.add_argument("--output", type=Path, default=ROOT.parent / "reports/data/rust-kernels-2026-10-07")
    args = parser.parse_args(); output = args.output
    metadata = json.loads((output / "environment.json").read_text())
    assert source_hashes() == metadata["source_sha256"], "measured sources changed"
    collected = [json.loads(line) for line in (output / "timings.jsonl").read_text().splitlines()]
    verified = json.loads((output / "verification.json").read_text())
    expected = {(r["case"], lang, p) for r in verified for lang in ["go", "rust"] for p in range(1, metadata["passes"] + 1)}
    actual = {(r["case"], r["language"], r["pass"]) for r in collected}
    assert actual == expected and len(collected) == len(expected), "incomplete or duplicate timing passes"
    cpu = metadata["cpu_affinity"]
    env = os.environ.copy(); env.update({"GOMAXPROCS": "1", "GOWORK": "off", "CGO_ENABLED": "0"}); env.pop("GOGC", None)
    binaries = {"go": ROOT / "bin/go-spike", "rust": ROOT / "bin/rust-spike"}
    memory_cases = ["metrics/change/many/decode", "records/width_17/decode", "ingest/ordered/4096", "ingest/shuffled/4096"]
    with (output / "memory.jsonl").open("w") as raw:
        for name in memory_cases:
            size = next(r["output_bytes"] for r in verified if r["case"] == name)
            retained = max(1, (64 << 20) // max(1, size))
            for p in range(1, 4):
                for lang in (["go", "rust"] if p % 2 else ["rust", "go"]):
                    print(f"Retain 64 MiB: {name}, {lang}, pass {p}", flush=True)
                    args_run = [args.time, "-f", "%M", "taskset", "-c", str(cpu), str(binaries[lang]), "--mode", "memory", "--case", name, "--iterations", str(retained)]
                    result = subprocess.run(args_run, env=env, check=True, text=True, capture_output=True)
                    row = json.loads(result.stdout); row.update({"pass": p, "peak_rss_kib": int(result.stderr.strip().splitlines()[-1])})
                    raw.write(json.dumps(row) + "\n"); raw.flush()
    summary = []
    for name in sorted(r["case"] for r in verified):
        samples = {lang: [r["ns_per_op"] for r in collected if r["language"] == lang and r["case"] == name] for lang in binaries}
        medians = {lang: statistics.median(v) for lang, v in samples.items()}
        summary.append({"case": name, "go_ns": medians["go"], "rust_ns": medians["rust"], "speedup_go_over_rust": medians["go"] / medians["rust"], "passes_ns": samples})
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    metadata["finished_utc"] = dt.datetime.now(dt.timezone.utc).isoformat()
    metadata["completion_driver_sha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    metadata["completion_note"] = "All timing and allocation passes completed unchanged; memory collection resumed after installing GNU time locally."
    metadata["binary_sha256"] = {lang: hashlib.sha256(binary.read_bytes()).hexdigest() for lang, binary in binaries.items()}
    assert source_hashes() == metadata["source_sha256"], "measured sources changed"
    (output / "environment.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(f"Complete: {len(summary)} cases, {metadata['passes']} paired timing passes, 24 memory processes", flush=True)


if __name__ == "__main__": main()
