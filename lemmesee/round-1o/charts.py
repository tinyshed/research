"""Round 1o's product charts, the three the owner and the owner's bot asked
for that no question of round 1c's generator draws: the share of a day's
rows that failed, the money of the last week by its kind, and the 95th
percentile of a delay a day. Each reads the database first where it must —
a status's failed values, a kind's few values — and keeps to the last days
of its table's own time, as round 1m's screen does on a living database.

The screen of round 1o is round 1m's with these in the places they fit:
its pulse and its money over a time, then health as the failed share a
day, speed as the 95th percentile a day, and the week's money by kind; a
place with no chart of its own keeps round 1m's choice.

    python -I charts.py bench <bench dir> <round 1l learn dir>
    python -I charts.py live <see dir> <fixture> <ssh target> <container> <round 1l learn dir>

bench counts the stories round 1o's screen shows on round 1m's bench beside
round 1m's screen (round 1m's counting, a chart reading what was planted);
live adds the screen to the fixture's pool as "five: the screen, round 1o".
"""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
for d in ("round-1c", "round-1k", "round-1m", "round-1n"):
    sys.path.insert(0, str(ROOT / d))
import kpi  # noqa: E402
import learn  # noqa: E402
import rank  # noqa: E402
import roles  # noqa: E402
import see  # noqa: E402
from aq import generate, has, identity, load  # noqa: E402

DAYS, WEEK = 28, 7
KINDISH = {"kind", "type", "category", "channel", "method", "plan", "tier", "source", "provider", "model", "class",
           "mode", "segment", "group", "product", "service"}
STARTISH = {"start", "started", "begin", "began", "created", "requested", "submitted", "opened", "sent", "placed"}
ENDISH = {"end", "ended", "completed", "finished", "closed", "resolved", "delivered", "returned", "done", "answered",
          "approved", "shipped", "paid"}


@dataclass
class Chart:
    title: str
    sql: str
    refs: set = field(default_factory=set)   # (table, column) it reads
    timed: bool = True
    template: str = "T3"
    unit: str | None = None


def last(engine, E, tb, col, days) -> str:
    since = f"INTERVAL {days} DAY" if engine == "mysql" else f"INTERVAL '{days} days'"
    return f"b.{E(col.name)} >= (SELECT MAX(m.{E(col.name)}) FROM {E(tb.name)} m) - {since}"


def day(engine, ref) -> str:
    return f"DATE({ref})" if engine == "mysql" else f"date_trunc('day', {ref})::date"


def busiest(schema):
    return sorted((t for t in schema.tables if t.timed and not t.boilerplate), key=lambda t: -(t.rows or 0))


def when_of(tb):
    return kpi.created(tb) or next((c for c in tb.columns if c.time == "event"), None)


def failed_share(schema, engine, db, run, E, q=None):
    """The failed share a day of the table and status round 1m's screen split
    by, when q is that split; else of the busiest table with failed values."""
    if q is not None and q.by is not None and q.by.c >= 0 and q.by.t == q.base and q.time is not None:
        pairs = [(schema.tables[q.base], schema.tables[q.base].columns[q.by.c], schema.tables[q.time.t].columns[q.time.c])]
    else:
        pairs = [(tb, st, when_of(tb)) for tb in busiest(schema) for st in kpi.statuses(tb) if when_of(tb) is not None]
    for tb, st, when in pairs:
        vals = kpi.probe(run, engine, db, schema, tb, st, E)
        bad = [v for v in vals if v.lower() in kpi.FAILED]
        if bad and len(bad) < len(vals):
            cond = f"b.{E(st.name)} IN (" + ", ".join("'" + v.replace("'", "''") + "'" for v in bad) + ")"
            live = kpi.live_rows(schema, tb, E, engine)
            sql = (f"SELECT {day(engine, f'b.{E(when.name)}')} AS t, AVG(CASE WHEN {cond} THEN 1.0 ELSE 0 END) AS v "
                   f"FROM {E(tb.name)} b WHERE " + " AND ".join(live + [last(engine, E, tb, when, DAYS)]) +
                   " GROUP BY 1 ORDER BY 1")
            return Chart(f"share of {tb.name} {'/'.join(bad[:3])} a day", sql, {(tb.name, st.name), (tb.name, when.name)},
                         unit="share")
    return None


def money_by_kind(schema, engine, E):
    for tb in busiest(schema):
        money = next((c for c in tb.columns if c.measure == "money" and c.fk is None), None)
        when = when_of(tb)
        if money is None or when is None:
            continue
        kinds = [c for c in tb.columns if c.fk is None and not c.pk and c.fam not in ("int", "decimal", "float")
                 and has(c.tk, KINDISH) and (c.distinct is None or 2 <= abs(c.distinct) <= 30 or -1 < c.distinct < 0)]
        if not kinds:
            continue
        k = kinds[0]
        live = kpi.live_rows(schema, tb, E, engine)
        sql = (f"SELECT b.{E(k.name)} AS g, SUM(b.{E(money.name)}) AS v FROM {E(tb.name)} b WHERE "
               + " AND ".join(live + [last(engine, E, tb, when, WEEK)]) + " GROUP BY 1 ORDER BY 2 DESC LIMIT 12")
        return Chart(f"sum of {tb.name}.{money.name} by {k.name}, the last week", sql,
                     {(tb.name, money.name), (tb.name, k.name)}, timed=False, template="T4")
    return None


