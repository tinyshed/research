"""Round 1i: what the look costs the database. Each pool statement of rounds
1d–1f explained, run whole and run on samples of its base table, timed;
then how many panels the look needs, how well the plan's estimate knows the
price, and what a sample changes in the five.

    python -I cost.py run <corpus/see> <out> <look dir>... [--only=<fixture-engine>,...]
    python -I cost.py measure <corpus/see> <out> <look dir>...
    python -I cost.py scale <corpus/see> <out> <fixture-engine> <database>,<its larger copy>... <look dir>...

run writes <out>/runs/<fixture-engine>.json and keeps what it has; measure
writes <out>/cost.json; scale, PostgreSQL only, <out>/scale-<fixture-engine>.json.
"""

from __future__ import annotations

import json
import math
import random
import re
import sys
import time
from pathlib import Path

HERE = Path(__file__).parent
FRACTIONS = (0.10, 0.01)
LIMIT_MS = 60000
# below this many rows a sample saves nothing worth a changed answer
SAMPLE_FROM = 10000
CHUNKS = 20
SEED = 20261028
INTEGER = re.compile(r"^(tiny|small|medium|big)?int(eger)?\b", re.I)


def pools(dirs: list[Path]) -> dict[str, list[dict]]:
    out = {}
    for d in dirs:
        for f in d.glob("*/pool.json"):
            out[f.parent.name] = [p for p in json.loads(f.read_text(encoding="utf-8"))["panels"] if "round 1" not in p["in"]]
    return dict(sorted(out.items()))


def base_of(sql: str) -> str | None:
    m = re.search(r'FROM ([`"])([^`"]+)\1 b\b', sql)
    return m.group(2) if m else None


def table_of(snap: dict, name: str) -> dict | None:
    return next((t for t in snap["tables"] if t["name"] == name), None)


def rows_estimate(t: dict | None) -> float:
    v = ((t or {}).get("features", {}).get("rows_estimate") or {}).get("value")
    return float(v) if isinstance(v, (int, float)) and v >= 0 else math.nan


def text(v) -> str | None:
    """A value as psql and mysql print it, as the pools hold them."""
    if v is None:
        return None
    if isinstance(v, bool):
        return "t" if v else "f"
    return str(v)


class Db:
    def __init__(self, snap: dict):
        self.engine = "postgres" if snap["engine"].startswith("postgres") else "mysql"
        if self.engine == "postgres":
            import psycopg
            self.conn = psycopg.connect(host="127.0.0.1", port=55432, user="lemmesee", password="lemmesee",
                                        dbname=snap["database"], autocommit=True)
            self.conn.execute(f"SET statement_timeout = {LIMIT_MS}")
        else:
            import pymysql
            self.conn = pymysql.connect(host="127.0.0.1", port=33306, user="lemmesee", password="lemmesee",
                                        database=snap["database"], charset="utf8mb4", autocommit=True)
            with self.conn.cursor() as c:
                c.execute(f"SET SESSION max_execution_time = {LIMIT_MS}")

    def run(self, sql: str) -> tuple[list | None, float, str | None]:
        t0 = time.perf_counter()
        try:
            cur = self.conn.cursor()
            cur.execute(sql)
            rows = [[text(v) for v in r] for r in cur.fetchall()]
            cur.close()
            return rows, (time.perf_counter() - t0) * 1000, None
        except Exception as e:  # noqa: BLE001 — a timeout or a fault is a result here
            return None, (time.perf_counter() - t0) * 1000, str(e).splitlines()[0][:160]

    def explain(self, sql: str) -> dict:
        try:
            cur = self.conn.cursor()
            if self.engine == "postgres":
                cur.execute("EXPLAIN (FORMAT JSON) " + sql)
                plan = cur.fetchone()[0]
                plan = json.loads(plan) if isinstance(plan, str) else plan
                top = plan[0]["Plan"]
                return {"cost": float(top["Total Cost"]), "rows": float(top["Plan Rows"])}
            cur.execute("EXPLAIN FORMAT=JSON " + sql)
            q = json.loads(cur.fetchone()[0])["query_block"]
            return {"cost": float(q.get("cost_info", {}).get("query_cost", "nan")), "rows": math.nan}
        except Exception as e:  # noqa: BLE001
            return {"error": str(e).splitlines()[0][:160]}

    def sampled(self, sql: str, snap: dict, frac: float) -> tuple[str | None, str]:
        """The statement on a sample of its base table, and how it was taken."""
        name = base_of(sql)
        t = table_of(snap, name) if name else None
        if t is None:
            return None, "no base table"
        if self.engine == "postgres":
            pct = f"{frac * 100:g}"
            return re.sub(r'FROM "([^"]+)" b\b', rf'FROM "\1" b TABLESAMPLE SYSTEM ({pct}) REPEATABLE ({SEED})', sql,
                          count=1), "pages"
        pk = next((c["columns"] for c in t.get("constraints", []) if c["kind"] == "p"), None)
        col = next((c for c in t["columns"] if pk and c["name"] == pk[0]), None)
        if not col or not INTEGER.match(col.get("type_name", "")):
            return None, "no integer key"
        cur = self.conn.cursor()
        cur.execute(f"SELECT MIN(`{col['name']}`), MAX(`{col['name']}`) FROM `{name}`")
        lo, hi = cur.fetchone()
        if lo is None:
            return None, "empty"
        n = rows_estimate(t)
        m = max(1, math.ceil(frac * (n if n == n else 0) / CHUNKS))
        rng = random.Random(f"{SEED}:{name}:{frac}")
        starts = sorted(rng.randint(int(lo), int(hi)) for _ in range(CHUNKS))
        order = ", ".join(f"`{k}`" for k in pk)
        part = " UNION ALL ".join(f"(SELECT * FROM `{name}` WHERE `{col['name']}` >= {s} ORDER BY {order} LIMIT {m})"
                                  for s in starts)
        return re.sub(r"FROM `([^`]+)` b\b", lambda _: f"FROM ({part}) b", sql, count=1), "key runs"


