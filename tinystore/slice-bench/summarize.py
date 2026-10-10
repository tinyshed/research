#!/usr/bin/env python3
"""Prints a round's runs as the report's tables: a row a case, a cell a program,
its passes in the order they ran."""
import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path

DATA = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parent.parent / "reports/data/rust-slice-2026-10-10"


def load(*names):
    """The runs of the files named, by program, case and callers, in the order of their passes."""
    by = defaultdict(list)
    for name in names:
        path = DATA / f"{name}.jsonl"
        if not path.exists():
            continue
        rows = [json.loads(line) for line in path.read_text().splitlines() if line.strip()]
        for row in sorted(rows, key=lambda row: row["pass"]):
            by[(row["program"], row["case"], row["callers"])].append(row)
    return by


def rate(value):
    return f"{value:,.0f}" if value >= 100 else f"{value:,.1f}"


def median(by, program, case, callers, field="per_second"):
    found = by.get((program, case, callers))
    return statistics.median(row[field] for row in found) if found else None


def table(by, title, programs, cases, ratio=None):
    """Calls a second, each pass; the last column is the medians' ratio of `ratio`, a pair of programs."""
    ratio = ratio or (programs[-1], programs[0])
    print(f"\n### {title}\n")
    print("| Case | In flight | " + " | ".join(programs) + f" | {ratio[0]} to {ratio[1]} |")
    print("|---|---|" + "---|" * (len(programs) + 1))
    for case, callers in cases:
        cells = []
        for program in programs:
            found = by.get((program, case, callers))
            cells.append("—" if not found else "; ".join(rate(row["per_second"]) for row in found))
        over, under = median(by, ratio[0], case, callers), median(by, ratio[1], case, callers)
        print(f"| {case} | {callers} | " + " | ".join(cells) + " | " + (f"{over / under:.2f}×" if over and under else "—") + " |")


def latencies(by, title, programs, cases):
    print(f"\n### {title}\n")
    print("| Case | In flight | " + " | ".join(f"{program} p50; p99, µs" for program in programs) + " |")
    print("|---|---|" + "---|" * len(programs))
    for case, callers in cases:
        cells = []
        for program in programs:
            p50, p99 = median(by, program, case, callers, "p50_ns"), median(by, program, case, callers, "p99_ns")
            cells.append("—" if p50 is None else f"{p50 / 1000:,.1f}; {p99 / 1000:,.1f}")
        print(f"| {case} | {callers} | " + " | ".join(cells) + " |")


def memory(by, title, programs, cases):
    print(f"\n### {title}\n")
    print("| Case | In flight | " + " | ".join(f"{program}, MiB" for program in programs) + " |")
    print("|---|---|" + "---|" * len(programs))
    for case, callers in cases:
        cells = []
        for program in programs:
            found = by.get((program, case, callers))
            cells.append("—" if not found else "; ".join(f"{row['process']['max_rss_kib'] / 1024:.0f}" for row in found))
        print(f"| {case} | {callers} | " + " | ".join(cells) + " |")


native = [("kv-get", 1), ("kv-get", 64), ("kv-set", 1), ("kv-set", 64), ("sql-point", 1), ("sql-point", 64),
          ("sql-insert", 1), ("sql-insert", 64), ("sql-rows", 1), ("jobs-add", 1), ("jobs-add", 64), ("jobs-drain", 8)]
bun = [("kv-get", 1), ("kv-get", 64), ("kv-set", 64), ("sql-point", 64), ("sql-rows", 1), ("jobs-add", 64)]
buns = ["bun-go-sidecar", "bun-rust-sidecar", "bun-rust-embedded"]

runs = load("runs")
table(runs, "In process, calls a second, each pass", ["go", "rust"], native)
latencies(runs, "In process, latency", ["go", "rust"], [case for case in native if case[0] != "jobs-drain"])
memory(runs, "In process, the program's peak resident memory", ["go", "rust"], native)
table(runs, "Through Bun, calls a second, each pass", buns, bun)
latencies(runs, "Through Bun, latency", buns, bun)
memory(runs, "Through Bun, the Bun process's peak resident memory", buns, bun)

scaling = load("runs", "scaling")
reads = [(case, callers) for case in ("kv-get", "sql-point") for callers in (1, 4, 8, 16, 64)]
table(scaling, "Reads by the callers at once, calls a second, each pass", ["go", "rust"], reads)

later = load("next-native")
if later:
    cases = [case for case in native + [("kv-get", 8), ("kv-get", 16), ("sql-point", 16)] if ("rust-next",) + case in later]
    cases.sort(key=lambda case: (["kv-get", "kv-set", "sql-point", "sql-insert", "sql-rows", "jobs-add", "jobs-drain"].index(case[0]), case[1]))
    table(later, "The later commit in process, calls a second, each pass", ["rust", "rust-next"], cases)
later = load("next-bun")
if later:
    programs = ["bun-rust-sidecar", "bun-rust-sidecar-next", "bun-rust-embedded", "bun-rust-embedded-next"]
    cases = [case for case in bun if (programs[1],) + case in later]
    table(later, "The later commit through Bun, calls a second, each pass", programs, cases,
          ratio=("bun-rust-sidecar-next", "bun-rust-sidecar"))
