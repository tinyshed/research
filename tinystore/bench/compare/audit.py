"""Audit a round directory and print per-stage repeats and medians.

    python audit.py reports/data/<round>/<machine>

Failed operations are not throughput. Queue snapshots are reported separately
because a completed foreground request need not have drained its background job.
"""

import json
import pathlib
import statistics
import sys


def passes(values):
    return "; ".join(f"{value:,.0f}" for value in values)


def stage_table(round_):
    invalid = False
    grouped = {}
    for run in sorted(round_["runs"], key=lambda run: run["repeat"]):
        for stage in run["stages"]:
            key = (run["contender"], stage["name"], stage["goroutines"])
            grouped.setdefault(key, []).append(stage)
            app = stage.get("app", {})
            if app.get("logs_dropped") or app.get("jobs_waiting"):
                print(f'{key}, repeat {run["repeat"]}: {app}')

    print("\n| Contender | Stage | Workers | Passes/s | Median/s |")
    print("|---|---|---:|---:|---:|")
    for (contender, name, workers), stages in grouped.items():
        errors = sum(stage.get("errors", 0) for stage in stages)
        if errors:
            invalid = True
            print(f"| {contender} | {name} | {workers} | INVALID: {errors} errors | - |")
            continue
        rates = [stage["per_second"] for stage in stages]
        print(f"| {contender} | {name} | {workers} | {passes(rates)} | {statistics.median(rates):,.0f} |")
    return invalid


def footprint_table(round_):
    print("\n| Contender | Composite RAM MiB | Final disk MiB |")
    print("|---|---:|---:|")
    for contender in dict.fromkeys(run["contender"] for run in round_["runs"]):
        runs = [run for run in round_["runs"] if run["contender"] == contender]
        memory = [run["peak_rss_bytes"] + max(run.get("service_peak_rss_bytes", 0),
                                             run.get("service_pss_bytes", 0)) for run in runs]
        disk = [run["disk_bytes"] for run in runs]
        ram_mib, disk_mib = statistics.median(memory) / 2**20, statistics.median(disk) / 2**20
        print(f"| {contender} | {ram_mib:.1f} | {disk_mib:.2f} |")


def array_summary(results):
    invalid = False
    for result in results:
        if "cycles" not in result:
            print(f'{result["program"]}: {result["bytes"]:,} bytes, '
                  f'{result["added_bytes"]:,} added, cgo={result.get("cgo", False)}')
            continue
        lost = sum(cycle["lost"] for cycle in result["cycles"])
        wrong = sum(cycle["wrong"] for cycle in result["cycles"])
        invalid |= bool(lost or wrong)
        print(f'{result["contender"]}: {len(result["cycles"])} crashes, {lost} lost, {wrong} wrong')
    return invalid


def summarize(directory):
    invalid = False
    for path in sorted(directory.glob("*.json")):
        round_ = json.loads(path.read_text())
        print(f"\n## {path.stem}\n")
        if isinstance(round_, list):
            invalid |= array_summary(round_)
            continue
        if round_.get("failed"):
            invalid = True
            print("Failed: " + "; ".join(round_["failed"]))
        invalid |= stage_table(round_)
        footprint_table(round_)
    return invalid


if __name__ == "__main__":
    sys.exit(summarize(pathlib.Path(sys.argv[1])))
