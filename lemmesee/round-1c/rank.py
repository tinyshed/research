"""What orders round 1c's questions: the features a question shows from tier
S, and from tier N where it is read; a hand prior, round 1c's rules; the
statistics' vetoes; and the choice of five that do not repeat each other.
A learned ranker (learn.py) reads the same features.
"""

from __future__ import annotations

import math

from aq import Question, Schema, has, identity

ACTIVITY = {"order", "orders", "sale", "sales", "invoice", "invoices", "payment", "payments", "transaction", "transactions",
            "booking", "bookings", "reservation", "reservations", "event", "events", "log", "logs", "visit", "visits",
            "ticket", "tickets", "purchase", "purchases", "shipment", "shipments", "subscription", "subscriptions",
            "rental", "rentals", "match", "matches", "game", "games", "result", "results", "entry", "entries", "post",
            "posts", "comment", "comments", "review", "reviews", "message", "messages", "session", "sessions", "loan",
            "loans", "trip", "trips", "flight", "flights", "race", "races", "appointment", "appointments", "enrollment",
            "enrollments", "application", "applications", "submission", "submissions", "attempt", "attempts", "vote",
            "votes", "click", "clicks", "view", "views", "donation", "donations", "expense", "expenses", "deposit",
            "withdrawal", "transfer", "transfers", "line", "lines", "item", "items", "detail", "details", "batting",
            "pitching", "fielding", "salaries", "salary", "rating", "ratings", "answer", "answers", "question",
            "questions", "task", "tasks", "issue", "issues", "call", "calls", "delivery", "deliveries", "receipt"}
ENTITY = {"user", "users", "customer", "customers", "product", "products", "employee", "employees", "player", "players",
          "team", "teams", "member", "members", "account", "accounts", "client", "clients", "student", "students",
          "course", "courses", "book", "books", "movie", "movies", "store", "stores", "company", "companies", "supplier",
          "suppliers", "vendor", "vendors", "author", "authors", "driver", "drivers", "patient", "patients", "doctor"}
LOOKUPISH = {"type", "types", "status", "statuses", "category", "categories", "country", "countries", "role", "roles",
             "region", "regions", "language", "languages", "currency", "currencies", "genre", "genres", "state", "states"}
MONEYWORD = {"total", "amount", "revenue", "price", "sales", "income", "profit", "cost", "salary", "paid", "payment", "fare"}
FAMOUS_TIME = {"created", "order", "ordered", "date", "booked", "book", "invoice", "sale", "paid", "placed", "posted", "submitted", "start", "started"}

ROLES = ["fact", "event", "entity", "lookup", "link", "snapshot", "table"]
MKINDS = ["rows", "column", "product", "net", "parents", "gap", "late", "per"]
MROLES = ["money", "quantity", "ratio", "score", "number"]
BYKINDS = ["label", "person", "key", "geo", "bool", "named", "measured", "typed", "own_label"]
SHAPES = ["series", "breakdown", "top", "split", "distribution"]


def _log(x):
    return math.log1p(x) if x is not None else float("nan")


def _ranks(values: list[float]) -> list[float]:
    """0 for the largest, 1 for the next, ties sharing a place."""
    order = sorted(set(values), reverse=True)
    place = {v: i for i, v in enumerate(order)}
    return [float(place[v]) for v in values]


