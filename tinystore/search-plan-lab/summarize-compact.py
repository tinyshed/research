"""The compact round's tables: python summarize-compact.py <data dir>.

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
    return f"{value / 2**20:.0f}"


print("## Builds\n")
rows = []
for b in lines("builds.jsonl"):
    index = b["index"]
    objects = ", ".join(f"{name} {mib(size)}" for name, size in index.get("objects", []) if size >= 2**20)
    rows.append([b["backend"], mib(index["bytes"]), objects, f"{index['seconds']:.0f}", index.get("postings", ""),
                 index.get("blocks", ""), index.get("keys", ""), index.get("inline", ""),
                 f"{b['memory']['hwm_kib'] / 1024:.0f}"])
table(["backend", "store MiB", "objects MiB", "build s", "postings", "blocks", "keys", "keys inline",
       "build HWM MiB"], rows)

print("## Checks against scoring every posting, compact\n")
rows = []
for c in lines("checks.jsonl"):
    rows.append([c["executor"], c["ahead"], c["plan"], c["questions"], c["differ"], f"{c['blocks_opened']:.1f}",
                 f"{c['statements']:.1f}", f"{c['mean_ms']:.2f}", f"{c['directory_ms']:.2f}",
                 f"{c['opening_ms']:.2f}", f"{c['mean_ms_exhaustive']:.2f}"])
table(["executor", "ahead", "plan", "questions", "differ", "blocks opened", "statements", "mean ms",
       "directories ms", "blocks ms", "every posting ms"], rows)

print("## Queries, 600 questions\n")
groups = defaultdict(list)
memories = defaultdict(set)
for q in lines("queries.jsonl"):
    if "context" not in q:
        continue
    c = q["context"]
    if q["step"] == "query":
        groups[(c["arm"], q["plan"], c["cut"] if q["plan"] != "or" else "-")].append(q)
    elif q["step"] == "memory":
        memories[c["arm"]].add(f"{q['memory']['hwm_kib'] / 1024:.0f}")
rows = []
for (arm, plan, cut), members in sorted(groups.items()):
    members.sort(key=lambda m: m["context"]["round"])
    cell = lambda field, digits: "; ".join(f"{m[field]:.{digits}f}" for m in members)
    rows.append([arm, plan, cut, cell("p50_ms", 2), cell("p99_ms", 2), cell("hydrated_p99_ms", 2),
                 cell("per_second", 0), cell("ndcg10", 4), cell("recall10", 4)])
table(["arm", "plan", "cut", "p50 ms", "p99 ms", "read back p99 ms", "queries/s", "nDCG@10", "Recall@10"], rows)
table(["arm", "reader HWM MiB"], [(arm, ", ".join(sorted(values))) for arm, values in sorted(memories.items())])

print("## Churn: 70 searches, 25 sets, 5 deletes in a hundred operations\n")
rows = []
for c in lines("churn.jsonl"):
    if c["step"] == "churn":
        rows.append([c["context"]["pack_every"] or "never", c["op"], f"{c['or_p50_ms']:.2f}", f"{c['or_p99_ms']:.2f}",
                     f"{c['pruned_p50_ms']:.2f}", f"{c['pruned_p99_ms']:.2f}", f"{c['set_p50_ms']:.2f}",
                     f"{c['set_p99_ms']:.2f}", c["head_rows"], c["gone"], f"{c['norms_bytes'] / 2**20:.1f}",
                     c["checked"], c["differ"]])
table(["packed every", "operations", "or p50", "or p99", "pruned p50", "pruned p99", "set p50", "set p99",
       "head rows", "gone", "norms MiB", "checked", "differ"], rows)
rows = []
for c in lines("churn.jsonl"):
    if c["step"] == "churned":
        rows.append([c["context"]["pack_every"] or "never", c["searches"], c["writes"], c["checked"], c["differ"],
                     c["packs"], f"{c['pack_mean_ms']:.0f}", f"{c['pack_max_ms']:.0f}",
                     f"{c['memory']['hwm_kib'] / 1024:.0f}"])
table(["packed every", "searches", "writes", "checked", "differ", "packs", "pack mean ms", "pack max ms",
       "HWM MiB"], rows)