def p95_delay(schema, engine, E, q=None):
    """The 95th percentile a day of the delay round 1m's screen averaged, when
    q is that question; else of the busiest table with a start and an end."""
    if engine == "mysql":
        return None  # no percentile there; round 1m's average stays
    if q is not None and q.measure.kind == "gap":
        (t0, c0), (t1, c1) = q.measure.cols[:2]
        candidates = [(schema.tables[t0], schema.tables[t0].columns[c0], schema.tables[t1].columns[c1])]
    else:
        candidates = []
        for tb in busiest(schema):
            times = [c for c in tb.columns if c.fam in ("timestamp", "date") and c.time in ("created", "event", None)]
            start = next((c for c in times if has(c.tk, STARTISH)), None)
            end = next((c for c in times if has(c.tk, ENDISH) and c is not start), None)
            if start is not None and end is not None:
                candidates.append((tb, start, end))
    for tb, start, end in candidates:
        live = kpi.live_rows(schema, tb, E, engine)
        x, y = f"b.{E(start.name)}", f"b.{E(end.name)}"
        sql = (f"SELECT {day(engine, x)} AS t, percentile_cont(0.95) WITHIN GROUP (ORDER BY EXTRACT(EPOCH FROM ({y}::timestamp - {x}::timestamp))) AS v "
               f"FROM {E(tb.name)} b WHERE " + " AND ".join(live + [f"{y} IS NOT NULL", f"{y} >= {x}", last(engine, E, tb, start, DAYS)]) +
               " GROUP BY 1 ORDER BY 1")
        return Chart(f"95th percentile of seconds from {tb.name}.{start.name} to {end.name} a day", sql,
                     {(tb.name, start.name), (tb.name, end.name)}, unit="seconds")
    return None


def screen(schema, engine, db, run, model) -> list:
    """Round 1o's screen: round 1m's questions where no chart of round 1o fits."""
    E = kpi.quote(engine)
    qs = [q for q in generate(schema) if rank.veto(schema, q) is None]
    row = {"features": [rank.features(schema, q) for q in qs], "slots": [rank.slots(schema, q) for q in qs]}
    X, _, _ = learn.matrix_free(row, model.feature_name(), None)
    s = model.predict(X) + np.array([rank.stats_points(schema, q) for q in qs])
    ids = [identity(schema, q) for q in qs]
    mw = roles.money_words()
    base = [qs[i] for i in roles.screen(schema, qs, ids, s, row["slots"], mw)]
    round_1m = list(base)
    role_of = {id(q): roles.timed_role(identity(schema, q), rank.slots(schema, q), mw) for q in base}
    # the pulse split by the kind of where it happened — a chat's kind, a channel, a shop — or else its own
    # kind: how many messages a day, how many of them in private (the owner, 30 October)
    score = {id(q): float(v) for q, v in zip(qs, s)}
    for k, q in enumerate(base):
        if role_of[id(q)] != "pulse" or q.time is None:
            continue
        splits = [x for x in qs if x.base == q.base and x.time == q.time and x.shape == "split" and x.by is not None
                  and x.by.c >= 0 and x.measure.kind == "rows"
                  and has(schema.tables[x.by.t].columns[x.by.c].tk, KINDISH)]
        if splits:
            best = max(splits, key=lambda x: (x.by.t != q.base, score[id(x)]))
            role_of[id(best)] = "pulse"
            base[k] = best
        break
    # health and speed follow round 1m's own choice of table and columns; the week's money by kind is new
    kinds = money_by_kind(schema, engine, E)
    health = speed = None
    out = []
    for q in base:
        r = role_of[id(q)]
        if r == "health" and (h := failed_share(schema, engine, db, run, E, q)) is not None:
            out.append(h)
        elif r == "speed" and (v := p95_delay(schema, engine, E, q)) is not None:
            out.append(v)
        elif r == "people" and kinds is not None:
            out.append(kinds)
            kinds = None
        else:
            out.append(q)
    # a chart with no place of its own takes the last place that is no pulse, money, health or speed
    for extra in (health, speed, kinds):
        if extra is None:
            continue
        for k in range(len(out) - 1, -1, -1):
            o = out[k]
            if not isinstance(o, Chart) and role_of.get(id(o)) not in ("pulse", "money", "health", "speed"):
                out[k] = extra
                break
    return out, round_1m


def refs_of(schema, item) -> tuple[set, bool]:
    if isinstance(item, Chart):
        return item.refs, item.timed
    sys.path.insert(0, str(ROOT / "round-1m"))
    import bench
    return bench.refs(schema, item), item.time is not None


