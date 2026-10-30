"""The weekly screen on a living database, for its owner to judge beside
round 1k's three fives: Luna and DeepSeek's ranker (round 1l) scores the
schema's questions, roles.py's screen chooses five, and each is run on the
database's own host through round 1k's read-only session. A panel over a
time shows its last 28 days by day, where the week's change is; a speed is
drawn in the unit its values need, not in days.

    python -I screen_live.py pool <see dir> <fixture> <ssh target> <container> <round 1l learn dir>

adds the screen's panels to <see dir>/look/<fixture>/pool.json, marked
"five: the screen", which round 1k's live.py page draws as a fourth five.
Nothing read leaves this machine, and no host, table or column is named here.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT / "round-1c"))
sys.path.insert(0, str(ROOT / "round-1k"))
sys.path.insert(0, str(HERE))
import learn  # noqa: E402
import live  # noqa: E402
import rank  # noqa: E402
import roles  # noqa: E402
import see  # noqa: E402
from aq import compile_sql, generate, identity, load  # noqa: E402

DAYS = 28
UNITS = (("seconds", 86400), ("minutes", 1440), ("hours", 24), ("days", 1))


def recent(sql: str, schema, q, engine: str) -> str:
    """The statement kept to the last DAYS days of the time it is drawn over."""
    tb = schema.tables[q.time.t]
    col = tb.columns[q.time.c].name
    E = (lambda n: f"`{n}`") if engine == "mysql" else (lambda n: '"' + n.replace('"', '""') + '"')
    m = re.search(r"(\w+\." + re.escape(E(col)) + r") IS NOT NULL", sql)
    if not m:
        return sql
    ref = m.group(1)
    since = (f"(SELECT MAX({E(col)}) FROM {E(tb.name)}) - INTERVAL {DAYS} DAY" if engine == "mysql"
             else f"(SELECT MAX({E(col)}) FROM {E(tb.name)}) - INTERVAL '{DAYS} days'")
    return sql.replace(m.group(0), f"{m.group(0)} AND {ref} >= {since}", 1)


def in_unit(p: dict) -> dict:
    """A speed's average days in seconds, minutes or hours, by its median."""
    vals = [float(r[-1]) for r in p["rows"] if r and r[-1] not in (None, "")]
    if not vals:
        return p
    med = float(np.median(np.abs(vals)))
    name, k = next(((n, k) for n, k in UNITS if med * k < 120), UNITS[-1])
    if k != 1:
        p["rows"] = [r[:-1] + [None if r[-1] in (None, "") else f"{float(r[-1]) * k:.2f}"] for r in p["rows"]]
        p["title"] = p["title"].replace("average days from", f"average {name} from")
    return p


def panel(schema, q, engine: str, database: str, pid: str) -> dict:
    if q.time is None or schema.tables[q.time.t].columns[q.time.c].fam not in ("timestamp", "date"):
        p = see.panel_of(schema, q, engine, database, pid)
    else:
        sql = recent(compile_sql(schema, q, engine, "day"), schema, q, engine)
        rows, err, ms = see.run_sql(engine, database, sql)
        rows = [[(r[0] or "")[:10]] + r[1:] for r in rows]
        agg = {"rows": "count", "per": "count", "flagged": "count", "late": "count", "parents": "count"}.get(
            q.measure.kind, q.measure.agg)
        p = {"id": pid, "template": see.template_of(q), "aggregate": agg, "chart": "",
             # the title as any five's would read, so the page stays blind; the axis shows the days
             "title": q.title(schema), "sql": sql, "bucket": "day", "columns": [],
             "rows": rows[:500], "error": err, "full_scan": None, "cost": None, "client_ms": ms, "server": {},
             "reasons": [], "flags": [], "abstained": None, "top": {}}
    if q.measure.kind == "gap":
        p = in_unit(p)
    return p


def pool(see_dir: Path, fixture: str, ssh: str, container: str, learn_dir: Path) -> None:
    import lightgbm as lgb
    snap = json.loads((see_dir / fixture / "snapshot.json").read_text(encoding="utf-8"))
    see.run_sql = live.remote(ssh, container, snap["role"], snap["database"])
    schema = load(snap, True)
    qs = [q for q in generate(schema) if rank.veto(schema, q) is None]
    row = {"features": [rank.features(schema, q) for q in qs], "slots": [rank.slots(schema, q) for q in qs]}
    model = lgb.Booster(model_file=str(learn_dir / "luna-and-deepseek" / "ranker.txt"))
    X, _, _ = learn.matrix_free(row, model.feature_name(), None)
    s = model.predict(X) + np.array([rank.stats_points(schema, q) for q in qs])
    ids = [identity(schema, q) for q in qs]
    five = roles.screen(schema, qs, ids, s, row["slots"], roles.money_words())
    path = see_dir / "look" / fixture / "pool.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    data["panels"] = [p for p in data["panels"] if "five: the screen" not in p["in"]]
    for k, i in enumerate(five):
        p = panel(schema, qs[i], snap["engine"], snap["database"], f"{fixture}:screen-{k}")
        p["in"] = ["five: the screen"]
        p["result"] = {}
        data["panels"].append(p)
        print(f"  {k + 1}. {len(p['rows'])} rows" + (f", not run: {p['error'][:80]}" if p.get("error") else ""), flush=True)
    path.write_text(json.dumps(data, ensure_ascii=False), encoding="utf-8")
    print(fixture, "screen added")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, *a = sys.argv[1:]
    if cmd == "pool":
        pool(Path(a[0]), a[1], a[2], a[3], Path(a[4]))