def context(schema: Schema) -> dict:
    """What a table is beside the others: a tree compares a question with a
    number, never with the other tables of its schema, so the comparison is
    given to it."""
    ctx = getattr(schema, "_context", None)
    if ctx is not None:
        return ctx
    tables = schema.tables
    ins = [float(len(t.referenced_by)) for t in tables]
    outs = [float(len(t.out)) for t in tables]
    ncols = [float(len(t.columns)) for t in tables]
    meas = [float(sum(1 for c in t.columns if c.measure in ("money", "quantity", "number"))) for t in tables]
    money = [float(sum(1 for c in t.columns if c.measure == "money")) for t in tables]
    times = [float(sum(1 for c in t.columns if c.time in ("created", "event", "int"))) for t in tables]
    ctx = {
        "in_rank": _ranks(ins), "out_rank": _ranks(outs), "ncols_rank": _ranks(ncols), "meas_rank": _ranks(meas),
        "money_rank": _ranks(money), "time_rank": _ranks(times),
        "in_share": [v / max(max(ins), 1.0) for v in ins], "out_share": [v / max(max(outs), 1.0) for v in outs],
        "ncols_share": [v / max(max(ncols), 1.0) for v in ncols], "meas_share": [v / max(max(meas), 1.0) for v in meas],
        "timed_tables": float(sum(1 for t in tables if t.timed)),
        "money_tables": float(sum(1 for m in money if m > 0)),
    }
    # each table's measures and categories in their order
    ctx["measure_order"] = {}
    ctx["category_order"] = {}
    for t in tables:
        ms = [c.c for c in t.columns if c.measure in ("money", "quantity", "number", "ratio", "score")]
        ms.sort(key=lambda c: (0 if t.columns[c].measure == "money" else 1 if t.columns[c].measure == "quantity" else 2, c))
        for i, c in enumerate(ms):
            ctx["measure_order"][(t.t, c)] = float(i)
        cs = [c.c for c in t.columns if c.category]
        for i, c in enumerate(cs):
            ctx["category_order"][(t.t, c)] = float(i)
    schema._context = ctx
    return ctx


