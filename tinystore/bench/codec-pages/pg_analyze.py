"""H1.4: the same codec orders in PostgreSQL and in a file with no pages.

    python3 -I pg_analyze.py <pg results.jsonl> <store results dir> <out dir>

For every dataset, block length and storage (EXTERNAL, EXTENDED), Kendall's
tau-b between the codecs' payload order and the order of (a) PostgreSQL's
total relation size, every fork of the heap, its TOAST table and both
indexes; (b) the heap and TOAST main forks alone; (c) the pages that hold
tuples, without the pages bulk extension left empty. The no-padding control
is a flat file of length-prefixed bodies, 4 + body bytes a block, beside
SQLite's 8 KiB file of store.py for the same rows. Writes pg-cells.csv,
pg-model.json and pg-summary.json.
"""

import csv
import glob
import json
import os
import sys
from collections import defaultdict

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from analyze import tau_b  # noqa: E402

PAGE = 8192


def main():
    pg_path, store_dir, out = sys.argv[1], sys.argv[2], sys.argv[3]
    os.makedirs(out, exist_ok=True)
    pg = [json.loads(line) for line in open(pg_path)]
    sqlite = {}
    for path in glob.glob(os.path.join(store_dir, "*.store.jsonl")):
        for line in open(path):
            r = json.loads(line)
            if r["page_size"] == 8192 and r["block"] in (240, 1000, 1024):
                sqlite[(r["dataset"], r["codec"], r["block"])] = r
    cells_in = defaultdict(dict)
    model = defaultdict(int)
    for r in pg:
        s = r["sizes"]
        key = (r["dataset"], r["codec"], r["block"])
        st = sqlite[key]
        if st["failed_blocks"]:
            continue
        model["loads"] += 1
        model["heap_pages_exact_accounting"] += r["heap_pages_bad"] == 0
        model["toast_pages_exact_accounting"] += r["toast_pages_bad"] == 0
        model["external_rule_holds"] += r["external_rule_holds"]
        model["chunks_as_predicted"] += r["chunks_as_predicted"]
        model["heap_pages_used_predicted_exactly"] += r["heap_pages_predicted"] == s["heap_pages_used"]
        model["toast_pages_used_predicted_exactly"] += r["toast_pages_predicted"] == s["toast_pages_used"]
        model["heap_pages_used_measured"] += s["heap_pages_used"]
        model["heap_pages_used_predicted"] += r["heap_pages_predicted"]
        model["toast_pages_used_measured"] += s["toast_pages_used"]
        model["toast_pages_used_predicted"] += r["toast_pages_predicted"]
        model["heap_main_pages"] += s["heap_main"] // PAGE
        model["toast_main_pages"] += s["toast_main"] // PAGE
        model["heap_pages_empty"] += r["heap_pages_empty"]
        model["toast_pages_empty"] += r["toast_pages_empty"]
        cells_in[(r["dataset"], r["block"], r["storage"])][r["codec"]] = {
            "payload": st["payload_bytes"], "values": st["values"], "blocks": st["blocks"],
            "pg_total": s["total"], "pg_main_forks": s["heap_main"] + s["toast_main"],
            "pg_pages_with_tuples": (s["heap_pages_used"] + s["toast_pages_used"]) * PAGE,
            "flat": st["payload_bytes"] + 4 * st["blocks"], "sqlite8k": st["file_bytes"],
        }
    cells = []
    for (ds, L, storage), codecs in sorted(cells_in.items()):
        names = sorted(codecs)
        p = [codecs[c]["payload"] for c in names]
        row = {"dataset": ds, "block": L, "storage": storage, "codecs": len(names),
               "values": codecs[names[0]]["values"]}
        for k in ("pg_total", "pg_main_forks", "pg_pages_with_tuples", "flat", "sqlite8k"):
            t, conc, disc, tp, tq = tau_b(p, [codecs[c][k] for c in names])
            row[f"tau_{k}"] = None if t is None else round(t, 4)
            row[f"reversed_{k}"] = disc
            row[f"tied_only_{k}"] = tq
        row["best_payload"] = min(names, key=lambda c: codecs[c]["payload"])
        least = min(codecs[c]["pg_total"] for c in names)
        row["best_pg_total"] = "|".join(c for c in names if codecs[c]["pg_total"] == least)
        # changes only when the payload's best codec is strictly larger in PostgreSQL than another
        row["best_changes"] = codecs[row["best_payload"]]["pg_total"] > least
        for c in names:
            row[f"{c}_payload_B_per_value"] = round(codecs[c]["payload"] / codecs[c]["values"], 4)
            row[f"{c}_pg_total_B_per_value"] = round(codecs[c]["pg_total"] / codecs[c]["values"], 4)
        cells.append(row)
    with open(os.path.join(out, "pg-cells.csv"), "w", newline="") as f:
        fields = sorted({k for c in cells for k in c}, key=lambda k: (not k[0].isalpha(), k))
        lead = ["dataset", "block", "storage", "codecs", "values"]
        w = csv.DictWriter(f, fieldnames=lead + [k for k in fields if k not in lead])
        w.writeheader()
        w.writerows(cells)
    summary = {"model": dict(model)}
    for storage in ("external", "extended"):
        sel = [c for c in cells if c["storage"] == storage]
        summary[storage] = {}
        for k in ("pg_total", "pg_main_forks", "pg_pages_with_tuples", "flat", "sqlite8k"):
            taus = [c[f"tau_{k}"] for c in sel if c[f"tau_{k}"] is not None]
            summary[storage][k] = {
                "cells": len(sel), "cells_tau_below_1": sum(1 for t in taus if t < 1),
                "cells_with_reversal": sum(1 for c in sel if c[f"reversed_{k}"] > 0),
                "reversed_pairs": sum(c[f"reversed_{k}"] for c in sel),
                "tied_only": sum(c[f"tied_only_{k}"] for c in sel),
                "tau_mean": round(sum(taus) / len(taus), 4) if taus else None,
                "tau_min": min(taus) if taus else None}
        summary[storage]["best_changes"] = sum(1 for c in sel if c["best_changes"])
    with open(os.path.join(out, "pg-summary.json"), "w") as f:
        json.dump(summary, f, indent=1, sort_keys=True)
    print(json.dumps(summary, indent=1, sort_keys=True))


if __name__ == "__main__":
    main()