def run(see_dir: Path, out: Path, dirs: list[Path], only: set[str] | None) -> None:
    (out / "runs").mkdir(parents=True, exist_ok=True)
    for fe, panels in pools(dirs).items():
        if only and fe not in only:
            continue
        dest = out / "runs" / f"{fe}.json"
        if dest.exists():
            continue
        snap = json.loads((see_dir / fe / "snapshot.json").read_text(encoding="utf-8"))
        db = Db(snap)
        t0 = time.time()
        got = []
        for p in panels:
            name = base_of(p["sql"])
            r = {"id": p["id"], "base": name, "base_rows": rows_estimate(table_of(snap, name) if name else None),
                 "plan": db.explain(p["sql"])}
            rows, ms, err = db.run(p["sql"])
            r["whole"] = {"ms": round(ms, 2), "error": err, "rows": len(rows) if rows is not None else None}
            for frac in FRACTIONS:
                key = f"{frac * 100:g}%"
                if not (r["base_rows"] >= SAMPLE_FROM):
                    continue
                sql, how = db.sampled(p["sql"], snap, frac)
                if sql is None:
                    r[key] = {"how": how}
                    continue
                rows, ms, err = db.run(sql)
                r[key] = {"how": how, "ms": round(ms, 2), "error": err, "rows": rows}
            got.append(r)
        db.conn.close()
        dest.write_text(json.dumps({"fixture": fe, "engine": db.engine, "panels": got}, ensure_ascii=False),
                        encoding="utf-8")
        whole = sum(r["whole"]["ms"] for r in got) / 1000
        print(f"{fe}: {len(got)} statements, {whole:.1f} s whole, {time.time() - t0:.0f} s in all", flush=True)