def features(schema: Schema, q: Question) -> dict:
    tb = schema.tables[q.base]
    f: dict[str, float] = {}
    nan = float("nan")
    f["base_subject"] = 1.0 if tb.subject is not None else 0.0
    f["base_subject_rank"] = float(tb.subject) if tb.subject is not None else 9.0
    f["base_first_subject"] = 1.0 if tb.subject == 0 else 0.0
    f["base_detail"] = 1.0 if tb.detail_of is not None else 0.0
    f["base_detail_of_first"] = 1.0 if tb.detail_of is not None and schema.tables[tb.detail_of].subject == 0 else 0.0
    for r in ROLES:
        f[f"base_role_{r}"] = 1.0 if tb.role == r else 0.0
    f["base_timed"] = 1.0 if tb.timed else 0.0
    f["base_out"] = float(len(tb.out))
    f["base_in"] = float(len(tb.referenced_by))
    f["base_ncols"] = float(len(tb.columns))
    f["base_boilerplate"] = 1.0 if tb.boilerplate else 0.0
    f["base_activity_word"] = 1.0 if has(tb.tk, ACTIVITY) else 0.0
    f["base_entity_word"] = 1.0 if has(tb.tk, ENTITY) else 0.0
    f["base_lookup_word"] = 1.0 if has(tb.tk, LOOKUPISH) else 0.0
    f["base_person"] = 1.0 if tb.person else 0.0
    f["base_money_cols"] = float(sum(1 for c in tb.columns if c.measure == "money"))
    f["base_qty_cols"] = float(sum(1 for c in tb.columns if c.measure == "quantity"))
    ctx = context(schema)
    for k in ("in_rank", "out_rank", "ncols_rank", "meas_rank", "money_rank", "time_rank", "in_share", "out_share", "ncols_share", "meas_share"):
        f[f"base_{k}"] = ctx[k][q.base]
    f["schema_timed_tables"] = ctx["timed_tables"]
    f["schema_money_tables"] = ctx["money_tables"]
    f["schema_tables"] = float(len(schema.tables))
    f["schema_subjects"] = float(sum(1 for t in schema.tables if t.subject is not None))
    # tier N
    f["base_log_rows"] = _log(tb.rows)
    known = [t.rows for t in schema.tables if t.rows is not None]
    f["base_rows_share"] = (tb.rows / max(known)) if tb.rows is not None and known and max(known) > 0 else nan

    m = q.measure
    for k in MKINDS:
        f[f"m_{k}"] = 1.0 if m.kind == k else 0.0
    # a line's amount is one measure, with its discount or without
    f["m_product"] = 1.0 if m.kind in ("product", "net") else 0.0
    f["m_sum"] = 1.0 if m.agg == "sum" else 0.0
    f["m_avg"] = 1.0 if m.agg == "avg" else 0.0
    col = schema.tables[m.cols[0][0]].columns[m.cols[0][1]] if m.cols else None
    for r in MROLES:
        f[f"m_role_{r}"] = 1.0 if col is not None and m.kind == "column" and col.measure == r else 0.0
    f["m_moneyword"] = 1.0 if col is not None and has(col.tk, MONEYWORD) else 0.0
    f["m_named_total"] = 1.0 if col is not None and has(col.tk, {"total", "amount", "revenue"}) else 0.0
    f["m_null_frac"] = col.null_frac if col is not None and col.null_frac is not None else nan
    f["m_short_name"] = 1.0 if col is not None and len(col.name) <= 3 else 0.0
    f["m_order"] = ctx["measure_order"].get(m.cols[0], 9.0) if m.cols else -1.0
    f["m_position"] = (m.cols[0][1] / max(len(schema.tables[m.cols[0][0]].columns), 1)) if m.cols else -1.0

    d = q.time
    f["has_time"] = 1.0 if d else 0.0
    if d:
        tc = schema.tables[d.t].columns[d.c]
        f["time_created"] = 1.0 if d.kind == "created" else 0.0
        f["time_event"] = 1.0 if d.kind == "event" else 0.0
        f["time_int"] = 1.0 if d.kind == "int" else 0.0
        f["time_on_base"] = 1.0 if not d.path else 0.0
        f["time_path"] = float(len(d.path))
        f["time_famous"] = 1.0 if has(tc.tk, FAMOUS_TIME) else 0.0
        axes = [c for c in schema.tables[d.t].columns if c.time in ("created", "event", "int")]
        f["time_first_axis"] = 1.0 if axes and axes[0].c == d.c else 0.0
        f["time_axes"] = float(len(axes))
        f["time_null_frac"] = tc.null_frac if tc.null_frac is not None else nan
    else:
        for k in ("time_created", "time_event", "time_int", "time_on_base", "time_path", "time_famous", "time_first_axis", "time_axes"):
            f[k] = 0.0
        f["time_null_frac"] = nan

    g = q.by
    f["has_by"] = 1.0 if g else 0.0
    for k in BYKINDS:
        f[f"by_{k}"] = 1.0 if g and g.kind == k else 0.0
    if g:
        gt = schema.tables[g.t]
        gc = gt.columns[g.c] if g.c >= 0 else None
        f["by_path"] = float(len(g.path))
        f["by_on_base"] = 1.0 if not g.path else 0.0
        f["by_table_lookup"] = 1.0 if gt.role == "lookup" else 0.0
        f["by_table_entity"] = 1.0 if gt.role == "entity" else 0.0
        f["by_table_subject"] = 1.0 if gt.subject is not None else 0.0
        f["by_table_boilerplate"] = 1.0 if gt.boilerplate else 0.0
        f["by_log_distinct"] = _log(gc.distinct) if gc is not None and gc.distinct is not None else (_log(gt.rows) if gc is None else nan)
        f["by_null_frac"] = gc.null_frac if gc is not None and gc.null_frac is not None else nan
        f["by_top_freq"] = gc.top_freq if gc is not None and gc.top_freq is not None else nan
        f["by_status"] = 1.0 if gc is not None and has(gc.tk, {"status", "state", "type", "kind", "category"}) else 0.0
        f["by_country"] = 1.0 if gc is not None and has(gc.tk, {"country", "nationality"}) else 0.0
        f["by_order"] = ctx["category_order"].get((g.t, g.c), 9.0) if g.c >= 0 else -1.0
        f["by_table_in_rank"] = ctx["in_rank"][g.t]
        f["by_table_in_share"] = ctx["in_share"][g.t]
        f["by_table_ncols"] = float(len(gt.columns))
    else:
        for k in ("by_path", "by_on_base", "by_table_lookup", "by_table_entity", "by_table_subject", "by_table_boilerplate", "by_status", "by_country"):
            f[k] = 0.0
        f["by_order"] = f["by_table_in_rank"] = f["by_table_in_share"] = f["by_table_ncols"] = -1.0
        f["by_log_distinct"] = f["by_null_frac"] = f["by_top_freq"] = nan
    for s in SHAPES:
        f[f"shape_{s}"] = 1.0 if q.shape == s else 0.0
    return f


