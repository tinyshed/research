"""The million-document round's tables: python summarize1m.py <data dir>.

Every cell two rounds measured is written `round 1; round 2`.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

data = Path(sys.argv[1])


def lines(name):
    return [json.loads(line) for line in (data / name).read_text(encoding="utf-8").splitlines()]


builds = lines("builds.jsonl")
queried = lines("queries.jsonl")
writes = lines("writes.jsonl")


def table(header, rows):
    print("| " + " | ".join(header) + " |")
    print("|" + "|".join("---" for _ in header) + "|")
    for row in rows:
        print("| " + " | ".join(str(cell) for cell in row) + " |")
    print()


def mib(kib):
    return f"{kib / 1024:.0f}"


print("## Builds\n")
rows = []
for b in builds:
    index = b["index"]
    objects = dict(index.get("objects", []))
    rows.append([
        b["backend"], b["documents"], f"{b['source_bytes'] / 2**20:.0f}", f"{index['bytes'] / 2**20:.0f}",
        f"{objects.get('docs', 0) / 2**20:.0f}", f"{objects.get('idx_data', 0) / 2**20:.0f}",
        f"{index['seconds']:.0f}", index.get("frequent_terms", ""),
        f"{index['frequent_seconds']:.1f}" if "frequent_seconds" in index else "",
        mib(b["memory"]["hwm_kib"]),
    ])
table(["backend", "documents", "source MiB", "store MiB", "docs MiB", "idx_data MiB", "build s",
       "frequent terms", "frequent s", "build HWM MiB"], rows)

print("## Queries, 600 questions\n")
groups = defaultdict(list)
for q in queried:
    if q["step"] == "query":
        c = q["context"]
        groups[(c["backend"], q["plan"], c["cut"] if q["plan"] != "or" else "—")].append(q)
rows = []
for (backend, plan, cut), members in sorted(groups.items()):
    members.sort(key=lambda m: m["context"]["round"])
    cell = lambda field, digits: "; ".join(f"{m[field]:.{digits}f}" for m in members)
    rows.append([backend, plan, cut, cell("p50_ms", 2), cell("p99_ms", 2), cell("hydrated_p50_ms", 2),
                 cell("hydrated_p99_ms", 2), cell("per_second", 0), cell("ndcg10", 4), cell("recall10", 4),
                 cell("mrr10", 4)])
table(["backend", "plan", "cut", "p50 ms", "p99 ms", "hydrated p50 ms", "hydrated p99 ms", "queries/s",
       "nDCG@10", "Recall@10", "MRR@10"], rows)

print("## Query processes\n")
rows = []
for q in queried:
    if q["step"] == "memory":
        c = q["context"]
        rows.append([c["backend"], c["cut"], c["round"], f"{q['open_millis']:.0f}",
                     mib(q["baseline"]["rss_kib"]), mib(q["memory"]["rss_kib"]), mib(q["memory"]["hwm_kib"])])
table(["backend", "cut", "round", "open ms", "RSS before open MiB", "RSS at end MiB", "HWM MiB"], rows)

print("## Sets\n")
rows = []
for w in writes:
    rows.append([w["backend"], w["round"], f"{w['single_p50_ms']:.2f}", f"{w['single_p90_ms']:.2f}",
                 f"{w['single_p99_ms']:.2f}", f"{w['single_max_ms']:.2f}",
                 "; ".join(f"{batch:.0f}" for batch in w["batches_ms"]), mib(w["memory"]["hwm_kib"])])
table(["backend", "round", "one set p50 ms", "p90 ms", "p99 ms", "max ms", "100 sets a commit, ms each",
       "HWM MiB"], rows)

files = data / "index-files.txt"
if files.exists():
    print("## Files after the writes\n")
    table(["bytes", "path"], [line.split("\t") for line in files.read_text(encoding="utf-8").splitlines()])
