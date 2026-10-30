"""The screen document the prototype draws, until its own core is ported:
lemmesee's round 1o made in Python — the row of numbers (round 1n's kpi.py)
and the screen of five (round 1o's charts.py over round 1l's ranker of Luna
and DeepSeek) — with, for every role, the next questions in the ranker's
order, run too, so that dropping a panel or adding one needs no database.

    python -I export.py <see dir> <fixture> <out.json> [<ssh target> <container>]

<see dir>/<fixture> holds round 1's snapshot.json; with a target, every
statement runs on the database's own host in round 1k's read-only session.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
LEMMESEE = HERE.parent.parent / "lemmesee"
for d in ("round-1c", "round-1k", "round-1m", "round-1n", "round-1o"):
    sys.path.insert(0, str(LEMMESEE / d))

import numpy as np  # noqa: E402

import charts  # noqa: E402
import kpi  # noqa: E402
import learn  # noqa: E402
import rank  # noqa: E402
import roles  # noqa: E402
import see  # noqa: E402
from aq import generate, identity, load  # noqa: E402

SPARE = 4  # questions a role keeps in reserve


def role_of(item, schema, mw) -> str:
    if isinstance(item, charts.Chart):
        return {"share": "health", "seconds": "speed"}.get(item.unit, "money" if item.template == "T4" else "other")
    return roles.timed_role(identity(schema, item), rank.slots(schema, item), mw) or roles.role(
        identity(schema, item), rank.slots(schema, item), mw)


def main(see_dir: Path, fixture: str, out: Path, ssh: str | None = None, container: str | None = None) -> None:
    import lightgbm as lgb
    snap = json.loads((see_dir / fixture / "snapshot.json").read_text(encoding="utf-8"))
    if ssh:
        import live
        run = live.remote(ssh, container, snap["role"], snap["database"])
    else:
        run = see.run_sql
    see.run_sql = run
    schema = load(snap, True)
    model = lgb.Booster(model_file=str(LEMMESEE / "corpus" / "round-1l" / "learn" / "luna-and-deepseek" / "ranker.txt"))
    mw = roles.money_words()
    numbers = kpi.row(load(snap, True), snap["engine"], snap["database"], run)
    by_title = {t.title: t for t in numbers}
    five, _ = charts.screen(schema, snap["engine"], snap["database"], run, model)
    # the spares: the ranker's order, the best few of each role that are not on the screen
    qs = [q for q in generate(schema) if rank.veto(schema, q) is None]
    row = {"features": [rank.features(schema, q) for q in qs], "slots": [rank.slots(schema, q) for q in qs]}
    X, _, _ = learn.matrix_free(row, model.feature_name(), None)
    s = model.predict(X) + np.array([rank.stats_points(schema, q) for q in qs])
    on_screen = {x.title if isinstance(x, charts.Chart) else x.title(schema) for x in five}
    spares, seen = {}, set()
    for i in np.argsort(-s, kind="stable"):
        q = qs[int(i)]
        ident = identity(schema, q)
        r = roles.timed_role(ident, row["slots"][int(i)], mw) or roles.role(ident, row["slots"][int(i)], mw)
        t = q.title(schema)
        if t in on_screen or ident in seen or len(spares.get(r, [])) >= SPARE:
            continue
        seen.add(ident)
        spares.setdefault(r, []).append(q)
    panels = []
    for k, item in enumerate(five):
        p = charts.panel_of(schema, item, snap["engine"], snap["database"], run, f"s{k}")
        panels.append({"key": p["title"], "title": p["title"], "role": role_of(item, schema, mw), "template": p["template"],
                       "bucket": p["bucket"], "rows": p["rows"][:200], "error": p.get("error")})
    reserve = {}
    for r, items in spares.items():
        for k, q in enumerate(items):
            p = charts.panel_of(schema, q, snap["engine"], snap["database"], run, f"{r}{k}")
            reserve.setdefault(r, []).append({"key": p["title"], "title": p["title"], "role": r, "template": p["template"],
                                              "bucket": p["bucket"], "rows": p["rows"][:200], "error": p.get("error")})
    doc = {"store": {"name": fixture, "engine": snap["engine"], "database": snap["database"],
                     "about": json.loads((see_dir / fixture / "runs.json").read_text(encoding="utf-8")).get("note", "")
                     if (see_dir / fixture / "runs.json").exists() else ""},
           "numbers": [{"key": t.title, "title": t.title, "shown": kpi.shown(t, by_title), "kind": t.kind} for t in numbers],
           "panels": panels, "spares": reserve}
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(doc, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"{out}: {len(doc['numbers'])} numbers, {len(panels)} panels, {sum(len(v) for v in reserve.values())} spares")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    a = sys.argv[1:]
    main(Path(a[0]), a[1], Path(a[2]), *(a[3:5] if len(a) >= 5 else ()))