def scale(see_dir: Path, out: Path, fe: str, databases: list[str], dirs: list[Path], n: float = 1e5) -> None:
    """One fixture's statements on copies of its database at other sizes:
    a sample of a fixed number of rows, then 1 %, then whole, timed; and how
    often the sample's result has the whole one's points and leader."""
    panels = pools(dirs)[fe]
    snap = json.loads((see_dir / fe / "snapshot.json").read_text(encoding="utf-8"))
    report = {}
    for name in databases:
        db = Db(snap | {"database": name})
        cur = db.conn.cursor()
        got = []
        for p in panels:
            base = base_of(p["sql"])
            cur.execute("SELECT reltuples FROM pg_class WHERE relname = %s AND relkind = 'r'", (base,))
            rows_now = float(cur.fetchone()[0])
            r = {"id": p["id"], "rows": rows_now, "plan": db.explain(p["sql"])}
            fixed = min(1.0, n / rows_now) if rows_now >= n else None
            for key, frac in ((f"{n:g} rows", fixed), ("1%", 0.01 if rows_now >= SAMPLE_FROM else None), ("whole", None)):
                sql = db.sampled(p["sql"], snap, frac)[0] if frac else p["sql"]
                if key != "whole" and frac is None:
                    continue
                res, ms, err = db.run(sql)
                r[key] = {"ms": round(ms, 2), "error": err, "rows": scaled(p, res, frac) if frac else res}
            got.append(r)
        db.conn.close()
        same = {}
        for key in (f"{n:g} rows", "1%"):
            pairs = [(r[key]["rows"], r["whole"]["rows"], p) for r, p in zip(got, panels) if key in r]
            ok_n = [a is not None and b is not None and len(a) == len(b) for a, b, _ in pairs]
            ok_lead = [a is not None and b is not None and (p.get("bucket") is not None or not a or not b or a[0][0] == b[0][0])
                       for a, b, p in pairs]
            same[key] = {"sampled": len(pairs), "as many points": round(float(sum(ok_n) / max(1, len(pairs))), 2),
                         "the same leader": round(float(sum(ok_lead) / max(1, len(pairs))), 2)}
        secs = {key: [round(sum(r[key]["ms"] for r in got if key in r) / 1000, 2),
                      round(max((r[key]["ms"] for r in got if key in r), default=0) / 1000, 2)]
                for key in (f"{n:g} rows", "1%", "whole")}
        largest = max(r["rows"] for r in got)
        report[name] = {"largest table's rows": largest, "seconds: all, the slowest statement": secs,
                        "out of time whole": sum(1 for r in got if r["whole"]["error"]), "results": same}
        (out / f"scale-{name}.json").write_text(json.dumps(got, ensure_ascii=False), encoding="utf-8")
        print(name, json.dumps(report[name], ensure_ascii=False), flush=True)
    (out / f"scale-{fe}.json").write_text(json.dumps(report, indent=1, ensure_ascii=False), encoding="utf-8")


# ---- the measures

CONFIG = ("small", "numbers", "point")  # round 1g's baseline, whose five judged as a set is the best
TOP_K = (5, 8, 10, 15, 20, 30)


def scaled(p: dict, rows: list | None, frac: float) -> list | None:
    """A sample's counts and sums as the whole table's."""
    if rows is None or p.get("aggregate") not in ("count", "sum"):
        return rows
    out = []
    for r in rows:
        try:
            v = float(r[-1]) / frac
            out.append(r[:-1] + [str(int(round(v))) if p["aggregate"] == "count" else repr(v)])
        except (TypeError, ValueError):
            out.append(r)
    return out


def variant(p: dict, rows: list | None, error: str | None, dg) -> dict:
    q = dict(p)
    q["rows"], q["error"] = rows or [], error
    q["result"] = dg.look1d.result_features(q)
    q["more"] = dg.look1e.more(q)
    return q


def spearman(a, b) -> float:
    import numpy as np
    ra = np.argsort(np.argsort(a)).astype(float)
    rb = np.argsort(np.argsort(b)).astype(float)
    return float(np.corrcoef(ra, rb)[0, 1])


