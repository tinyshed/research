"""Turns a results directory's rounds into the report's tables.

    python summarize.py results/<date> > results/<date>/summary.md

Every figure is the median of a contender's repeats, with the range beside it
when there were more than one. Only the standard library, so that it runs
wherever the results were copied to.
"""

import json
import pathlib
import statistics
import sys


def median(values):
    return statistics.median(values) if values else 0


def spread(values, fmt):
    shown = fmt(median(values))
    if len(values) > 1 and min(values) != max(values):
        shown += f" ({fmt(min(values))}–{fmt(max(values))})"
    return shown


def rate(v):
    if v >= 1_000_000:
        return f"{v / 1e6:.2f} M"
    if v >= 10_000:
        return f"{v / 1e3:.0f} k"
    if v >= 1_000:
        return f"{v / 1e3:.1f} k"
    return f"{v:.0f}"


def micros(v):
    if v >= 1000:
        return f"{v / 1000:.1f} ms"
    return f"{v:.0f} µs"


def mib(v):
    return f"{v / 2**20:.1f}"


def memory(run):
    return run["peak_rss_bytes"] + max(run.get("service_pss_bytes", 0), run.get("service_peak_rss_bytes", 0))


def contenders(round_):
    order, runs = [], {}
    for run in round_["runs"]:
        name = run["contender"]
        if name not in runs:
            order.append(name)
            runs[name] = []
        runs[name].append(run)
    return order, runs


def stage_keys(runs):
    keys = []
    for run_list in runs.values():
        for run in run_list:
            for s in run["stages"]:
                key = (s["name"], s["goroutines"])
                if key not in keys:
                    keys.append(key)
    return keys


def stage_values(run_list, key, field):
    out = []
    for run in run_list:
        for s in run["stages"]:
            if (s["name"], s["goroutines"]) == key and not s.get("first_error"):
                out.append(s[field])
    return out


def table(round_):
    order, runs = contenders(round_)
    lines = []
    head = ["", "peak MiB", "disk MiB", "ready s", "processes"]
    lines.append("| " + " | ".join(head) + " |")
    lines.append("|" + "---|" * len(head))
    for name in order:
        rs = runs[name]
        lines.append(
            f"| {name} | {spread([memory(r) for r in rs], mib)} | {spread([r['disk_bytes'] for r in rs], mib)} "
            f"| {spread([r['open_seconds'] for r in rs], lambda v: f'{v:.2f}')} | {rs[0].get('processes', 1)} |"
        )
    lines.append("")
    keys = stage_keys(runs)
    head = ["stage"] + order
    lines.append("| " + " | ".join(head) + " |")
    lines.append("|" + "---|" * len(head))
    for key in keys:
        row = [f"{key[0]}×{key[1]}"]
        for name in order:
            per = stage_values(runs[name], key, "per_second")
            p99 = stage_values(runs[name], key, "p99_us")
            cpu = stage_values(runs[name], key, "cpu_seconds")
            ops = stage_values(runs[name], key, "ops")
            if not per:
                row.append("—")
                continue
            cell = f"{spread(per, rate)}/s, p99 {micros(median(p99))}"
            if cpu and median(ops):
                cell += f", {median(cpu) / median(ops) * 1e6:.0f} µs CPU/op"
            row.append(cell)
        lines.append("| " + " | ".join(row) + " |")
    errors = []
    for name in order:
        for run in runs[name]:
            for s in run["stages"]:
                if s.get("errors"):
                    errors.append(f"- {name} #{run['repeat']} {s['name']}×{s['goroutines']}: "
                                  f"{s['errors']} errors, first: {s.get('first_error', '')[:200]}")
    for failed in round_.get("failed", []):
        errors.append(f"- failed: {failed.splitlines()[0][:200]}")
    if errors:
        lines.append("")
        lines.extend(errors)
    return "\n".join(lines)


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    directory = pathlib.Path(sys.argv[1])
    print(f"# {directory.name}\n")
    environment = directory / "environment.txt"
    if environment.exists():
        print("```text\n" + environment.read_text().strip() + "\n```\n")
    for path in sorted(directory.glob("*.json")):
        data = json.loads(path.read_text())
        if isinstance(data, dict) and "runs" in data:
            print(f"## {path.stem}\n")
            print(table(data) + "\n")
        else:
            print(f"## {path.stem}\n\n```json\n{json.dumps(data, indent=1)[:4000]}\n```\n")
    failures = directory / "failures.txt"
    if failures.exists():
        print("## failures\n\n```text\n" + failures.read_text().strip() + "\n```")


if __name__ == "__main__":
    main()
