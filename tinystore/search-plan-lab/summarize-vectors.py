"""The vectors round's tables: python summarize-vectors.py <data dir>.

Every cell two rounds measured is written `round 1; round 2`.
"""

import json
import sys
from collections import defaultdict
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


def mib(value):
    return f"{value / 2**20:,.0f}"


print("## Builds\n")
rows = []
for b in lines("builds.jsonl"):
    if b["step"] == "truth":
        rows.append(["truth in memory", "", "", f"{b['seconds']:.0f}", ""])
        continue
    seconds = b["seconds"]["total"] if isinstance(b["seconds"], dict) else b["seconds"]
    objects = ", ".join(f"{name} {mib(size)}" for name, size in b.get("objects", []) if size >= 2**20)
    rows.append([b["backend"], mib(b["bytes"]), objects, f"{seconds:.0f}", f"{b['memory']['hwm_kib'] / 1024:,.0f}"])
table(["backend", "file MiB", "objects MiB", "build s", "build HWM MiB"], rows)
for b in lines("builds.jsonl"):
    if b.get("backend") == "vectors":
        print("clusters:", json.dumps({key: b[key] for key in ("members", "cap_cos_min", "fits", "seconds")}), "\n")

print("## Arms\n")
groups = defaultdict(list)
memories = defaultdict(list)
for q in lines("queries.jsonl"):
    c = q["context"]
    if q["step"] == "vquery":
        groups[c["arm"]].append(q)
    elif q["step"] == "memory":
        memories[c["arm"]].append(f"{q['memory']['hwm_kib'] / 1024:,.0f}")
rows = []
for arm, members in groups.items():
    members.sort(key=lambda m: m["context"]["round"])
    cell = lambda field, digits: "; ".join(f"{m[field]:.{digits}f}" for m in members)
    read = members[0]["read"]
    rows.append([arm, members[0]["questions"], cell("p50_ms", 2), cell("p99_ms", 2), cell("recall_exact", 3),
                 cell("ndcg10", 4), "; ".join(str(m["differ"]) for m in members), f"{read['clusters']:.0f}",
                 f"{read['members']:,.0f}", f"{read['bytes'] / 2**20:.2f}", f"{read['rescored']:.0f}",
                 ", ".join(memories[arm])])
table(["arm", "questions", "p50 ms", "p99 ms", "recall@10 of exact", "nDCG@10", "differ", "clusters", "members",
       "MiB read", "rescored", "reader HWM MiB"], rows)

print("## Words, vectors and their fusion, 600 questions\n")
rows = []
for h in lines("hybrid.jsonl") + lines("hybrid-weighted.jsonl"):
    rows.append([h["context"]["arm"], h["mode"], f"{h['ndcg10']:.4f}", f"{h['recall10']:.4f}", f"{h['mrr10']:.4f}",
                 f"{h['p50_ms']:.2f}", f"{h['p99_ms']:.2f}"])
table(["vectors by", "mode", "nDCG@10", "Recall@10", "MRR@10", "p50 ms", "p99 ms"], rows)
