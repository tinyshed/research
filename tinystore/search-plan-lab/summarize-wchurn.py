"""The words' writer in steps: python summarize-wchurn.py <data dir>."""

import json
import sys
from pathlib import Path

data = Path(sys.argv[1])
lines = [json.loads(line) for line in (data / "churn.jsonl").read_text(encoding="utf-8").splitlines()]


def table(header, rows):
    print("| " + " | ".join(header) + " |")
    print("|" + "|".join("---" for _ in header) + "|")
    for row in rows:
        print("| " + " | ".join(str(cell) for cell in row) + " |")
    print()


def arm(context):
    if context.get("purge_at"):
        return f"steps of {context['pack_keys']}, purged at {context['purge_at']:.0%}"
    if context.get("pack_keys"):
        return f"steps of {context['pack_keys']}"
    return "whole"


print("## Every 5,000 operations\n")
rows = []
for c in lines:
    if c["step"] != "churn":
        continue
    rows.append([arm(c["context"]), c["op"], f"{c['or_p50_ms']:.2f}", f"{c['or_p99_ms']:.2f}",
                 f"{c['pruned_p50_ms']:.2f}", f"{c['pruned_p99_ms']:.2f}", f"{c['set_p50_ms']:.2f}",
                 f"{c['set_p99_ms']:.2f}", f"{c['head_rows']:,}", f"{c['gone']:,}", c["held_commits"],
                 f"{c['held_p99_ms']:.0f}", f"{c['held_max_ms']:.0f}", c["purges"], f"{c['purge_ms'] / 1e3:.1f}",
                 c["checked"], c["differ"]])
table(["arm", "operations", "or p50", "or p99", "pruned p50", "pruned p99", "set p50", "set p99", "head",
       "gone", "commits held", "held p99 ms", "held max ms", "purges", "purging s", "checked", "differ"], rows)

print("## Purges\n")
table(["arm", "op", "keys", "read ms", "s"],
      [[arm(c["context"]), c["op"], f"{c['keys']:,}", f"{c['read_ms']:.0f}", f"{c['millis'] / 1e3:.1f}"]
       for c in lines if c["step"] == "purge"])

print("## Whole runs\n")
table(["arm", "searches", "writes", "checked", "differ", "packs", "pack mean ms", "pack max ms", "HWM MiB"],
      [[arm(c["context"]), c["searches"], c["writes"], c["checked"], c["differ"], c["packs"],
        f"{c['pack_mean_ms']:.0f}", f"{c['pack_max_ms']:.0f}", f"{c['memory']['hwm_kib'] / 1024:.0f}"]
       for c in lines if c["step"] == "churned"])
