"""The vector writes round's tables: python summarize-vchurn.py <data dir>."""

import json
import sys
from pathlib import Path

data = Path(sys.argv[1])


def lines(name):
    path = data / name
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]


def table(header, rows):
    print("| " + " | ".join(header) + " |")
    print("|" + "|".join("---" for _ in header) + "|")
    for row in rows:
        print("| " + " | ".join(str(cell) for cell in row) + " |")
    print()


for b in lines("builds.jsonl"):
    print("build:", json.dumps({key: b[key] for key in ("documents", "clusters", "members", "seconds", "bytes")}), "\n")

churn = lines("churn.jsonl")
print("## Searches at each checkpoint\n")
rows = []
for c in churn:
    f, g, e = c["fixed"], c["budget"], c["exact"]
    rows.append([c["context"]["mode"], c["batch"], f"{c['live']:,}", f"{c['head_rows']:,}", c["clusters"],
                 f"{c['members_max']:.0f}", f"{f['p50_ms']:.2f}", f"{f['p99_ms']:.2f}", f"{f['recall']:.3f}",
                 f"{f['recall_zh']:.3f}", f"{f['members']:,.0f}", f"{g['p50_ms']:.2f}", f"{g['recall']:.3f}",
                 f"{g['recall_zh']:.3f}", f"{e['differ']}", f"{e['p50_ms']:.0f}", f"{c['bytes'] / 2**20:,.0f}"])
table(["mode", "batch", "live", "head", "clusters", "largest", "128 p50", "128 p99", "128 recall", "zh",
       "members read", "budget p50", "budget recall", "zh", "exact differ", "exact p50", "MiB"], rows)

print("## Writes and upkeep between checkpoints\n")
rows = []
for c in churn:
    if c["batch"] == 0:
        continue
    i, u, d, p, s = c["insert_ms"], c["update_ms"], c["delete_ms"], c["pack_ms"], c["split_ms"]
    rows.append([c["context"]["mode"], c["batch"], f"{i['p50']:.2f}", f"{i['p99']:.2f}", f"{u['p50']:.2f}",
                 f"{u['p99']:.2f}", f"{d['p50']:.2f}", f"{d['p99']:.2f}", p["count"], f"{p['p50']:.1f}",
                 f"{p['p99']:.1f}", f"{p['max']:.1f}", f"{p['total'] / 1e3:.1f}", s["count"], f"{s['p50']:.1f}",
                 f"{s['max']:.1f}"])
table(["mode", "batch", "insert p50", "p99", "update p50", "p99", "delete p50", "p99", "packs", "pack p50",
       "p99", "max", "packing s", "splits", "split p50", "max"], rows)
