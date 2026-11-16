"""dir-scan's tables, from the raw lines of windows.jsonl and linux.jsonl.

A cell is the median of its passes with the least and the most, in seconds;
memory is the peak resident set of the process, the median of its passes.
"""

import json
import statistics
import sys
from pathlib import Path

FIXTURES = ["lib10k", "lib100k", "flat100k", "lib1m"]
STEPS = [
    ("walk", None, "1. bare walk"),
    ("first", "table", "2. first scan"),
    ("unchanged", "table", "3. scan, nothing changed"),
    ("unchanged", "memory", "3m. same, sorted in memory"),
    ("changed", "table", "4. scan, 1 % changed"),
]


def load(path):
    lines = []
    if path.exists():
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line.startswith("{"):
                lines.append(json.loads(line))
    return lines


def cell(values):
    if not values:
        return "-"
    m = statistics.median(values)
    return f"{m:.3f} [{min(values):.3f}, {max(values):.3f}] n={len(values)}"


def mib(values):
    return f"{statistics.median(values) / 2**20:.1f}" if values else "-"


def table(lines, cache):
    passes = {}
    for line in lines:
        if line["op"] not in ("walk", "scan"):
            continue
        fixture, at, step = line["label"].split("/")
        if at != cache:
            continue
        passes.setdefault((fixture, step, line.get("sort")), []).append(line)
    if not passes:
        return None
    out = ["| step | " + " | ".join(FIXTURES) + " |", "|---|" + "---|" * len(FIXTURES)]
    for step, sort, title in STEPS:
        row = []
        for fixture in FIXTURES:
            got = passes.get((fixture, step, sort), [])
            row.append(cell([p["seconds"] for p in got]))
        out.append(f"| {title} | " + " | ".join(row) + " |")
    for step, sort, title in STEPS:
        row = []
        for fixture in FIXTURES:
            got = passes.get((fixture, step, sort), [])
            row.append(cell([p["cpu_seconds"] for p in got]))
        out.append(f"| CPU s, {title} | " + " | ".join(row) + " |")
    for step, sort, title in STEPS:
        row = []
        for fixture in FIXTURES:
            got = passes.get((fixture, step, sort), [])
            row.append(mib([p["peak_rss"] for p in got]))
        out.append(f"| peak RSS MiB, {title} | " + " | ".join(row) + " |")
    counts = []
    for fixture in FIXTURES:
        got = passes.get((fixture, "changed", "table"), [])
        if got:
            counts.append(f"{fixture}: changed {[p['changed'] for p in got]}, commits {[p['commits'] for p in got]}")
        got = passes.get((fixture, "first", "table"), [])
        if got:
            counts.append(f"{fixture}: files {got[0]['files']}, folders {got[0]['folders']}, first commits {got[0]['commits']}")
    return "\n".join(out) + "\n\n" + "\n".join(f"- {c}" for c in counts)


def sizes(lines):
    out = []
    for line in lines:
        if line["op"] != "size":
            continue
        rows = line["rows"]
        out.append(
            f"- {line['label']}: {rows} rows, file {line['file_bytes']} B = {line['file_bytes'] / rows:.1f} B a file, "
            f"paths {line['path_bytes'] / rows:.1f} B a row"
        )
        for o in line["objects"]:
            per = f"{o['bytes'] / rows:.1f} B a file, payload {o['payload'] / max(o['cells'], 1):.1f} B a cell"
            out.append(
                f"  - {o['name']}: {o['pages']} pages, {o['cells']} cells, payload {o['payload']}, unused {o['unused']}, {per}"
            )
    return "\n".join(out)


def main():
    data = Path(sys.argv[1])
    for platform in ("windows", "linux"):
        lines = load(data / f"{platform}.jsonl")
        if not lines:
            continue
        info = next((l for l in lines if l["op"] == "info"), {})
        print(f"## {platform} (SQLite {info.get('sqlite')}, rusqlite {info.get('rusqlite')})\n")
        for cache in ("warm", "cold"):
            got = table(lines, cache)
            if got:
                print(f"### {cache}: seconds, median [least, most]\n\n{got}\n")
        print(f"### the list's file\n\n{sizes(lines)}\n")
        gens = [l for l in lines if l["op"] == "gen"]
        for g in gens:
            print(f"- gen {g['shape']}: {g['files']} files, {g['folders']} folders, {g['bytes']} B, {g['seconds']} s")
        print()


main()
