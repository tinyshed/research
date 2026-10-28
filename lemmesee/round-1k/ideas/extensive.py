"""Extensive or intensive: a sum means something of a quantity that grows
with what it measures (people, tonnes, money, minutes of delay) and nothing
of one that does not (a person's weight, a rate, a card's number). The
first spans orders of magnitude; the second sits in a band. This reads the
spread of every summed or averaged column of rounds 1d-1f from the database
itself, and asks whether it says what the judges keep that the look missed.

    python -I extensive.py <out>

Each column is read once: p5, p50 and p95 of its values, on the round's
own PostgreSQL and MySQL, with round 1c's limit of five seconds a
statement. No model is fitted.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE.parent))
import local  # noqa: E402

CONFIG = "small, round 1e numbers, per panel"


def quantiles(see, engine: str, database: str, schema_name: str, table: str, col: str):
    """p5, p50 and p95 of the column's positive values, and the share of its
    values that are zero or below: a zero kept for a missing weight is not
    a weight."""
    if engine == "mysql":
        q = f"`{col}` + 0"
        sql = (f"SELECT x FROM (SELECT {q} AS x, ROW_NUMBER() OVER (ORDER BY {q}) AS rn, COUNT(*) OVER () AS n "
               f"FROM `{table}` WHERE {q} > 0) s WHERE rn IN (GREATEST(1, ROUND(0.05 * n)), "
               f"GREATEST(1, ROUND(0.5 * n)), GREATEST(1, ROUND(0.95 * n))) ORDER BY rn")
        rows, err, _ = see.run_sql(engine, database, sql)
        vals = [float(r[0]) for r in rows if r and r[0] is not None]
        share, err2, _ = see.run_sql(engine, database, f"SELECT AVG({q} <= 0) FROM `{table}` WHERE `{col}` IS NOT NULL")
        nonpos = float(share[0][0]) if share and share[0] and share[0][0] is not None else math.nan
        return ((vals + [math.nan] * 3)[:3] if len(vals) >= 3 else [math.nan] * 3) + [nonpos], err or err2
    q = f'"{col}"'
    sql = (f"SELECT percentile_cont(0.05) WITHIN GROUP (ORDER BY x) FILTER (WHERE x > 0), "
           f"percentile_cont(0.5) WITHIN GROUP (ORDER BY x) FILTER (WHERE x > 0), "
           f"percentile_cont(0.95) WITHIN GROUP (ORDER BY x) FILTER (WHERE x > 0), avg((x <= 0)::int) "
           f"FROM (SELECT {q}::float8 AS x FROM \"{schema_name}\".\"{table}\" WHERE {q} IS NOT NULL) s")
    rows, err, _ = see.run_sql(engine, database, sql)
    if rows and all(v is not None for v in rows[0][:4]):
        return [float(v) for v in rows[0][:4]], err
    return [math.nan] * 4, err


def spread(p5: float, p95: float) -> float:
    """Decades between the fifth and the ninety-fifth percentile of the
    positive values."""
    if not (math.isfinite(p5) and math.isfinite(p95)) or p5 <= 0:
        return math.nan
    return math.log10(p95 / p5)


def main(out: Path) -> None:
    from scipy.stats import spearmanr
    sys.argv = [sys.argv[0]]
    dg = local.module("ext_diagnose", ROOT / "round-1g" / "diagnose.py")
    see = dg.look1d.see
    aq = sys.modules["aq"]
    corpus = ROOT / "corpus"
    dirs = [corpus / "round-1d" / "look", corpus / "round-1e" / "look", corpus / "round-1f" / "look"]
    D = dg.load_all(dirs, corpus / "see")
    scores = json.loads((corpus / "round-1g" / "scores.json").read_text(encoding="utf-8"))[CONFIG]
    cache: dict = {}
    rows = []
    for fe in D["fes"]:
        snap = json.loads((corpus / "see" / fe / "snapshot.json").read_text(encoding="utf-8"))
        schema = aq.load(snap, True)
        raw = [t for t in snap["tables"] if t.get("kind", "table") in ("table", "partitioned")]
        for i, (p, ident) in enumerate(zip(D["mine"][fe], D["ids"][fe])):
            agg, cols = ident[1]
            if agg not in ("sum", "avg") or len(cols) != 1:
                continue
            t, c = next(iter(cols))
            key = (fe, t, c)
            if key not in cache:
                tb = schema.tables[t]
                cache[key] = quantiles(see, snap["engine"], snap["database"], raw[t].get("schema", "public"),
                                       tb.name, tb.columns[c].name)
            (p5, p50, p95, nonpos), err = cache[key]
            rows.append({"db": fe, "title": p["title"], "agg": agg, "p5": p5, "p50": p50, "p95": p95, "nonpositive": nonpos,
                         "spread": spread(p5, p95), "magnitude": math.log10(abs(p50)) if p50 and math.isfinite(p50) else math.nan,
                         "teachers": dg.target(p, D["ans"]), "look": float(scores[fe][i]),
                         "judges": [float(bool(D["ans"][j][p["id"]]["keep"])) if p["id"] in D["ans"][j] else math.nan
                                    for j in dg.COUNTERS], "error": err})
    out.mkdir(parents=True, exist_ok=True)
    (out / "extensive-panels.json").write_text(json.dumps(rows, ensure_ascii=False, indent=0), encoding="utf-8")
    result = {"columns read": len(cache), "columns failed": sum(1 for v in cache.values() if not math.isfinite(v[0][3]))}
    for agg in ("sum", "avg"):
        sel = [r for r in rows if r["agg"] == agg and math.isfinite(r["spread"])]
        s = np.array([r["spread"] for r in sel])
        res = {"panels": len(sel)}
        for name, vals in (("teachers", [r["teachers"] for r in sel]), ("look", [r["look"] for r in sel])):
            v = np.array([x if x is not None else math.nan for x in vals], dtype=float)
            ok = np.isfinite(v)
            res[f"spearman with {name}"] = round(float(spearmanr(s[ok], v[ok])[0]), 3)
        for j, judge in enumerate(dg.COUNTERS):
            y = np.array([r["judges"][j] for r in sel])
            look = np.array([r["look"] for r in sel])
            ok = np.isfinite(y)
            res[f"spearman with {judge}"] = round(float(spearmanr(s[ok], y[ok])[0]), 3)
            # what the look missed: the judge's keep less the look's
            res[f"spearman with {judge} less the look"] = round(float(spearmanr(s[ok], (y - look)[ok])[0]), 3)
        bands = {}
        for lo, hi, name in ((0, 0.5, "under half a decade"), (0.5, 1.5, "half to one and a half"), (1.5, 99, "over one and a half")):
            b = [r for r in sel if lo <= r["spread"] < hi]
            if b:
                bands[name] = {"panels": len(b), "teachers": round(float(np.nanmean([r["teachers"] or math.nan for r in b])), 3),
                               "look": round(float(np.mean([r["look"] for r in b])), 3),
                               **{judge: round(float(np.nanmean([r["judges"][j] for r in b])), 3)
                                  for j, judge in enumerate(dg.COUNTERS)}}
        res["bands"] = bands
        # an identifier's band: fifteen digits or more, or a column of zeros kept for nothing
        for name, test in (("magnitude of 10^12 or more", lambda r: r["magnitude"] >= 12),
                           ("half or more zero or below", lambda r: r["nonpositive"] >= 0.5)):
            b = [r for r in rows if r["agg"] == agg and math.isfinite(r["magnitude"]) and test(r)]
            if b:
                res[name] = {"panels": len(b), "teachers": round(float(np.nanmean([r["teachers"] or math.nan for r in b])), 3),
                             "look": round(float(np.mean([r["look"] for r in b])), 3),
                             **{judge: round(float(np.nanmean([r["judges"][j] for r in b])), 3)
                                for j, judge in enumerate(dg.COUNTERS)}}
        result[agg] = res
    (out / "extensive.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
