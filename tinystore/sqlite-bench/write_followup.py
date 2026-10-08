#!/usr/bin/env python3
"""Follow up the noisy FULL single-row commit with balanced longer samples."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import statistics
import run


def main():
    output = run.ROOT.parent / "reports/data/sqlite-native-2026-10-07"
    baseline = json.loads((output / "environment.json").read_text())
    assert run.source_hashes() == baseline["source_sha256"]
    env = os.environ.copy()
    env.update(GOMAXPROCS="1", CGO_ENABLED="0", GOWORK="off")
    env.pop("GOGC", None)
    cpu = min(os.sched_getaffinity(0))
    binaries = {"go": run.ROOT / "bin/go-sqlite-bench", "rust": run.ROOT / "rust/target/release/tinystore-sqlite-native-bench"}
    assert {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in binaries.items()} == baseline["binary_sha256"]
    metadata = {"started_utc": dt.datetime.now(dt.timezone.utc).isoformat(), "reason": "single-row FULL commits had a 9ms/op outlier in the initial run", "passes": 6, "minimum_final_seconds": 0.25, "cpu_affinity": cpu, "source_sha256": run.source_hashes(), "followup_source_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), "binary_sha256": baseline["binary_sha256"]}
    timings = []
    with (output / "write-followup.jsonl").open("w") as raw:
        for p in range(6):
            order = run.IMPLEMENTATIONS[p % 3:] + run.IMPLEMENTATIONS[:p % 3]
            print(f"Single-row FULL commit pass {p+1}/6", flush=True)
            for implementation in order:
                for page in [1024, 4096]:
                    row = run.run_one(implementation, run.ROOT / "stores" / f"fixture-{page}.db", "update1", "bench", env, cpu, ["--seconds", "0.25"])
                    row.update(page_size=page, pass_id=p+1, utc=dt.datetime.now(dt.timezone.utc).isoformat())
                    timings.append(row)
                    raw.write(json.dumps(row) + "\n"); raw.flush()
    summary = []
    for page in [1024, 4096]:
        samples = {name: [r["ns_per_op"] for r in timings if r["implementation"] == name and r["page_size"] == page] for name in run.IMPLEMENTATIONS}
        medians = {name: statistics.median(values) for name, values in samples.items()}
        summary.append({"page_size": page, "case": "update1", "median_ns": medians, "passes_ns": samples, "tinystore_over_native": medians["go_tinystore"] / medians["rust_native"], "raw_go_over_native": medians["go_raw"] / medians["rust_native"]})
    assert run.source_hashes() == baseline["source_sha256"]
    metadata["finished_utc"] = dt.datetime.now(dt.timezone.utc).isoformat()
    (output / "write-followup-environment.json").write_text(json.dumps(metadata, indent=2) + "\n")
    (output / "write-followup-summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print("Complete: 36 additional FULL single-row commit measurements", flush=True)


if __name__ == "__main__":
    main()
