"""Round 1c's five beside round 1's, run on each fixture and drawn on round
1's page: round 1's five with statistics, round 1c's from the schema alone,
and round 1c's with statistics; a panel that answers a gold question says so.

    python -I see.py <learn dir> <corpus/see> <out dir> <gold.json>...

Reads each fixture's snapshot and round 1's runs from <corpus/see>/<fixture>/,
runs round 1c's panels through the compose servers' command-line clients
with a time limit, and writes <out dir>/<fixture>.html and a summary.
"""

from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import baseline  # noqa: E402
import gold  # noqa: E402
import learn  # noqa: E402
import rank  # noqa: E402
from aq import compile_sql, generate, identity, load  # noqa: E402

PAGE = Path(__file__).parent.parent / "round-1" / "harness" / "run" / "src" / "page.html"
LIMIT_MS = 5000
NULL = "<<null>>"


def run_sql(engine: str, database: str, sql: str) -> tuple[list[list[str | None]], str | None, float]:
    started = time.perf_counter()
    if engine == "mysql":
        cmd = ["docker", "exec", "-i", "-e", "MYSQL_PWD=lemmesee", "lemmesee-round-1-mysql-1", "mysql", "-ulemmesee",
               database, "-B", "-N", "-e", f"SET SESSION max_execution_time={LIMIT_MS}; {sql}"]
    else:
        cmd = ["docker", "exec", "-i", "-e", f"PGOPTIONS=-c statement_timeout={LIMIT_MS}", "lemmesee-round-1-postgres-1",
               "psql", "-U", "lemmesee", "-d", database, "-At", "-F", "\t", "-P", f"null={NULL}", "-v", "ON_ERROR_STOP=1",
               "-c", sql]
    p = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
    ms = (time.perf_counter() - started) * 1000
    if p.returncode != 0:
        return [], (p.stderr.strip().splitlines() or ["error"])[-1][:300], ms
    rows = []
    for line in p.stdout.splitlines():
        if not line.strip():
            continue
        rows.append([None if v in (NULL, "NULL") else v for v in line.split("\t")])
    return rows, None, ms


def bucket_for(span_days: float | None) -> str:
    if span_days is None:
        return "month"
    if span_days <= 90:
        return "day"
    if span_days <= 420:
        return "week"
    if span_days <= 6 * 366:
        return "month"
    return "year"


def date_days(v: str | None) -> float | None:
    if not v or len(v) < 10:
        return None
    try:
        y, m, d = int(v[:4]), int(v[5:7]), int(v[8:10])
    except ValueError:
        return None
    return y * 365.25 + m * 30.4 + d


def template_of(q) -> str:
    if q.shape == "series":
        return "T1" if q.measure.kind == "rows" else "T3"
    if q.shape == "split":
        return "T6"
    if q.shape == "distribution":
        return "T5"
    if q.shape == "top":
        return "T9"
    return "T2" if q.measure.kind == "rows" else "T4"


def panel_of(schema, q, engine: str, database: str, pid: str) -> dict:
    bucket = None
    if q.time is not None:
        col = schema.tables[q.time.t].columns[q.time.c]
        if col.fam in ("timestamp", "date"):
            tname = schema.tables[q.time.t].name
            quote = (lambda n: f"`{n}`") if engine == "mysql" else (lambda n: f'"{n}"')
            rows, err, _ = run_sql(engine, database, f"SELECT min({quote(col.name)}), max({quote(col.name)}) FROM {quote(tname)}")
            lo, hi = (rows[0] + [None, None])[:2] if rows else (None, None)
            span = (date_days(hi) - date_days(lo)) if date_days(lo) is not None and date_days(hi) is not None else None
            bucket = bucket_for(span)
        else:
            bucket = "year" if any("year" in t for t in col.tk) else "month"
    sql = compile_sql(schema, q, engine, bucket)
    rows, err, ms = run_sql(engine, database, sql)
    if q.time is not None and schema.tables[q.time.t].columns[q.time.c].fam not in ("timestamp", "date"):
        # a year or a YYYYMM held as an integer, drawn as a date
        fixed = []
        for r in rows:
            v = r[0] or ""
            if len(v) == 4 and v.isdigit():
                r = [f"{v}-01-01"] + r[1:]
            elif len(v) == 6 and v.isdigit():
                r = [f"{v[:4]}-{v[4:]}-01"] + r[1:]
            fixed.append(r)
        rows = fixed
    elif rows and q.time is not None:
        rows = [[(r[0] or "")[:10]] + r[1:] for r in rows]
    agg = {"rows": "count", "per": "count", "flagged": "count", "late": "count", "parents": "count"}.get(q.measure.kind, q.measure.agg)
    return {
        "id": pid, "template": template_of(q), "aggregate": agg, "chart": "", "title": q.title(schema), "sql": sql,
        "bucket": bucket, "columns": [], "rows": rows[:500], "error": err, "full_scan": None, "cost": None,
        "client_ms": ms, "server": {}, "reasons": [], "flags": [], "abstained": None, "top": {},
    }


