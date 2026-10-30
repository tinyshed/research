"""Round 1n: numbers first. A product's screen opens with the figures it is
judged by — how many use it, how many did the one thing it is for, how many
came back, the shares between them, how many are new this week and this
month, what it earned, how much failed — and every round before this one
made charts alone: round 1's templates had no number, its teachers were told
a question is "drawn as one chart", its judges that "a single bar … is not
worth a chart", and its rules abstain where a status says which rows count.

From the schema alone: who uses the product is the table the most others
point at; the one thing it is for is the most pointed-at table that points at
them. Their numbers follow, as one statement each; a status's values are
read from the database first, and its done values, by their words, filter
the totals, its failed values make the share of failures.

    python -I kpi.py row <see dir> <fixture> [<ssh target> <container>]

runs the row on the fixture (on its own host, read only, when a target is
given, as round 1k's live.py) and writes <see dir>/look/<fixture>/kpi.json.
"""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT / "round-1c"))
sys.path.insert(0, str(ROOT / "round-1k"))
import see  # noqa: E402
from aq import alive, has, load  # noqa: E402

DONE = {"published", "complete", "completed", "done", "paid", "success", "successful", "succeeded", "active",
        "approved", "confirmed", "delivered", "closed", "finished", "applied", "committed", "fulfilled",
        "settled", "accepted", "resolved", "shipped", "sent", "ok"}
FAILED = {"failed", "failure", "error", "errored", "cancelled", "canceled", "rejected", "refunded", "declined",
          "expired", "timeout", "timedout", "lost", "aborted", "bounced", "retracted", "denied", "void", "voided"}
STATUSY = {"status", "state", "stage", "result", "outcome", "resolution"}
REPLACED = {"merged", "replaced", "duplicate", "superseded"}
WINDOWS = (7, 30)


@dataclass
class Tile:
    title: str
    sql: str | None = None             # one value
    of: tuple[str, str] | None = None  # a share: the titles of two tiles, top over bottom
    value: float | None = None
    error: str | None = None
    kind: str = "number"
    parts: list[str] = field(default_factory=list)


def quote(engine: str):
    return (lambda n: f"`{n}`") if engine == "mysql" else (lambda n: '"' + n.replace('"', '""') + '"')


def live_rows(schema, tb, E, engine: str) -> list[str]:
    """A table's live rows: round 1c's soft deletion, and a row merged into
    another or replaced by it, which a person no longer is."""
    out = []
    for c, how in alive(tb):
        ref = f"b.{E(tb.columns[c].name)}"
        out.append(f"{ref} IS NULL" if how == "null" else
                   f"COALESCE({ref}, 0) = 0" if engine == "mysql" else f"NOT COALESCE({ref}::int::boolean, false)")
    for c, p in tb.out:
        col = tb.columns[c]
        if p == tb.t and has(col.tk, REPLACED):
            out.append(f"b.{E(col.name)} IS NULL")
    return out


def since(engine: str, days: int) -> str:
    return f"NOW() - INTERVAL {days} DAY" if engine == "mysql" else f"NOW() - INTERVAL '{days} days'"


def created(tb):
    return next((c for c in tb.columns if c.time == "created"), None)


def statuses(tb):
    return [c for c in tb.columns if c.fk is None and not c.pk and c.fam not in ("int", "decimal", "float")
            and c.category in ("typed", "named", None) and has(c.tk, STATUSY)]


PEOPLE = {"user", "account", "member", "customer", "client", "person", "people", "player", "student", "author",
          "owner", "subscriber", "patient", "employee", "participant", "guest", "contact", "profile", "buyer",
          "seller", "driver", "rider", "tenant", "learner", "teacher", "candidate", "applicant", "visitor"}


def people(tb) -> bool:
    return tb.person or has(tb.tk, PEOPLE) or any(has([w.rstrip("s")], PEOPLE) for w in tb.tk)