def shown(schema, spec, five) -> tuple[int, int, int]:
    found = over = 0
    for st in spec.get("stories", []):
        ch = st.get("change") or {}
        hits = []
        for item in five:
            r, timed = refs_of(schema, item)
            if ch.get("column"):
                ok = (st.get("table"), ch["column"]) in r
            elif "rate" in ch:
                ok = timed and any(t == st.get("table") for t, _ in r)
            else:
                ok = bool(r & {(e.get("table"), e.get("column")) for e in st.get("seen_in", [])})
            if ok:
                hits.append(timed)
        found += bool(hits)
        over += any(hits)
    return len(spec.get("stories", [])), found, over


def bench_count(bench: Path, learn_new: Path) -> None:
    import lightgbm as lgb
    model = lgb.Booster(model_file=str(learn_new / "luna-and-deepseek" / "ranker.txt"))
    tot = {"round 1m's screen": [0, 0, 0], "round 1o's screen": [0, 0, 0]}
    per = {}
    for spec_p in sorted(bench.glob("m*/spec.json")):
        key = spec_p.parent.name
        snap = json.loads((ROOT / "corpus" / "see" / f"story{key}-postgres" / "snapshot.json").read_text(encoding="utf-8"))
        schema = load(snap, True)
        spec = json.loads(spec_p.read_text(encoding="utf-8"))
        new, old = screen(schema, snap["engine"], snap["database"], see.run_sql, model)
        per[key] = {}
        for name, five in (("round 1m's screen", old), ("round 1o's screen", new)):
            n, f, o = shown(schema, spec, five)
            tot[name] = [a + b for a, b in zip(tot[name], (n, f, o))]
            per[key][name] = {"found": f, "shown": o, "five": [x.title if isinstance(x, Chart) else x.title(schema) for x in five]}
        print(key, {k: f"{v['shown']}/{n}" for k, v in per[key].items()}, flush=True)
    (bench / "count-1o.json").write_text(json.dumps({"total": tot, "per database": per}, indent=1, ensure_ascii=False),
                                         encoding="utf-8")
    for name, (n, f, o) in tot.items():
        print(f"{name:20} found {f}/{n}  shown over time {o}/{n}")


def panel_of(schema, item, engine, db, run, pid) -> dict:
    sys.path.insert(0, str(ROOT / "round-1m"))
    import screen_live
    if not isinstance(item, Chart):
        return screen_live.panel(schema, item, engine, db, pid)
    rows, err, ms = run(engine, db, item.sql)
    rows = [[(r[0] or "")[:10]] + r[1:] if item.timed else r for r in rows]
    if item.unit == "share":
        rows = [r[:-1] + [None if r[-1] in (None, "") else f"{100 * float(r[-1]):.1f}"] for r in rows]
    p = {"id": pid, "template": item.template, "aggregate": "avg", "chart": "", "title": item.title, "sql": item.sql,
         "bucket": "day" if item.timed else None, "columns": [], "rows": rows[:500], "error": err, "full_scan": None,
         "cost": None, "client_ms": ms, "server": {}, "reasons": [], "flags": [], "abstained": None, "top": {}}
    if item.unit == "seconds":
        # in the unit its values need, by their median
        vals = [abs(float(r[-1])) for r in p["rows"] if r and r[-1] not in (None, "")]
        med = float(np.median(vals)) if vals else 0.0
        name, k = next(((n, k) for n, k in (("seconds", 1), ("minutes", 60), ("hours", 3600)) if med / k < 120), ("days", 86400))
        p["rows"] = [r[:-1] + [None if r[-1] in (None, "") else f"{float(r[-1]) / k:.2f}"] for r in p["rows"]]
        p["title"] = p["title"].replace("of seconds", f"of {name}")
    return p


def live(see_dir: Path, fixture: str, ssh: str, container: str, learn_new: Path) -> None:
    import lightgbm as lgb
    import live as live1k
    snap = json.loads((see_dir / fixture / "snapshot.json").read_text(encoding="utf-8"))
    run = live1k.remote(ssh, container, snap["role"], snap["database"])
    see.run_sql = run
    schema = load(snap, True)
    model = lgb.Booster(model_file=str(learn_new / "luna-and-deepseek" / "ranker.txt"))
    five, _ = screen(schema, snap["engine"], snap["database"], run, model)
    path = see_dir / "look" / fixture / "pool.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    tag = "five: the screen, round 1o"
    data["panels"] = [p for p in data["panels"] if tag not in p["in"]]
    for k, item in enumerate(five):
        p = panel_of(schema, item, snap["engine"], snap["database"], run, f"{fixture}:screen1o-{k}")
        p["in"], p["result"] = [tag], {}
        data["panels"].append(p)
        print(f"  {k + 1}. {len(p['rows'])} rows" + (f", not run: {p['error'][:80]}" if p.get("error") else ""), flush=True)
    path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
    print(fixture, "round 1o's screen added")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, *a = sys.argv[1:]
    if cmd == "bench":
        bench_count(Path(a[0]), Path(a[1]))
    elif cmd == "live":
        live(Path(a[0]), a[1], a[2], a[3], Path(a[4]))