def why(names: list[str], contrib, x) -> list[str]:
    """The ranker's reasons for a question: the features that raised its
    score most, as SHAP values, with the value each had."""
    pairs = sorted(((float(contrib[k]), names[k], float(x[k])) for k in range(len(names))), key=lambda p: -p[0])
    out = []
    for c, n, v in pairs[:5]:
        if c <= 0.02:
            break
        shown = "unknown" if v != v else (f"{v:g}")
        out.append(f"{n} = {shown}: +{c:.2f}")
    neg = sorted(((float(contrib[k]), names[k], float(x[k])) for k in range(len(names))), key=lambda p: p[0])[:2]
    for c, n, v in neg:
        if c < -0.05:
            out.append(f"{n} = {'unknown' if v != v else f'{v:g}'}: {c:.2f}")
    return out


def main(learn_dir: Path, see: Path, out: Path, golds: list[Path]) -> None:
    import lightgbm as lgb
    model = lgb.Booster(model_file=str(learn_dir / "ranker.txt"))
    names = model.feature_name()
    table = json.loads((learn_dir / "words.json").read_text(encoding="utf-8"))
    out.mkdir(parents=True, exist_ok=True)
    page = PAGE.read_text(encoding="utf-8")
    summary = {}
    for gpath in golds:
        g = json.loads(gpath.read_text(encoding="utf-8"))
        for fx, spec in g["fixtures"].items():
            folder = see / f"{fx}-{spec['engine']}"
            snap = json.loads((folder / "snapshot.json").read_text(encoding="utf-8"))
            r1 = json.loads((folder / "runs.json").read_text(encoding="utf-8"))
            engine, database = snap["engine"], snap["database"]
            panels, configs = [], {}
            # round 1's five with statistics, as it ran
            r1_panels = {p["id"]: p for p in r1["panels"]}
            r1_cands = {c["id"]: c for c in json.loads((folder / "proposal" / "candidates.json").read_text(encoding="utf-8"))["candidates"]}
            schema1 = load(snap, True)
            gold1 = {}
            for n, gq in enumerate(spec["questions"], 1):
                gid, _ = gold.identity(schema1, gq)
                if gid is not None:
                    gold1.setdefault(gid, n)
            configs["O"] = []
            hits = {"round 1": 0}
            for k, pid in enumerate(r1["configs"].get("B1", []), 1):
                p = dict(r1_panels[pid])
                p["id"] = "r1-" + pid
                p["top"] = {"O": k}
                n = gold1.get(baseline.ident(schema1, r1_cands[pid])) if pid in r1_cands else None
                if n:
                    p["flags"] = list(p.get("flags", [])) + [f"gold question {n}"]
                    hits["round 1"] += 1
                panels.append(p)
                configs["O"].append(p["id"])
            for cfg, reads_n in (("A", False), ("B1", True)):
                schema = load(snap, reads_n)
                qs = [q for q in generate(schema) if rank.veto(schema, q) is None]
                row = {"features": [rank.features(schema, q) for q in qs], "slots": [rank.slots(schema, q) for q in qs]}
                X, _, _ = learn.matrix_free(row, names, table)
                scores = model.predict(X)
                contrib = model.predict(X, pred_contrib=True)
                if reads_n:
                    scores = scores + np.array([rank.stats_points(schema, q) for q in qs])
                chosen = rank.choose(schema, qs, list(scores))
                gold_ids = {}
                for n, gq in enumerate(spec["questions"], 1):
                    gid, _ = gold.identity(schema, gq)
                    if gid is not None:
                        gold_ids.setdefault(gid, n)
                configs[cfg] = []
                hits[cfg] = 0
                for k, i in enumerate(chosen, 1):
                    q = qs[i]
                    pid = f"v2-{cfg}-{k}"
                    p = panel_of(schema, q, engine, database, pid)
                    p["reasons"] = why(names, contrib[i], X[i])
                    n = gold_ids.get(identity(schema, q))
                    if n:
                        p["flags"] = [f"gold question {n}"]
                        hits[cfg] += 1
                    p["top"] = {cfg: k}
                    panels.append(p)
                    configs[cfg].append(pid)
            data = {
                "fixture": fx, "note": (r1.get("note") or "") + " Gold questions answered are marked.",
                "engine": engine, "server_version": snap.get("server_version", ""), "database": database,
                "snapshot": r1.get("snapshot", ""), "configs": configs,
                "names": {"O": "round 1 · B1, its tuned rules", "A": "round 1c · the schema alone", "B1": "round 1c · with statistics"},
                "pool": [p["id"] for p in panels], "panels": panels,
            }
            embedded = json.dumps(data, ensure_ascii=False).replace("</", "<\\/")
            (out / f"{fx}.html").write_text(page.replace("/*DATA*/null", embedded), encoding="utf-8")
            errors = [p["title"] for p in panels if p.get("error")]
            summary[fx] = {"gold in five": hits, "errors": errors,
                           "ms": [round(p["client_ms"]) for p in panels if p["id"].startswith("v2-")]}
            print(fx, hits, "errors:", errors)
    (out / "summary.json").write_text(json.dumps(summary, indent=1, ensure_ascii=False), encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), [Path(p) for p in sys.argv[4:]])
