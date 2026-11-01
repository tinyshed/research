"""Rank statistics, model checks and block-length optima from store.py's output.

    python3 -I analyze.py <results dir> <out dir>

Reads every <dataset>.store.jsonl and <dataset>.encode.jsonl in the results
dir and writes, to the out dir:

  h12-model.json      every SQLite file against its prediction, and each miss
  h11-values.csv      payload and file bytes a value by dataset, length, page, codec
  h11-cells.csv       Kendall's tau-b between payload order and file order a cell
  h11-reversals.csv   every pair of codecs whose order the file reverses
  h13-optima.csv      the block length minimising payload and file, per codec
  summary.json        the aggregates the report quotes

A codec counts on a dataset only if every block of it came back bit for bit
at that block length. File bytes are the whole file, page_count x page size,
page 1 included; the table's own pages and their unused bytes are beside it.
"""

import csv
import glob
import itertools
import json
import os
import sys
from collections import defaultdict

H11_LENGTHS = (240, 1000, 1024)


def tau_b(xs, ys):
    """Kendall's tau-b, and the pair counts behind it."""
    conc = disc = tx = ty = 0
    for (x1, y1), (x2, y2) in itertools.combinations(zip(xs, ys), 2):
        dx, dy = (x1 > x2) - (x1 < x2), (y1 > y2) - (y1 < y2)
        if dx == 0 and dy == 0:
            continue
        if dx == 0:
            tx += 1
        elif dy == 0:
            ty += 1
        elif dx == dy:
            conc += 1
        else:
            disc += 1
    n0 = conc + disc
    denom = ((n0 + tx) * (n0 + ty)) ** 0.5
    return ((conc - disc) / denom if denom else None), conc, disc, tx, ty


def load(results):
    recs = []
    for path in sorted(glob.glob(os.path.join(results, "*.store.jsonl"))):
        with open(path) as f:
            for line in f:
                recs.append(json.loads(line))
    return recs