def names(schema: Schema, q: Question) -> list[str]:
    return list(features(schema, q))


def slots(schema: Schema, q: Question) -> dict[str, list[str]]:
    """The words a question is made of, by where they stand: what a teacher
    asks about is learned per word (salary, attendance, status) where no
    list of ours names it."""
    from aq import singular
    tb = schema.tables[q.base]
    out = {"base": [singular(t) for t in tb.tk] or ["#none"]}
    m = q.measure
    if m.cols:
        c = schema.tables[m.cols[0][0]].columns[m.cols[0][1]]
        kind = "product" if m.kind == "net" else m.kind
        out["measure"] = [f"{kind}:{t}" for t in c.tk] + [f"#{kind}:{m.agg}"]
    else:
        out["measure"] = [f"#{m.kind}"]
    if q.time:
        c = schema.tables[q.time.t].columns[q.time.c]
        out["time"] = c.tk or ["#none"]
    else:
        out["time"] = ["#none"]
    if q.by:
        bt = schema.tables[q.by.t]
        if q.by.c >= 0:
            out["by"] = bt.columns[q.by.c].tk or ["#none"]
        else:
            out["by"] = ["@" + singular(t) for t in bt.tk] or ["#none"]
    else:
        out["by"] = ["#none"]
    return out


# ---------------------------------------------------------------- round 1c's rules

WEIGHTS = {
    "base_first_subject": 2.0, "base_subject": 1.5, "base_detail_of_first": 1.2, "base_detail": 0.6,
    "base_role_event": 0.6, "base_role_fact": 0.4, "base_role_lookup": -2.0, "base_role_link": -3.0,
    "base_role_snapshot": -0.5, "base_boilerplate": -4.0, "base_activity_word": 0.6,
    "m_rows": 0.2, "m_column": 0.0, "m_product": 1.6, "m_net": 1.9, "m_parents": 0.2, "m_gap": 0.4, "m_late": 0.3,
    "m_per": -0.6, "m_avg": -0.6, "m_role_money": 1.3, "m_role_quantity": 0.5, "m_role_number": -0.6,
    "m_role_score": -0.4, "m_role_ratio": -0.3, "m_named_total": 0.4, "m_short_name": -0.2,
    "has_time": 0.6, "time_created": 0.4, "time_event": 0.2, "time_int": 0.3, "time_first_axis": 0.4,
    "time_famous": 0.3, "time_path": -0.2,
    "by_label": 0.7, "by_person": 0.4, "by_key": -0.6, "by_geo": 0.6, "by_bool": -0.4, "by_named": 0.5,
    "by_measured": 0.2, "by_typed": -0.6, "by_path": -0.3, "by_table_boilerplate": -3.0, "by_status": 0.3,
    "by_country": 0.2, "shape_split": -0.4, "shape_distribution": -0.8,
}


def prior(f: dict) -> float:
    return sum(w * f.get(k, 0.0) for k, w in WEIGHTS.items() if not math.isnan(f.get(k, 0.0)))


