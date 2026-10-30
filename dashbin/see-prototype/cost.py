"""What a look costs a store, measured as chapter 14 asks: every statement a
look ran (the lines `SEE_TRACE` wrote), run again under `EXPLAIN (ANALYZE,
BUFFERS)` in round 1's container with the settings it ran with, and for every
table the most rows one statement read of it, whether a statement read a big
one whole, and the time the server spent on the statements that read it.

    python cost.py <trace.jsonl> --database <db> --snapshot <snapshot.json> [--out cost.json]

A table is big above 10 000 rows by the catalog's estimate. A statement
reads a table whole when its plan scans it in sequence, or along an index with
no condition to look up; it reads of a table what each scan node over it
returned and threw away, times its loops.
"""

import argparse
import json
import subprocess
import sys
from collections import defaultdict

LIMIT = 10_000
BUDGET_MS = 2_000


def explain(container, user, database, sql, sets, timeout_ms):
    script = f"BEGIN;\nSET LOCAL statement_timeout = {timeout_ms};\n" + "".join(f"{s};\n" for s in sets) \
        + f"EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) {sql};\nROLLBACK;\n"
    p = subprocess.run(["docker", "exec", "-i", container, "psql", "-U", user, "-d", database, "-At", "-q", "-v", "ON_ERROR_STOP=1", "-f", "-"],
                       input=script.encode("utf-8"), capture_output=True)
    if p.returncode != 0:
        return None, p.stderr.decode("utf-8", "replace").strip().splitlines()[-1:] or ["error"]
    return json.loads(p.stdout.decode("utf-8")), None


def reads(node, out):
    """Rows each scan node read of its table: returned and thrown away, times its loops."""
    rel = node.get("Relation Name")
    if rel:
        loops = node.get("Actual Loops", 1) or 1
        n = (node.get("Actual Rows", 0) + node.get("Rows Removed by Filter", 0) + node.get("Rows Removed by Index Recheck", 0)) * loops
        r = out[rel]
        r["rows"] += n
        r["kinds"].add(node["Node Type"])
        if node["Node Type"] == "Seq Scan" or (node["Node Type"] in ("Index Scan", "Index Only Scan") and "Index Cond" not in node):
            r["whole"] = True
    for child in node.get("Plans", []):
        reads(child, out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("trace")
    ap.add_argument("--database", required=True)
    ap.add_argument("--snapshot", required=True)
    ap.add_argument("--container", default="lemmesee-round-1-postgres-1")
    ap.add_argument("--user", default="lemmesee")
    ap.add_argument("--timeout-ms", type=int, default=30_000)
    ap.add_argument("--out")
    a = ap.parse_args()
    snap = json.load(open(a.snapshot, encoding="utf-8"))
    estimate = {t["name"]: ((t.get("features", {}).get("rows_estimate") or {}).get("value") or 0) for t in snap["tables"]}
    lines = [json.loads(l) for l in open(a.trace, encoding="utf-8") if l.strip()]
    tables = defaultdict(lambda: {"most_rows": 0, "ms": 0.0, "whole": 0, "statements": 0})
    statements = []
    for x in lines:
        # answered from the catalog, refused, or given way to a sample: no row read
        if x.get("known") or x.get("gave_way") or (x.get("error") or "").startswith("not run"):
            statements.append({"sql": x["sql"], "skipped": x.get("known") or x.get("error") or "gave way"})
            continue
        plan, err = explain(a.container, a.user, a.database, x["sql"], x.get("sets", []), a.timeout_ms)
        if err:
            statements.append({"sql": x["sql"], "error": err[0], "look_ms": x["ms"]})
            continue
        top = plan[0]
        per = defaultdict(lambda: {"rows": 0, "kinds": set(), "whole": False})
        reads(top["Plan"], per)
        ms = top.get("Execution Time", 0.0) + top.get("Planning Time", 0.0)
        for rel, r in per.items():
            t = tables[rel]
            t["most_rows"] = max(t["most_rows"], r["rows"])
            t["ms"] += ms
            t["statements"] += 1
            if r["whole"] and estimate.get(rel, 0) > LIMIT:
                t["whole"] += 1
        statements.append({"sql": x["sql"], "ms": round(ms, 1), "look_ms": round(x["ms"], 1),
                           "reads": {k: {"rows": v["rows"], "kinds": sorted(v["kinds"])} for k, v in per.items()}})
    big = {k: v for k, v in tables.items() if estimate.get(k, 0) > LIMIT}
    summary = {
        "database": a.database,
        "statements": len(lines),
        "skipped": sum(1 for s in statements if "skipped" in s),
        "refused": sum(1 for s in statements if str(s.get("skipped", "")).startswith("not run")),
        "errors": sum(1 for s in statements if "error" in s),
        "server_ms": round(sum(s.get("ms", 0) for s in statements)),
        "tables_read": len(tables),
        "big_tables_read": len(big),
        "over_rows": sorted(k for k, v in tables.items() if v["most_rows"] > LIMIT),
        "read_whole": sorted(k for k, v in big.items() if v["whole"]),
        "over_time": sorted(k for k, v in tables.items() if v["ms"] > BUDGET_MS),
        "most_rows": max((v["most_rows"] for v in tables.values()), default=0),
        "most_ms_a_table": round(max((v["ms"] for v in tables.values()), default=0)),
    }
    doc = {"summary": summary, "tables": {k: {**v, "ms": round(v["ms"], 1), "estimate": estimate.get(k, 0)} for k, v in tables.items()}, "statements": statements}
    if a.out:
        json.dump(doc, open(a.out, "w", encoding="utf-8"), indent=1)
    print(json.dumps(summary))


if __name__ == "__main__":
    sys.exit(main())