def main():
    results, out = sys.argv[1], sys.argv[2]
    os.makedirs(out, exist_ok=True)
    recs = load(results)

    # H1.2: the model against every file ------------------------------------------
    keys = ("page_count", "leaf", "interior", "overflow", "dbstat_agrees_with_walk")
    model = {"files": len(recs), "exact": {k: 0 for k in keys}, "all_exact": 0, "misses": [],
             "format_errors": 0, "unaccounted_pages": 0, "freelist_pages": 0,
             "pages_predicted": 0, "pages_measured": 0, "by_page_size": {}}
    for r in recs:
        e = r["exact"]
        for k in keys:
            model["exact"][k] += bool(e[k])
        w = r["measured"]["walk"]
        model["format_errors"] += w["format_errors"]
        model["unaccounted_pages"] += abs(w["unaccounted_pages"])
        model["freelist_pages"] += r["measured"]["freelist"]
        model["pages_predicted"] += r["predicted"]["page_count"]
        model["pages_measured"] += r["measured"]["page_count"]
        bp = model["by_page_size"].setdefault(str(r["page_size"]), {"files": 0, "exact": 0, "rows": 0})
        bp["files"] += 1
        bp["rows"] += r["blocks"]
        if all(e.values()):
            model["all_exact"] += 1
            bp["exact"] += 1
        else:
            model["misses"].append({k: r[k] for k in ("dataset", "codec", "block", "page_size")} |
                                   {"predicted": r["predicted"]["page_count"], "measured": r["measured"]["page_count"],
                                    "exact": e})
    model["rows"] = sum(r["blocks"] for r in recs)
    model["datasets"] = len({r["dataset"] for r in recs})
    with open(os.path.join(out, "h12-model.json"), "w") as f:
        json.dump(model, f, indent=1, sort_keys=True)

    # the values table -------------------------------------------------------------
    by = defaultdict(dict)  # (dataset, L, U) -> codec -> row
    rows = []
    for r in recs:
        t = r["measured"]["walk"]["objects"]["t"]
        unused = (r["measured"]["dbstat"] or {}).get("t", {})
        row = {
            "dataset": r["dataset"], "block": r["block"], "page_size": r["page_size"], "codec": r["codec"],
            "values": r["values"], "blocks": r["blocks"], "failed_blocks": r["failed_blocks"],
            "payload_bytes": r["payload_bytes"], "file_bytes": r["file_bytes"],
            "payload_B_per_value": r["payload_bytes"] / r["values"],
            "file_B_per_value": r["file_bytes"] / r["values"],
            "table_leaf": t["leaf"], "table_interior": t["interior"], "table_overflow": t["overflow"],
            "table_unused_bytes": sum(v for k, v in unused.items() if k.startswith("unused_")),
            "page_count": r["measured"]["page_count"],
        }
        rows.append(row)
        by[(r["dataset"], r["block"], r["page_size"])][r["codec"]] = row
    rows.sort(key=lambda x: (x["dataset"], x["block"], x["page_size"], x["codec"]))
    with open(os.path.join(out, "h11-values.csv"), "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(r for r in rows if r["block"] in H11_LENGTHS)

    # H1.1: rank agreement a cell --------------------------------------------------
    cells, reversals = [], []
    for (ds, L, U), codecs in sorted(by.items()):
        if L not in H11_LENGTHS:
            continue
        ok = {c: v for c, v in codecs.items() if v["failed_blocks"] == 0}
        names = sorted(ok)
        p = [ok[c]["payload_bytes"] for c in names]
        q = [ok[c]["file_bytes"] for c in names]
        tau, conc, disc, tp, tq = tau_b(p, q)
        bestp = min(names, key=lambda c: ok[c]["payload_bytes"])
        bestq = [c for c in names if ok[c]["file_bytes"] == min(q)]
        cells.append({"dataset": ds, "block": L, "page_size": U, "codecs": len(names),
                      "excluded": ",".join(sorted(set(codecs) - set(ok))),
                      "tau_b": None if tau is None else round(tau, 4), "concordant": conc, "reversed": disc,
                      "tied_in_payload": tp, "tied_in_file_only": tq,
                      "best_payload": bestp, "best_file": "|".join(bestq),
                      "best_changes": bestp not in bestq,
                      "values": next(iter(codecs.values()))["values"]})
        for a, b in itertools.combinations(names, 2):
            ra, rb = ok[a], ok[b]
            if (ra["payload_bytes"] - rb["payload_bytes"]) * (ra["file_bytes"] - rb["file_bytes"]) < 0:
                small, large = (a, b) if ra["payload_bytes"] < rb["payload_bytes"] else (b, a)
                s, l = ok[small], ok[large]
                reversals.append({
                    "dataset": ds, "block": L, "page_size": U, "smaller_payload": small, "larger_payload": large,
                    "payload_ratio": round(l["payload_bytes"] / s["payload_bytes"], 4),
                    "file_ratio": round(l["file_bytes"] / s["file_bytes"], 4),
                    "d_leaf": l["table_leaf"] - s["table_leaf"],
                    "d_interior": l["table_interior"] - s["table_interior"],
                    "d_overflow": l["table_overflow"] - s["table_overflow"],
                    "d_payload_bytes": l["payload_bytes"] - s["payload_bytes"],
                    "d_file_bytes": l["file_bytes"] - s["file_bytes"],
                    "mean_body_small": round(s["payload_bytes"] / s["blocks"], 1),
                    "mean_body_large": round(l["payload_bytes"] / l["blocks"], 1),
                })
    for name, data in (("h11-cells.csv", cells), ("h11-reversals.csv", reversals)):
        with open(os.path.join(out, name), "w", newline="") as f:
            if data:
                w = csv.DictWriter(f, fieldnames=list(data[0]))
                w.writeheader()
                w.writerows(data)

    # H1.3: block-length optima ----------------------------------------------------
    sweep = defaultdict(dict)  # (dataset, codec, U) -> L -> row
    for (ds, L, U), codecs in by.items():
        for c, v in codecs.items():
            sweep[(ds, c, U)][L] = v
    optima = []
    for (ds, c, U), lengths in sorted(sweep.items()):
        good = {L: v for L, v in lengths.items() if v["failed_blocks"] == 0}
        if len(good) < len(lengths) or len(good) < 2:
            continue
        lp = min(good, key=lambda L: (good[L]["payload_bytes"], L))
        lf = min(good, key=lambda L: (good[L]["file_bytes"], L))
        optima.append({
            "dataset": ds, "codec": c, "page_size": U, "lengths": len(good),
            "payload_optimal_L": lp, "file_optimal_L": lf,
            # differ only when the payload's best length leaves a strictly larger file
            "differ": good[lp]["file_bytes"] > good[lf]["file_bytes"],
            "payload_B_per_value_at_payload_opt": round(good[lp]["payload_B_per_value"], 5),
            "payload_B_per_value_at_file_opt": round(good[lf]["payload_B_per_value"], 5),
            "file_B_per_value_at_payload_opt": round(good[lp]["file_B_per_value"], 5),
            "file_B_per_value_at_file_opt": round(good[lf]["file_B_per_value"], 5),
            "file_penalty_of_payload_opt": round(good[lp]["file_bytes"] / good[lf]["file_bytes"] - 1, 5),
            "values": good[lp]["values"],
        })
    with open(os.path.join(out, "h13-optima.csv"), "w", newline="") as f:
        if optima:
            w = csv.DictWriter(f, fieldnames=list(optima[0]))
            w.writeheader()
            w.writerows(optima)

    # the aggregates ---------------------------------------------------------------
    def agg(sel):
        taus = [c["tau_b"] for c in sel if c["tau_b"] is not None]
        return {"cells": len(sel), "cells_tau_below_1": sum(1 for t in taus if t < 1),
                "cells_with_reversal": sum(1 for c in sel if c["reversed"] > 0),
                "reversed_pairs": sum(c["reversed"] for c in sel),
                "pairs": sum(c["concordant"] + c["reversed"] + c["tied_in_payload"] + c["tied_in_file_only"] for c in sel),
                "tied_in_file_only": sum(c["tied_in_file_only"] for c in sel),
                "best_changes": sum(1 for c in sel if c["best_changes"]),
                "tau_min": min(taus) if taus else None,
                "tau_mean": round(sum(taus) / len(taus), 4) if taus else None}
    summary = {"h12": {k: model[k] for k in ("files", "all_exact", "exact", "rows", "datasets", "format_errors",
                                             "unaccounted_pages", "freelist_pages", "pages_predicted",
                                             "pages_measured")},
               "h11": {"all": agg(cells)}, "h13": {}}
    for U in sorted({c["page_size"] for c in cells}):
        summary["h11"][f"U{U}"] = agg([c for c in cells if c["page_size"] == U])
    for L in H11_LENGTHS:
        summary["h11"][f"L{L}"] = agg([c for c in cells if c["block"] == L])
    big = [c for c in cells if c["values"] >= 100_000]
    summary["h11"]["datasets_of_100k_values_or_more"] = agg(big)
    def h13(sel):
        pen = sorted(o["file_penalty_of_payload_opt"] for o in sel if o["differ"])
        return {"curves": len(sel), "differ": len(pen),
                "penalty_median": pen[len(pen) // 2] if pen else None,
                "penalty_over_5_percent": sum(1 for x in pen if x > 0.05),
                "penalty_max": pen[-1] if pen else None}
    summary["h13"] = {"all": h13(optima), "datasets_of_100k_values_or_more": h13([o for o in optima if o["values"] >= 100_000])}
    for U in sorted({o["page_size"] for o in optima}):
        summary["h13"][f"U{U}"] = h13([o for o in optima if o["page_size"] == U])
    for c in sorted({o["codec"] for o in optima}):
        summary["h13"][c] = h13([o for o in optima if o["codec"] == c])
    failures = defaultdict(int)
    for r in recs:
        if r["failed_blocks"] and r["page_size"] == 4096:
            failures[f'{r["dataset"]} {r["codec"]} L{r["block"]}'] = r["failed_blocks"]
    summary["failed"] = dict(failures)
    with open(os.path.join(out, "summary.json"), "w") as f:
        json.dump(summary, f, indent=1, sort_keys=True)
    print(json.dumps(summary, indent=1, sort_keys=True))


if __name__ == "__main__":
    main()
