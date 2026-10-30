"""How far a bounded look's answers are from the whole's: the documents of
the same stores looked at unbounded (`--unbounded`, or a build before the
bound) and bounded, number by number and panel by panel.

    python drift.py <unbounded-dir> <bounded-dir> [--out drift.json]

Each directory holds `<store>.doc.json` as `see --doc` writes it. A number
is compared by its relative error; a chart by day over the days both have,
by its mean relative error and its correlation; a ranking by how many of its
first five are the first five of the whole's. What the bound refused is
counted apart, as what the screen loses.
"""

import argparse
import glob
import json
import math
import os
import statistics


def value(shown):
    s = (shown or "").replace(",", "").replace("%", "").strip()
    try:
        return float(s)
    except ValueError:
        return None


def panels(doc):
    out = {p["key"]: p for p in doc.get("panels", [])}
    for ps in doc.get("spares", {}).values():
        for p in ps:
            out.setdefault(p["key"], p)
    return out


def num(v):
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def series(p):
    return {r[0]: num(r[-1]) for r in p.get("rows", []) if r and num(r[-1]) is not None}


def corr(a, b):
    if len(a) < 3:
        return None
    ma, mb = statistics.fmean(a), statistics.fmean(b)
    va = sum((x - ma) ** 2 for x in a)
    vb = sum((y - mb) ** 2 for y in b)
    if va == 0 or vb == 0:
        return None
    return sum((x - ma) * (y - mb) for x, y in zip(a, b)) / math.sqrt(va * vb)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("whole")
    ap.add_argument("bounded")
    ap.add_argument("--out")
    a = ap.parse_args()
    numbers, timed, ranked, refused, stores = [], [], [], {"numbers": 0, "panels": 0}, []
    counted = {"numbers": 0, "panels": 0}
    for f in sorted(glob.glob(os.path.join(a.whole, "*.doc.json"))):
        name = os.path.basename(f)[: -len(".doc.json")]
        g = os.path.join(a.bounded, name + ".doc.json")
        if not os.path.exists(g):
            continue
        w, b = json.load(open(f, encoding="utf-8")), json.load(open(g, encoding="utf-8"))
        bn = {n["title"]: n for n in b["numbers"]}
        for n in w["numbers"]:
            if n.get("kind") == "funnel" or n["title"] not in bn:
                continue
            x, y = value(n["shown"]), value(bn[n["title"]]["shown"])
            if x is None:
                continue
            counted["numbers"] += 1
            if y is None:
                refused["numbers"] += 1
                continue
            numbers.append({"store": name, "title": n["title"], "whole": x, "bounded": y, "error": abs(y - x) / abs(x) if x else (0.0 if y == 0 else 1.0)})
        wp, bp = panels(w), panels(b)
        for key, p in wp.items():
            if key not in bp or p.get("error") or not p.get("rows"):
                continue
            q = bp[key]
            counted["panels"] += 1
            if q.get("error") or not q.get("rows"):
                refused["panels"] += 1
                continue
            if p.get("bucket") and len(p["rows"][0]) == 2:
                s, t = series(p), series(q)
                days = sorted(set(s) & set(t))
                if not days:
                    continue
                errs = [abs(t[d] - s[d]) / abs(s[d]) for d in days if s[d]]
                timed.append({"store": name, "title": key, "days": len(days), "of": len(s), "error": statistics.median(errs) if errs else 0.0,
                              "corr": corr([s[d] for d in days], [t[d] for d in days])})
            elif not p.get("bucket") and len(p["rows"][0]) == 2:
                top_w = [r[0] for r in p["rows"][:5]]
                top_b = [r[0] for r in q["rows"][:5]]
                ranked.append({"store": name, "title": key, "top5": len(set(top_w) & set(top_b)) / max(1, min(5, len(top_w)))})
        stores.append(name)

    def share(xs, cut):
        return round(sum(1 for x in xs if x <= cut) / len(xs), 3) if xs else None

    errs = [n["error"] for n in numbers]
    terr = [t["error"] for t in timed]
    tcor = [t["corr"] for t in timed if t["corr"] is not None]
    summary = {
        "stores": len(stores),
        "numbers": {"compared": counted["numbers"], "refused": refused["numbers"], "within_1pct": share(errs, 0.01), "within_10pct": share(errs, 0.10), "within_25pct": share(errs, 0.25),
                    "median_error": round(statistics.median(errs), 4) if errs else None},
        "panels": {"compared": counted["panels"], "refused": refused["panels"]},
        "by_day": {"compared": len(timed), "median_error": round(statistics.median(terr), 4) if terr else None, "within_10pct": share(terr, 0.10),
                   "median_corr": round(statistics.median(tcor), 3) if tcor else None, "median_days_kept": statistics.median([t["days"] / max(1, t["of"]) for t in timed]) if timed else None},
        "ranked": {"compared": len(ranked), "median_top5": statistics.median([r["top5"] for r in ranked]) if ranked else None,
                   "all_five": share([1 - r["top5"] for r in ranked], 0.0)},
    }
    if a.out:
        json.dump({"summary": summary, "numbers": numbers, "by_day": timed, "ranked": ranked}, open(a.out, "w", encoding="utf-8"), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == "__main__":
    main()