def actor_and_object(schema):
    """Who uses the product: the table of people the most others point at
    (else any table of ten rows or more). The one thing it is for: the most
    pointed-at table with a time of its own that points at them by one key
    and is not itself people."""
    tabs = [tb for tb in schema.tables if not tb.boilerplate and tb.role not in ("lookup", "link")]
    pointing = {tb.t: {child for child, _ in tb.referenced_by if child != tb.t} for tb in schema.tables}
    rank = lambda tb: (len(pointing[tb.t]), tb.rows or 0)
    folk = [tb for tb in tabs if people(tb) and pointing[tb.t]]
    pool = folk or [tb for tb in tabs if (tb.rows is None or tb.rows >= 10) and pointing[tb.t]]
    actor = max(pool, key=rank, default=None)
    if actor is None:
        return None, None, None
    best = None
    for tb in tabs:
        if tb.t == actor.t or people(tb) or not tb.timed:
            continue
        via = [c for c, p in tb.out if p == actor.t]
        if len(via) > 1:
            # a row two people touch: the one who made it, else the first
            maker = [c for c in via if has(tb.columns[c].tk, {"owner", "creator", "created", "author", "user", "customer",
                                                               "client", "submitted", "by", "from", "member"} | set(actor.tk))]
            via = maker[:1] or via[:1]
        if len(via) == 1:
            key = (len(pointing[tb.t]), tb.rows or 0)
            if best is None or key > best[0]:
                best = (key, tb, via[0])
    if best is None:
        return actor, None, None
    return actor, best[1], best[2]


def probe(run, engine, database, schema, tb, col, E) -> dict[str, float]:
    rows, err, _ = run(engine, database, f"SELECT b.{E(col.name)}, COUNT(*) FROM {E(tb.name)} b GROUP BY 1 ORDER BY 2 DESC LIMIT 30")
    return {} if err else {str(r[0]): float(r[1]) for r in rows if r and r[0] is not None}