def measure(see_dir: Path, out: Path, dirs: list[Path]) -> None:
    import numpy as np
    sys.path.insert(0, str(HERE.parent / "round-1g"))
    import diagnose as dg
    import sets
    D = dg.load_all(dirs, see_dir)
    fes, ans = D["fes"], D["ans"]
    runs = {}
    for f in (out / "runs").glob("*.json"):
        d = json.loads(f.read_text(encoding="utf-8"))
        runs |= {r["id"]: r | {"engine": d["engine"]} for r in d["panels"]}
    fes = [fe for fe in fes if all(p["id"] in runs for p in D["mine"][fe])]

    # each panel as the look would read it: whole, and on each sample
    kinds = ["whole"] + [f"{x * 100:g}%" for x in FRACTIONS]
    seen = {k: {} for k in kinds}
    for fe in fes:
        for p in D["mine"][fe]:
            r = runs[p["id"]]
            seen["whole"][p["id"]] = p
            for frac, k in zip(FRACTIONS, kinds[1:]):
                s = r.get(k) or {}
                if "ms" in s:
                    seen[k][p["id"]] = variant(p, scaled(p, s["rows"], frac), s["error"], dg)

    # the look out of fold by family, as round 1g taught it, scoring every way
    scores = {k: {fe: np.zeros(len(D["mine"][fe])) for fe in fes} for k in kinds}
    fams = sorted(set(D["family"].values()))
    for draw in range(dg.DRAWS):
        order = list(fams)
        np.random.default_rng(dg.SEED + draw).shuffle(order)
        fold_of = {f: k % dg.FOLDS for k, f in enumerate(order)}
        for k in range(dg.FOLDS):
            test = [fe for fe in fes if fold_of[D["family"][fe]] == k]
            train = [fe for fe in D["fes"] if fold_of[D["family"][fe]] != k]
            model = dg.fit(D, train, *CONFIG, seed=dg.SEED + 100 * draw + k)
            for fe in test:
                for kind in kinds:
                    X = [dg.vector(seen[kind].get(p["id"], p), CONFIG[1]) for p in D["mine"][fe]]
                    scores[kind][fe] += model.predict(np.array(X, dtype=float)) / dg.DRAWS
        print(f"draw {draw + 1} done", flush=True)

    def ms_of(p, kind):
        r = runs[p["id"]]
        return (r.get(kind) or {}).get("ms", r["whole"]["ms"]) if kind != "whole" else r["whole"]["ms"]

    def policy(fe, use: dict[str, str | None], cut: float = math.inf) -> tuple[list[int], float]:
        """A five from scores where each panel is read as `use` says — whole,
        a sample, or not at all — and the database's time for it; a statement
        past `cut` milliseconds is stopped there and read as not run."""
        s, ms = [], 0.0
        for i, p in enumerate(D["mine"][fe]):
            kind = use[p["id"]]
            t = ms_of(p, kind) if kind else 0.0
            s.append(scores[kind][fe][i] if kind and t <= cut else -np.inf)
            ms += min(t, cut)
        return dg.look1e.Chooser(D["ids"][fe]).choose(np.array(s), sets.AXIS_ONCE), ms

    # a statement whose plan the database prices over this is left out: the
    # price that found nine in ten of round 1i's statements over a second
    priced = {"postgres": 406236.0, "mysql": 705734.0}

    systems: dict[str, dict] = {}
    for fe in fes:
        ps = D["mine"][fe]
        by_rank = sorted(range(len(ps)), key=lambda i: ps[i]["rank"])
        plans = {"every panel whole": ({p["id"]: "whole" for p in ps}, math.inf)}
        for k in TOP_K:
            top = {ps[i]["id"] for i in by_rank[:k]}
            plans[f"the best {k} by round 1c, whole"] = ({p["id"]: "whole" if p["id"] in top else None for p in ps},
                                                         math.inf)
        for kind in kinds[1:]:
            for floor in (1e5, 1e6):
                plans[f"tables over {floor:.0e} rows on a {kind} sample"] = ({
                    p["id"]: kind if runs[p["id"]]["base_rows"] >= floor and p["id"] in seen[kind] else "whole"
                    for p in ps}, math.inf)
        # together: the best ten, a large table sampled, and what stays dear
        # left out by its plan's price or stopped by a time limit
        top10 = {ps[i]["id"] for i in by_rank[:10]}
        for k_name, top in (("the best 10", top10), ("all 30", {p["id"] for p in ps})):
            sampled = {}
            for p in ps:
                r = runs[p["id"]]
                if p["id"] not in top:
                    sampled[p["id"]] = None
                elif r["base_rows"] >= 1e5 and p["id"] in seen["1%"]:
                    sampled[p["id"]] = "1%"
                else:
                    sampled[p["id"]] = "whole"
            plans[f"{k_name}, tables over 1e+05 rows on a 1% sample"] = (sampled, math.inf)
            for cut in (500.0, 1000.0, 2000.0):
                plans[f"{k_name}, tables over 1e+05 rows on a 1% sample, stopped at {cut / 1000:g} s"] = (sampled, cut)
            plans[f"{k_name}, tables over 1e+05 rows on a 1% sample, dear plans left out"] = ({
                pid: (None if kind == "whole" and runs[pid]["plan"].get("cost", 0) > priced[runs[pid]["engine"]] else kind)
                for pid, kind in sampled.items()}, math.inf)
        for name, (use, cut) in plans.items():
            systems.setdefault(name, {})[fe] = policy(fe, use, cut)

    result: dict = {"databases": len(fes), "counted": {j: len([fe for fe in D["covered"][j] if fe in fes])
                                                       for j in dg.COUNTERS}}
    base = systems["every panel whole"]
    rng = np.random.default_rng(SEED)
    for name, per in systems.items():
        row: dict = {}
        secs = np.array([per[fe][1] / 1000 for fe in fes])
        row["database seconds: median, 95th, most, all"] = [round(float(np.median(secs)), 2),
                                                            round(float(np.percentile(secs, 95)), 1),
                                                            round(float(secs.max()), 1), round(float(secs.sum()), 1)]
        row["the same five as whole, share of databases"] = round(float(np.mean(
            [sorted(per[fe][0]) == sorted(base[fe][0]) for fe in fes])), 2)
        for j in dg.COUNTERS:
            dbs = [fe for fe in D["covered"][j] if fe in fes]
            kept = {fe: sum(bool(ans[j][D["mine"][fe][i]["id"]]["keep"]) for i in per[fe][0]) for fe in dbs}
            row[f"{j}, kept of five"] = round(float(np.mean(list(kept.values()))), 2)
            if name != "every panel whole":
                d = np.array([kept[fe] - sum(bool(ans[j][D["mine"][fe][i]["id"]]["keep"]) for i in base[fe][0])
                              for fe in dbs], dtype=float)
                boots = [rng.choice(d, len(d)).mean() for _ in range(5000)]
                row[f"{j}, − whole"] = [round(float(d.mean()), 2)] + [round(float(np.percentile(boots, q)), 2)
                                                                      for q in (2.5, 97.5)]
        result[name] = row

    # the plan's estimate against the time, by engine
    est: dict = {}
    for eng in ("postgres", "mysql"):
        rs = [r for r in runs.values() if r["engine"] == eng and "cost" in r["plan"] and not r["whole"]["error"]
              and r["plan"]["cost"] == r["plan"]["cost"]]
        c = np.array([r["plan"]["cost"] for r in rs])
        t = np.array([r["whole"]["ms"] for r in rs])
        block = {"statements": len(rs), "Spearman": round(spearman(c, t), 3),
                 "ms: median, 95th, most": [round(float(np.median(t)), 1), round(float(np.percentile(t, 95)), 1),
                                            round(float(t.max()), 1)]}
        for limit in (100, 1000):
            slow = t > limit
            # the estimate's threshold that finds nine in ten of the slow
            if slow.sum():
                cut = float(np.percentile(c[slow], 10))
                flagged = c >= cut
                block[f"over {limit} ms"] = {"statements": int(slow.sum()), "cost threshold": round(cut, 1),
                                             "found": round(float((flagged & slow).sum() / slow.sum()), 2),
                                             "flagged that were slow": round(float((flagged & slow).sum() / flagged.sum()), 2),
                                             "flagged": int(flagged.sum())}
        est[eng] = block
    result["the plan's estimate"] = est
    timeouts = sum(1 for r in runs.values() if r["whole"]["error"] and "timeout" in r["whole"]["error"].lower()
                   or (r["whole"]["error"] or "").startswith("(3024"))
    result["statements whole: all, failed, out of time"] = [len(runs), sum(1 for r in runs.values() if r["whole"]["error"]),
                                                             timeouts]
    (out / "cost.json").write_text(json.dumps(result, indent=1, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(result, indent=1, ensure_ascii=False))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    only = next((set(a.split("=", 1)[1].split(",")) for a in sys.argv if a.startswith("--only=")), None)
    if args[0] == "run":
        run(Path(args[1]), Path(args[2]), [Path(x) for x in args[3:]], only)
    elif args[0] == "scale":
        scale(Path(args[1]), Path(args[2]), args[3], args[4].split(","), [Path(x) for x in args[5:]])
    else:
        measure(Path(args[1]), Path(args[2]), [Path(x) for x in args[3:]])