def veto(schema: Schema, q: Question) -> str | None:
    """What tier N rules out: an empty table, a breakdown of too many values
    or of one, a split of more than eight, a mostly empty column."""
    tb = schema.tables[q.base]
    if tb.rows is not None and tb.rows == 0:
        return "an empty table"
    if q.by and q.by.c >= 0:
        col = schema.tables[q.by.t].columns[q.by.c]
        if col.distinct is not None:
            if col.distinct <= 1:
                return "one value"
            if q.shape == "split" and col.distinct > 8:
                return "a split of too many values"
            if q.shape == "breakdown" and col.distinct > 60:
                return "too many values"
        if col.null_frac is not None and col.null_frac > 0.9:
            return "mostly null"
        if col.top_freq is not None and col.top_freq > 0.97:
            return "nearly one value"
    # the twelve passengers with the most boarding passes, each with one
    if q.by and q.by.c == -1 and q.shape == "top" and q.measure.kind in ("rows", "column", "product", "net"):
        pt = schema.tables[q.by.t]
        if pt.rows and tb.rows is not None and pt.rows > 10000 and tb.rows / pt.rows < 1.5:
            return "about one row each"
    if q.by and q.by.c == -1 and q.shape == "split":
        pt = schema.tables[q.by.t]
        if pt.rows is not None and pt.rows > 8:
            return "a split of too many values"
    if q.time:
        col = schema.tables[q.time.t].columns[q.time.c]
        if col.null_frac is not None and col.null_frac > 0.9:
            return "mostly null"
        if col.distinct is not None and col.distinct <= 1:
            return "one value"
    for tc in q.measure.cols:
        col = schema.tables[tc[0]].columns[tc[1]]
        if col.null_frac is not None and col.null_frac > 0.95:
            return "mostly null"
    return None


def stats_points(schema: Schema, q: Question) -> float:
    """Tier N's say in the order, beside a ranker that never saw it: a
    table of eight rows is no overview's subject."""
    tb = schema.tables[q.base]
    p = 0.0
    if tb.rows is not None:
        if tb.rows < 20:
            p -= 1.5
        elif tb.rows < 100:
            p -= 0.6
        # what happens in a database fills its largest tables, which no
        # schema without rows could teach the ranker
        known = [t.rows for t in schema.tables if t.rows]
        if known and max(known) > 1:
            p += 0.8 * math.log1p(tb.rows) / math.log1p(max(known))
    if q.by and q.by.c >= 0:
        col = schema.tables[q.by.t].columns[q.by.c]
        if col.null_frac is not None and col.null_frac > 0.3:
            p -= 0.5
        if col.distinct is not None and 3 <= col.distinct <= 12:
            p += 0.3
    return p


# ---------------------------------------------------------------- the five


# No cap on one kind of panel: capped at two, the five agreed with the
# teachers 0.47 of the time against 0.53, as they show several counts over
# time themselves
MAX_KIND = 5


def kind_of(ident: tuple) -> tuple:
    """What a panel looks like: its measure, and whether it has a time and
    a breakdown."""
    _, measure, time, by = ident
    return (measure[0], measure[1] == frozenset(), time is not None, by is not None)


def choose(schema: Schema, qs: list[Question], scores: list[float], k: int = 5) -> list[int]:
    """The k best that add something: no two over one time column or one
    breakdown, at most two from a table, no repeat of a question in another
    shape."""
    order = sorted(range(len(qs)), key=lambda i: -scores[i])
    chosen: list[int] = []
    per_table: dict[int, int] = {}
    per_kind: dict[tuple, int] = {}
    times, bys, asked = set(), set(), set()
    for i in order:
        q = qs[i]
        ident = identity(schema, q)
        t = (q.time.t, q.time.c) if q.time else None
        b = ident[3]
        kind = kind_of(ident)
        if ident in asked or per_table.get(q.base, 0) >= 2 or per_kind.get(kind, 0) >= MAX_KIND:
            continue
        if t is not None and t in times:
            continue
        if b is not None and b in bys:
            continue
        chosen.append(i)
        asked.add(ident)
        per_table[q.base] = per_table.get(q.base, 0) + 1
        per_kind[kind] = per_kind.get(kind, 0) + 1
        if t is not None:
            times.add(t)
        if b is not None:
            bys.add(b)
        if len(chosen) == k:
            break
    return chosen