def row(schema, engine: str, database: str, run) -> list[Tile]:
    E = quote(engine)
    actor, obj, via = actor_and_object(schema)
    tiles: list[Tile] = []
    if actor is None:
        return tiles
    A = E(actor.name)
    a_live = live_rows(schema, actor, E, engine)
    where = lambda conds: (" WHERE " + " AND ".join(conds)) if conds else ""
    total_a = f"{actor.name} in all"
    tiles.append(Tile(total_a, f"SELECT COUNT(*) FROM {A} b{where(a_live)}"))
    if obj is not None:
        O = E(obj.name)
        o_live = live_rows(schema, obj, E, engine)
        done = []
        for st in statuses(obj):
            vals = probe(run, engine, database, schema, obj, st, E)
            hit = [v for v in vals if v.lower() in DONE]
            if hit and len(hit) < len(vals):
                done = [f"b.{E(st.name)} IN (" + ", ".join("'" + v.replace("'", "''") + "'" for v in hit) + ")"]
                break
        f = E(obj.columns[via].name)
        label = f"{obj.name}" + (" done" if done else "")
        tiles.append(Tile(f"{label} in all", f"SELECT COUNT(*) FROM {O} b{where(o_live + done)}"))
        # who did the thing the product is for did it to the end: a draft is not a split made
        actors = f"{actor.name} with {label}"
        tiles.append(Tile(actors, f"SELECT COUNT(DISTINCT b.{f}) FROM {O} b{where(o_live + done)}"))
        again = f"{actor.name} with two {label} or more"
        tiles.append(Tile(again, f"SELECT COUNT(*) FROM (SELECT b.{f} FROM {O} b{where(o_live + done + [f'b.{f} IS NOT NULL'])} "
                                 f"GROUP BY b.{f} HAVING COUNT(*) >= 2) x"))
        tiles.append(Tile(f"share of {actor.name} with {label}", of=(actors, total_a), kind="share"))
        tiles.append(Tile(f"share of them with two or more", of=(again, actors), kind="share"))
        tiles.append(Tile(f"{actor.name} → with {label} → with two or more", kind="funnel", parts=[total_a, actors, again]))
    made = created(actor)
    if made is not None:
        for d in WINDOWS:
            tiles.append(Tile(f"new {actor.name}, {d} days",
                              f"SELECT COUNT(*) FROM {A} b{where(a_live + [f'b.{E(made.name)} >= {since(engine, d)}'])}"))
    # what it earned: the busiest timed table with money, its done rows, the last month
    for tb in sorted((t for t in schema.tables if t.timed and not t.boilerplate), key=lambda t: -(t.rows or 0)):
        money = next((c for c in tb.columns if c.measure == "money" and c.fk is None), None)
        when = created(tb) or next((c for c in tb.columns if c.time == "event"), None)
        if money is None or when is None:
            continue
        done = []
        for st in statuses(tb):
            vals = probe(run, engine, database, schema, tb, st, E)
            hit = [v for v in vals if v.lower() in DONE]
            if hit and len(hit) < len(vals):
                done = [f"b.{E(st.name)} IN (" + ", ".join("'" + v.replace("'", "''") + "'" for v in hit) + ")"]
                break
        tiles.append(Tile(f"{tb.name}.{money.name}, 30 days",
                          f"SELECT SUM(b.{E(money.name)}) FROM {E(tb.name)} b"
                          f"{where(live_rows(schema, tb, E, engine) + done + [f'b.{E(when.name)} >= {since(engine, 30)}'])}"))
        break
    # what failed: the busiest timed table whose status has failed values, the last week
    for tb in sorted((t for t in schema.tables if t.timed and not t.boilerplate), key=lambda t: -(t.rows or 0)):
        when = created(tb) or next((c for c in tb.columns if c.time == "event"), None)
        found = None
        for st in statuses(tb):
            vals = probe(run, engine, database, schema, tb, st, E)
            bad = [v for v in vals if v.lower() in FAILED]
            if bad and when is not None:
                found = (st, bad)
                break
        if found:
            st, bad = found
            cond = f"b.{E(st.name)} IN (" + ", ".join("'" + v.replace("'", "''") + "'" for v in bad) + ")"
            tiles.append(Tile(f"share of {tb.name} {'/'.join(bad[:3])}, 7 days",
                              f"SELECT AVG(CASE WHEN {cond} THEN 1.0 ELSE 0 END) FROM {E(tb.name)} b"
                              f"{where(live_rows(schema, tb, E, engine) + [f'b.{E(when.name)} >= {since(engine, 7)}'])}",
                              kind="share"))
            break
    by_title = {}
    for t in tiles:
        if t.sql:
            rows, err, _ = run(engine, database, t.sql)
            t.error = err
            t.value = float(rows[0][0]) if rows and rows[0] and rows[0][0] not in (None, "") else (None if err else 0.0)
        by_title[t.title] = t
    for t in tiles:
        if t.of:
            top, bottom = by_title[t.of[0]].value, by_title[t.of[1]].value
            t.value = top / bottom if top is not None and bottom else None
        if t.kind == "funnel":
            t.value = None
    return tiles


def shown(t: Tile, by_title: dict) -> str:
    if t.kind == "funnel":
        return " → ".join(f"{by_title[p].value:.0f}" if by_title[p].value is not None else "?" for p in t.parts)
    if t.value is None:
        return "—" if not t.error else f"not run: {t.error[:60]}"
    return f"{100 * t.value:.1f}%" if t.kind == "share" else f"{t.value:,.0f}" if abs(t.value) >= 10 else f"{t.value:.2f}"


def main(see_dir: Path, fixture: str, ssh: str | None = None, container: str | None = None) -> None:
    snap = json.loads((see_dir / fixture / "snapshot.json").read_text(encoding="utf-8"))
    if ssh:
        import live
        run = live.remote(ssh, container, snap["role"], snap["database"])
    else:
        run = see.run_sql
    schema = load(snap, True)
    tiles = row(schema, snap["engine"], snap["database"], run)
    by_title = {t.title: t for t in tiles}
    out = see_dir / "look" / fixture
    out.mkdir(parents=True, exist_ok=True)
    (out / "kpi.json").write_text(json.dumps([{"title": t.title, "kind": t.kind, "value": t.value, "shown": shown(t, by_title),
                                               "sql": t.sql, "error": t.error} for t in tiles], ensure_ascii=False, indent=1),
                                  encoding="utf-8")
    for t in tiles:
        print(f"  {t.title:60} {shown(t, by_title)}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, *a = sys.argv[1:]
    if cmd == "row":
        main(Path(a[0]), a[1], *(a[2:4] if len(a) >= 4 else ()))
