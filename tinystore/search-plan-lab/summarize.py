"""The round's tables from its raw lines: python summarize.py <data dir>.

Every cell that two rounds measured is written `round 1; round 2`.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

data = Path(sys.argv[1])
builds = [json.loads(line) for line in (data / "builds.jsonl").read_text(encoding="utf-8").splitlines()]
lines = [json.loads(line) for line in (data / "queries.jsonl").read_text(encoding="utf-8").splitlines()]
queries = [line for line in lines if line["step"] == "query"]
memories = [line for line in lines if line["step"] == "memory"]
opens = [line for line in lines if line["step"] == "open"]
typos = [line for line in lines if line["step"] == "typos"]


def key(line, *fields):
    context = line["context"]
    return tuple(context[field] if field in context else line[field] for field in fields)


def cells(rows, field, digits):
    return "; ".join(f"{row[field]:.{digits}f}" for row in sorted(rows, key=lambda row: row["context"]["round"]))


def table(header, rows):
    print("| " + " | ".join(header) + " |")
    print("|" + "|".join("---" for _ in header) + "|")
    for row in rows:
        print("| " + " | ".join(str(cell) for cell in row) + " |")
    print()


def grouped(selected, *fields):
    groups = defaultdict(list)
    for line in selected:
        groups[key(line, *fields)].append(line)
    return groups


def quality_and_speed(selected, *fields):
    rows = []
    for group, members in grouped(selected, *fields).items():
        rows.append(list(group) + [
            cells(members, "p50_ms", 2), cells(members, "p99_ms", 2), cells(members, "per_second", 0),
            cells(members, "ndcg10", 4), cells(members, "recall10", 4), cells(members, "mrr10", 4),
            members[0]["empty"], f"{members[0]['index_queries']:.1f}",
        ])
    return rows


speed = ["p50 ms", "p99 ms", "queries/s", "nDCG@10", "Recall@10", "MRR@10", "empty", "index queries"]

print("## Plans: FTS5 against Tantivy, analysis auto, cut 0.05\n")
selected = [q for q in queries if q["context"]["analysis"] == "auto" and q["context"]["variant"] == "clean"
            and q["context"]["matching"] == "exact" and q["context"]["cut"] == 0.05
            and q["context"]["cohort"] in ("ru", "en", "zh", "mixed")]
# The typo section answers some of these again later; the plans' own runs come first.
seen = set()
unique = []
for q in selected:
    marker = (q["context"]["cohort"], q["context"]["backend"], q["plan"], q["context"]["round"])
    if marker not in seen:
        seen.add(marker)
        unique.append(q)
table(["cohort", "backend", "plan"] + speed, quality_and_speed(unique, "cohort", "backend", "plan"))

print("## Cut: the share of documents past which a word does not drive a query, mixed, FTS5\n")
selected = [q for q in queries if q["context"]["cohort"] == "mixed" and q["context"]["backend"] == "fts"
            and q["plan"] in ("pruned-or", "adaptive-pruned") and q["context"]["variant"] == "clean"
            and q["context"]["matching"] == "exact"]
table(["cut", "plan"] + speed, sorted(quality_and_speed(selected, "cut", "plan")))

print("## Analyses: segmentation and stems, FTS5, cut 0.05\n")
selected = [q for q in queries if q["context"]["backend"] == "fts" and q["context"]["variant"] == "clean"
            and q["context"]["matching"] == "exact" and q["context"]["cut"] == 0.05
            and (q["context"]["cohort"] in ("zh", "ja", "th", "de", "fr", "es") or q["context"]["analysis"] == "surface")
            and q["plan"] in ("or", "pruned-or")]
first_round = {}
for q in selected:
    first_round.setdefault((q["context"]["cohort"], q["context"]["analysis"], q["plan"], q["context"]["round"]), q)
table(["cohort", "analysis", "plan"] + speed, sorted(quality_and_speed(first_round.values(), "cohort", "analysis", "plan")))

print("## Typos: one in each question's longest word, FTS5, analysis auto\n")
selected = [q for q in queries if q["context"]["analysis"] == "auto" and q["context"]["backend"] == "fts"
            and q["context"]["cut"] == 0.05 and q["context"]["cohort"] in ("ru", "en", "mixed")
            and q["plan"] in ("or", "pruned-or")]
by_variant = {}
for q in selected:
    marker = (q["context"]["cohort"], q["context"]["variant"], q["context"]["matching"], q["plan"], q["context"]["round"])
    by_variant[marker] = q
table(["cohort", "variant", "matching", "plan"] + speed,
      sorted(quality_and_speed(by_variant.values(), "cohort", "variant", "matching", "plan")))

print("## Typo FSTs\n")
table(["terms", "FST bytes", "build ms"], sorted({(t["terms"], t["bytes"], f"{t['millis']:.0f}") for t in typos}))

print("## Builds\n")
rows = []
for b in builds:
    objects = dict(b["index"].get("objects", []))
    rows.append([b["cohort"], b["analysis"], b["backend"], b["documents"], b["questions"],
                 f"{b['source_bytes'] / 2**20:.1f}", f"{b['index']['bytes'] / 2**20:.1f}",
                 f"{objects.get('idx_data', 0) / 2**20:.1f}", f"{b['index']['seconds']:.1f}",
                 f"{(b['memory'] or {}).get('hwm_kib', 0) / 1024:.0f}"])
table(["cohort", "analysis", "backend", "documents", "questions", "source MiB", "index MiB", "idx_data MiB",
       "build s", "build HWM MiB"], rows)

print("## Query processes: memory and opening\n")
rows = []
for m in memories:
    c = m["context"]
    rows.append([c["cohort"], c["analysis"], c["backend"], c["variant"], c["matching"], c["cut"], c["round"],
                 f"{m['open_millis']:.0f}", f"{(m['memory'] or {}).get('hwm_kib', 0) / 1024:.0f}"])
table(["cohort", "analysis", "backend", "variant", "matching", "cut", "round", "open ms", "HWM MiB"], rows)

print("## Frequent-term tables at open\n")
table(["terms", "ms"], [(o["frequent_terms"], f"{o['frequent_millis']:.0f}") for o in opens][:12])

sizes = data / "binary-sizes.json"
if sizes.exists():
    print("## Binary sizes\n")
    entries = json.loads(sizes.read_text(encoding="utf-8"))
    base = entries[0]["bytes"]
    table(["features", "bytes", "MiB", "over the baseline MiB"],
          [(e["features"], e["bytes"], f"{e['bytes'] / 2**20:.3f}", f"{(e['bytes'] - base) / 2**20:.3f}") for e in entries])
