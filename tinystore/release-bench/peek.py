#!/usr/bin/env python3
"""A round's lines at a glance while it runs: each case's results, errors,
and memory by group in each phase, in MiB.

  python3 peek.py results/<name>/<round>.jsonl
"""
import json
import sys

MIB = 1 << 20
for raw in open(sys.argv[1]):
    line = json.loads(raw)
    print(f"== {line['round']} {line['contender']} pass {line['pass']} exit {line.get('exit')}"
          f" ready {({k: round(v, 3) for k, v in line.get('ready', {}).items()})}")
    if "error" in line:
        print("  ERROR", line["error"][-600:])
    for result in line.get("results", []):
        print("  ", {k: (round(v, 1) if isinstance(v, float) else v) for k, v in result.items()})
    for phase, summary in line.get("phases", {}).items():
        groups = " ".join(f"{g}={v['pss_median'] / MIB:.1f}/{v['pss_max'] / MIB:.1f}"
                          f"/{v['cpu_seconds']:.1f}s" for g, v in summary["groups"].items())
        print(f"   {phase:<22} total {summary['pss_total_median'] / MIB:7.1f}/{summary['pss_total_max'] / MIB:7.1f}"
              f" cg anon {summary.get('cgroup_anon_median', 0) / MIB:6.1f} file "
              f"{summary.get('cgroup_file_median', 0) / MIB:6.1f}  {groups}")
    disk = {k: [d and round(d['allocated'] / MIB, 2) for d in v] for k, v in line.get("disk", {}).items()}
    print("   disk MiB", disk)
