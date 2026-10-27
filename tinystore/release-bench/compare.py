#!/usr/bin/env python3
"""Idle rounds side by side, as a report's tables are drawn from them: each
figure's median over its passes and the passes' range, by contender.

  python3 compare.py 0.1.0=<results>/<name> 0.4.0=<results>/<other> more=<file>.jsonl > compare.md

A figure is a phase `idle` median from run.py's lines: `pss <group>` and
`cpu <group>` for each group, `not-app` every group but the program's, the
page cache the case brought in, the program's `open ms`, each service's
`ready` seconds, and for TinyStore's server its ready plus the program's
open, the card's "ready to serve".
"""
import json
import statistics
import sys
from pathlib import Path

MIB = 1 << 20


def runs(directory):
    """A results directory's idle.jsonl, or a .jsonl file named itself."""
    path = Path(directory)
    out = {}
    for raw in (path if path.suffix == ".jsonl" else path / "idle.jsonl").read_text().splitlines():
        row = json.loads(raw)
        if row.get("error") or row.get("exit") not in (0, None):
            print(f"failed: {directory} {row['contender']} pass {row['pass']}", file=sys.stderr)
            continue
        out.setdefault(row["contender"], []).append(row)
    return out


def figures(rows):
    """figure -> its value in each pass"""
    out = {}

    def add(name, value):
        out.setdefault(name, []).append(value)

    for row in rows:
        idle = row["phases"].get("idle", {})
        groups = idle.get("groups", {})
        for group, values in groups.items():
            add(f"pss {group}", values["pss_median"] / MIB)
            add(f"cpu {group}", values["cpu_seconds"])
        add("pss not-app", sum(v["pss_median"] for g, v in groups.items() if g != "app") / MIB)
        add("pss total", idle["pss_total_median"] / MIB)
        add("cpu not-app", sum(v["cpu_seconds"] for g, v in groups.items() if g != "app"))
        add("page cache", (idle["cgroup_file_median"] - row["cgroup_before"].get("file", 0)) / MIB)
        opened = [result["openMs"] for result in row["results"] if "openMs" in result]
        if opened:
            add("open ms", opened[0])
        for name, seconds in row.get("ready", {}).items():
            add(f"ready {name}", seconds)
        if row.get("ready"):
            add("ready max", max(row["ready"].values()))
        if "tinystore" in row.get("ready", {}) and opened:
            add("ready+open tinystore", row["ready"]["tinystore"] + opened[0] / 1000)
    return out


def show(value):
    return f"{value:.3f}" if value < 1 else f"{value:.2f}" if value < 10 else f"{value:.1f}"


def main():
    sets = [arg.split("=", 1) for arg in sys.argv[1:]]
    loaded = {label: runs(directory) for label, directory in sets}
    contenders = []
    for data in loaded.values():
        contenders += [name for name in data if name not in contenders]
    for contender in contenders:
        per = {label: figures(data.get(contender, [])) for label, data in loaded.items()}
        names = []
        for found in per.values():
            names += [name for name in found if name not in names]
        print(f"\n## {contender}\n")
        print("| figure | " + " | ".join(f"{label}: median [min-max] passes" for label, _ in sets) + " |")
        print("|---|" + "---|" * len(sets))
        for name in names:
            cells = []
            for label, _ in sets:
                values = per[label].get(name)
                cells.append(f"{show(statistics.median(values))} [{show(min(values))}-{show(max(values))}] "
                             f"{len(values)}" if values else "")
            print(f"| {name} | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    main()
