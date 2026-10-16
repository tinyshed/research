"""The blocks round's tables: python summarize-blocks.py <data dir>.

Every cell two rounds measured is written `round 1; round 2`; reads after
the sets are a table of their own.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

data = Path(sys.argv[1])


def lines(name):
    return [json.loads(line) for line in (data / name).read_text(encoding="utf-8").splitlines()]


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
    rows.append([b["backend"], b["documents"], mib(index["bytes"]), objects, f"{index['seconds']:.0f}",
                 index.get("postings", ""), index.get("blocks", ""), f"{b['memory']['hwm_kib'] / 1024:.0f}"])
table(["backend", "documents", "store MiB", "objects MiB", "build s", "postings", "blocks", "build HWM MiB"], rows)

print("## Checks against scoring every posting\n")
rows = []
for c in lines("checks.jsonl"):
    rows.append(["after the sets" if c.get("after") else "built", c["plan"], c["cut"], c["questions"], c["differ"],
                 f"{c['blocks_opened']:.1f}", f"{c['blocks_opened_exhaustive']:.1f}", f"{c['mean_ms']:.2f}",
                 f"{c['mean_ms_exhaustive']:.2f}"])
table(["index", "plan", "cut", "questions", "differ", "blocks opened", "blocks, every posting", "mean ms",
       "mean ms, every posting"], rows)

queried = lines("queries.jsonl")
for title, wanted in (("Queries, 600 questions", lambda r: r != "after"), ("Queries after the sets", lambda r: r == "after")):
    print(f"## {title}\n")
    groups = defaultdict(list)
    for q in queried:
        if q["step"] == "query" and wanted(q["context"]["round"]):
            c = q["context"]
            groups[(c["backend"], q["plan"], c["cut"] if q["plan"] != "or" else "-")].append(q)
    rows = []
    for (backend, plan, cut), members in sorted(groups.items()):
        members.sort(key=lambda m: m["context"]["round"])
        cell = lambda field, digits: "; ".join(f"{m[field]:.{digits}f}" for m in members)
        rows.append([backend, plan, cut, cell("p50_ms", 2), cell("p99_ms", 2), cell("hydrated_p99_ms", 2),
                     cell("per_second", 0), cell("ndcg10", 4), cell("recall10", 4), cell("mrr10", 4)])
    table(["backend", "plan", "cut", "p50 ms", "p99 ms", "read back p99 ms", "queries/s", "nDCG@10",
           "Recall@10", "MRR@10"], rows)

print("## Query processes\n")
rows = []
for q in queried:
    if q["step"] == "memory":
        c = q["context"]
        rows.append([c["backend"], c["cut"], c["round"], f"{q['open_millis']:.0f}",
                     f"{q['baseline']['rss_kib'] / 1024:.0f}", f"{q['memory']['hwm_kib'] / 1024:.0f}"])
table(["backend", "cut", "round", "open ms", "RSS before open MiB", "HWM MiB"], rows)

print("## Sets\n")
rows = []
for w in lines("writes.jsonl"):
    rows.append([w["backend"], w["round"], f"{w['single_p50_ms']:.2f}", f"{w['single_p90_ms']:.2f}",
                 f"{w['single_p99_ms']:.2f}", f"{w['single_max_ms']:.2f}",
                 "; ".join(f"{batch:.0f}" for batch in w["batches_ms"]), f"{w['memory']['hwm_kib'] / 1024:.0f}"])
table(["backend", "round", "one set p50 ms", "p90 ms", "p99 ms", "max ms", "100 sets a commit, ms each",
       "HWM MiB"], rows)
