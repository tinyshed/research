"""Round 1c's analytical questions: what a panel computes, as an object
between the schema and the chart, generated from a snapshot and compiled to
SQL.

A question is a base table whose rows are aggregated; a measure — a count,
a sum or an average of a column or of an expression over one row, a count of
distinct parents, the time between two of a row's times, or the rows late
by them; and at most a time and a breakdown, each a column of the base or of
a table reached from it through foreign keys, many to one, so the grain
stays the base's and a sum stays a sum. Round 1's templates are its special
cases: one join, one column, one shape.

    from aq import load, generate
    schema = load(snapshot_dict, reads_n=True)
    questions = generate(schema)
    sql = questions[0].sql(schema)
"""

from __future__ import annotations

import math
import re
from dataclasses import dataclass, field

# ---------------------------------------------------------------- names


def tokens(name: str) -> list[str]:
    """`orderDate`, `order_date`, `OrderDATE2` → ['order', 'date']."""
    s = re.sub(r"([a-z0-9])([A-Z])", r"\1 \2", name)
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1 \2", s)
    return [t for t in re.split(r"[^A-Za-z]+", s.lower()) if t]


def norm(name: str) -> str:
    return re.sub(r"[^a-z0-9]", "", name.lower())


def singular(word: str) -> str:
    w = word.lower()
    for a, b in (("ies", "y"), ("sses", "ss"), ("ses", "s"), ("xes", "x"), ("s", "")):
        if w.endswith(a) and len(w) > len(a) + 2:
            return w[: -len(a)] + b
    return w


def has(tk: list[str], words) -> bool:
    return any(t in words or singular(t) in words for t in tk)


CREATED = {"created", "create", "creation", "inserted", "registered", "signup", "joined", "opened", "added", "submitted", "posted"}
UPDATED = {"updated", "modified", "changed", "edited", "touched", "synced", "refreshed", "seen", "login", "accessed"}
DELETED = {"deleted", "removed", "archived"}
BIRTH = {"birth", "dob", "birthday", "born", "death", "died", "expiry", "expires", "expiration", "valid", "until", "to", "end", "ends"}
TIMEISH = {"date", "time", "at", "on", "timestamp", "datetime", "day", "when", "dt", "ts"}
YEAR = {"year", "season", "yr"}
YEARMONTH = {"yearmonth", "period", "month", "ym"}
PLAN = {"scheduled", "planned", "expected", "estimated", "required", "due", "promised", "target", "ordered", "order", "requested", "start", "started", "begin", "opened", "created"}
ACTUAL = {"actual", "shipped", "delivered", "completed", "closed", "resolved", "paid", "finished", "arrived", "done", "end", "ended", "received", "returned"}

KEYISH = {"id", "uuid", "guid", "key", "hash", "token", "code", "sku", "ref", "phone", "fax", "zip", "postal", "postcode", "isbn", "ean", "upc", "iban", "ip", "url", "uri", "email", "mail", "password", "salt", "slug", "path", "file", "image", "photo", "picture", "avatar", "icon", "version", "lat", "lng", "lon", "latitude", "longitude", "number", "num", "no", "nr"}
MONEY = {"amount", "price", "total", "subtotal", "cost", "costs", "fee", "fees", "charge", "charges", "tax", "paid", "salary", "wage", "wages", "revenue", "income", "profit", "freight", "budget", "spend", "spent", "payment", "balance", "debit", "credit", "value", "sales", "turnover", "margin", "earnings", "bonus", "commission", "rent", "fare", "tip", "refund", "deposit", "withdrawal", "premium", "money", "cash"}
# Quantities that count units sold, so that price × them is an amount: not a
# track's milliseconds, which a price times is nothing
UNITS = {"qty", "quantity", "units", "count", "items", "sold", "pieces", "pcs", "tickets", "seats", "places", "nights", "number", "num", "amount", "volume", "liters", "litres", "kg"}
QUANTITY = {"qty", "quantity", "count", "units", "items", "points", "wins", "losses", "goals", "laps", "votes", "views", "clicks", "visits", "hits", "likes", "downloads", "plays", "sold", "consumption", "distance", "minutes", "hours", "seconds", "duration", "milliseconds", "ms", "length", "volume", "pages", "steps", "calories", "kills", "deaths", "assists", "runs", "attendance", "seats", "capacity", "stock", "weight", "size", "bytes", "impressions", "conversions", "orders", "sessions"}
RATIO = {"discount", "rate", "percent", "percentage", "pct", "ratio", "share", "probability", "avg", "average", "mean", "factor"}
SCORE = {"rating", "score", "stars", "rank", "ranking", "priority", "position", "grid", "round", "level", "grade", "age", "lap"}
CATEGORY = {"status", "state", "type", "kind", "category", "level", "role", "tier", "plan", "country", "source", "channel",
            "gender", "sex", "segment", "currency", "method", "mode", "class", "group", "league", "division", "genre",
            "language", "lang", "platform", "device", "browser", "os", "region", "city", "province", "nationality",
            "brand", "condition", "stage", "phase", "format", "medium", "unit", "size", "color", "colour", "pos",
            "bats", "throws", "hand", "conditions", "fare", "department", "dept", "team", "sport", "series", "result",
            "outcome", "reason", "gateway", "provider", "carrier", "vendor", "plan", "membership", "subscription"}
GEO = {"country", "city", "state", "region", "province", "nationality", "continent", "county", "district", "location"}
LABEL = {"name", "title", "label", "description", "desc", "caption", "subject", "headline", "heading", "nickname", "username", "login", "handle",
         "nome", "nombre", "nom", "titulo", "titel", "naam", "ten", "nazwa", "nazev"}
PERSON_PART = {"first", "last", "given", "family", "middle", "full", "fore", "sur", "maiden", "nick"}
PERSON_TABLE = {"user", "users", "customer", "customers", "employee", "employees", "member", "members", "person", "people",
                "persons", "player", "players", "driver", "drivers", "author", "authors", "student", "students", "teacher",
                "teachers", "patient", "patients", "client", "clients", "staff", "contact", "contacts", "account", "accounts",
                "manager", "managers", "agent", "agents", "actor", "actors", "artist", "artists", "doctor", "doctors"}
# Long by type alone: PostgreSQL's `text` and MySQL's `varchar(255)` hold a
# status as often as a page, as round 1 found
LONG_TEXT = {"longtext", "mediumtext", "clob", "nclob", "ntext", "json", "jsonb", "xml", "bytea", "blob", "longblob", "mediumblob", "binary", "varbinary", "image"}
SNAPSHOT = {"standing", "standings", "snapshot", "snapshots", "balance", "balances", "inventory", "inventories", "stock", "stocks", "history"}
BOILERPLATE = re.compile(
    r"^(django_|auth_|oauth|migrations?$|schema_migrations|ar_internal|failed_jobs|password_resets?|"
    r"personal_access_tokens|sessions?$|cache|jobs$|job_batches|flyway|databasechangelog|sqlite_|"
    r"qrtz_|act_|permissions?$|role_has|model_has|users_roles|spring_session|pg_stat)",
    re.I,
)

# ---------------------------------------------------------------- types


def family(type_name: str, base_type: str) -> str:
    t = (base_type or type_name or "").lower()
    full = (type_name or "").lower()
    # an array of numbers is no number: the airlines' days of the week
    if "[]" in full or t.startswith("_") or t == "array":
        return "other"
    if t in LONG_TEXT or full in LONG_TEXT:
        return "long"
    if t in {"bool", "boolean", "bit"} or full.startswith("tinyint(1)"):
        return "bool"
    if "timestamp" in t or "datetime" in t or t in {"smalldatetime", "datetime2"}:
        return "timestamp"
    if t == "date":
        return "date"
    if t in {"time", "timetz", "interval", "year"}:
        return "int" if t == "year" else "other"
    if any(k in t for k in ("int", "serial")) or t in {"number", "tech_id"}:
        return "int"
    if t in {"decimal", "numeric", "money", "smallmoney", "udecimal"}:
        return "decimal"
    if t in {"float", "double", "real", "double precision", "doubleprecision", "float4", "float8"}:
        return "float"
    if "char" in t or t in {"text", "enum", "string", "tinytext", "set", "citext", "name", "character", "uuid"}:
        return "uuid" if t == "uuid" else "text"
    return "other"


def length(type_name: str) -> int | None:
    m = re.search(r"\((\d+)", type_name or "")
    return int(m.group(1)) if m else None


def scale(type_name: str) -> int | None:
    m = re.search(r"\(\d+\s*,\s*(\d+)\)", type_name or "")
    return int(m.group(1)) if m else None


# ---------------------------------------------------------------- the schema


@dataclass
class Column:
    t: int
    c: int
    name: str
    type_name: str
    fam: str
    nullable: bool
    tk: list[str]
    distinct: float | None = None
    null_frac: float | None = None
    top_freq: float | None = None
    pk: bool = False
    unique: bool = False
    fk: int | None = None          # the table it points at
    fk_guessed: bool = False
    # what it may be
    time: str | None = None        # created | event | updated | birth | int
    measure: str | None = None     # money | quantity | ratio | score | number
    category: str | None = None    # named | measured | typed | bool | geo
    label: str | None = None       # name | person | first | last
    key: bool = False


@dataclass
class Table:
    t: int
    name: str
    tk: list[str]
    rows: float | None
    columns: list[Column]
    pk: list[int]
    boilerplate: bool
    out: list[tuple[int, int]] = field(default_factory=list)  # (column, parent table)
    referenced_by: list[tuple[int, int]] = field(default_factory=list)  # (child table, column)
    role: str = "table"            # fact | event | entity | lookup | link | snapshot | table
    timed: bool = False
    subject: int | None = None
    detail_of: int | None = None
    label: int | None = None       # its name column for a person
    person: bool = False
    first_last: tuple[int, int] | None = None


@dataclass
class Schema:
    engine: str
    tables: list[Table]
    reads_n: bool

    def col(self, ref):
        return self.tables[ref[0]].columns[ref[1]]


def _stat(features: dict, name: str, reads_n: bool):
    if not reads_n:
        return None
    f = features.get(name)
    if not f or f.get("state") not in ("present", "stale"):
        return None
    v = f.get("value")
    return v if isinstance(v, (int, float)) else None


def load(snap: dict, reads_n: bool) -> Schema:
    tables: list[Table] = []
    raw = [t for t in snap["tables"] if t.get("kind", "table") in ("table", "partitioned")]
    index = {}
    for t, rt in enumerate(raw):
        index[f"{rt.get('schema', 'public')}.{rt['name']}".lower()] = t
        index.setdefault(rt["name"].lower(), t)
    for t, rt in enumerate(raw):
        rows = _stat(rt.get("features", {}), "rows_estimate", reads_n)
        pk = next((k["columns"] for k in rt.get("constraints", []) if k["kind"] == "p"), [])
        uniques = [k["columns"] for k in rt.get("constraints", []) if k["kind"] == "u"]
        uniques += [i["columns"] for i in rt.get("indexes", []) if i.get("unique")]
        cols = []
        for c, rc in enumerate(rt["columns"]):
            feats = rc.get("features", {})
            col = Column(
                t=t, c=c, name=rc["name"], type_name=rc.get("type_name", ""),
                fam=family(rc.get("type_name", ""), rc.get("base_type", "")),
                nullable=bool(rc.get("nullable", True)), tk=tokens(rc["name"]),
            )
            n = _stat(feats, "n_distinct", reads_n)
            if n is None:
                n = _stat(feats, "index_distinct", reads_n)
            if n is not None and n < 0 and rows:
                n = -n * rows
            col.distinct = n if n is None or n >= 0 else None
            col.null_frac = _stat(feats, "null_frac", reads_n)
            col.top_freq = _stat(feats, "mcv_top_freq", reads_n)
            col.pk = rc["name"] in pk
            col.unique = (col.pk and len(pk) == 1) or [rc["name"]] in uniques
            cols.append(col)
        tables.append(Table(
            t=t, name=rt["name"], tk=tokens(rt["name"]), rows=rows, columns=cols,
            pk=[c.c for c in cols if c.pk], boilerplate=bool(BOILERPLATE.match(rt["name"])),
        ))
    schema = Schema(engine=snap.get("engine", ""), tables=tables, reads_n=reads_n)
    # declared keys of one column
    for t, rt in enumerate(raw):
        for k in rt.get("constraints", []):
            if k["kind"] != "f" or len(k["columns"]) != 1:
                continue
            p = index.get((k.get("ref_table") or "").lower())
            if p is None:
                p = index.get((k.get("ref_table") or "").split(".")[-1].lower())
            if p is None:
                continue
            c = next((x for x in tables[t].columns if x.name == k["columns"][0]), None)
            if c is not None and c.fk is None:
                c.fk = p
    _guess_keys(schema)
    for tb in tables:
        for col in tb.columns:
            if col.fk is not None:
                tb.out.append((col.c, col.fk))
                if col.fk != tb.t:
                    tables[col.fk].referenced_by.append((tb.t, col.c))
    _roles(schema)
    _subjects(schema)
    return schema


def _guess_keys(schema: Schema) -> None:
    """A column named after a table, or after its one-column key, points at
    it when the schema did not say: ccs's transactions_1k.CustomerID."""
    by_name: dict[str, int] = {}
    for tb in schema.tables:
        by_name.setdefault(norm(tb.name), tb.t)
        by_name.setdefault(norm(singular(tb.name)), tb.t)
    key_of = {}
    for tb in schema.tables:
        if len(tb.pk) == 1:
            key_of[norm(tb.columns[tb.pk[0]].name)] = tb.t
    for tb in schema.tables:
        for col in tb.columns:
            if col.fk is not None or (col.pk and len(tb.pk) == 1) or col.fam not in ("int", "text", "uuid"):
                continue
            n = norm(col.name)
            target = None
            if n in key_of and key_of[n] != tb.t and norm(schema.tables[key_of[n]].columns[schema.tables[key_of[n]].pk[0]].name) not in ("id",):
                target = key_of[n]
            else:
                m = re.match(r"^(.*?)(id|key|code|no)$", n)
                if m and m.group(1):
                    target = by_name.get(m.group(1))
            if target is None or target == tb.t:
                continue
            p = schema.tables[target]
            if len(p.pk) != 1:
                continue
            pcol = p.columns[p.pk[0]]
            same = (pcol.fam == col.fam) or {pcol.fam, col.fam} <= {"int"} or {pcol.fam, col.fam} <= {"text", "uuid"}
            if same:
                col.fk = target
                col.fk_guessed = True


def _roles(schema: Schema) -> None:
    for tb in schema.tables:
        names = {norm(c.name) for c in tb.columns}
        for col in tb.columns:
            tk = col.tk
            counted = col.fam in ("int", "decimal", "float") and (has(tk, QUANTITY | MONEY) or tk[:2] == ["no", "of"] or tk[:2] == ["num", "of"] or tk[:2] == ["number", "of"])
            keyish = (has(tk, KEYISH) and not counted) or col.pk or col.fk is not None or norm(col.name).endswith("id")
            composite = col.pk and len(tb.pk) > 1
            code = col.fam == "text" and col.fk is None and not (col.pk and len(tb.pk) == 1) and (composite or (col.distinct is not None and col.distinct <= 60))
            if code and has(tk, {"id", "code"}):
                keyish = False
            col.key = keyish
            # times
            if col.fam in ("timestamp", "date"):
                if has(tk, UPDATED):
                    col.time = "updated"
                elif has(tk, DELETED):
                    col.time = "deleted"
                elif has(tk, BIRTH):
                    col.time = "birth"
                elif has(tk, CREATED):
                    col.time = "created"
                else:
                    col.time = "event"
            elif col.fam == "int" and not col.fk and (has(tk, YEAR) or any(t.endswith("year") for t in tk) or norm(col.name) in YEARMONTH or (has(tk, {"date"}) and not has(tk, UPDATED))):
                if not has(tk, BIRTH) and not (has(tk, {"min", "max"})):
                    col.time = "int"
                    col.key = False
            # measures, on numbers that are no keys
            if col.fam in ("int", "decimal", "float") and not keyish and col.time is None:
                if has(tk, RATIO):
                    col.measure = "ratio"
                elif has(tk, {"qty", "quantity", "units", "sold", "pieces", "pcs"}) or tk[:2] in (["no", "of"], ["num", "of"], ["number", "of"]):
                    col.measure = "quantity"
                elif has(tk, MONEY) or (col.fam == "decimal" and scale(col.type_name) in (2, 4)):
                    col.measure = "money"
                elif has(tk, SCORE):
                    col.measure = "score"
                elif has(tk, QUANTITY):
                    col.measure = "quantity"
                else:
                    col.measure = "number"
            # categories
            short = col.fam == "text" and (length(col.type_name) or 255) <= 255 and not (col.distinct is None and has(tk, {"description", "comment", "comments", "note", "notes", "body", "content", "message", "text", "summary", "bio", "about", "address"}))
            few = col.distinct is not None and 1 < col.distinct <= 60 and (tb.rows is None or col.distinct <= max(12, 0.2 * tb.rows))
            small_int = col.fam == "int" and re.match(r"(?i)(tiny|small)int|int2|bit", col.type_name or "")
            flaglike = col.fam == "int" and (tk[:1] in (["is"], ["has"], ["can"]) or has(tk, {"enabled", "active", "deleted", "flag", "visible", "published", "approved", "verified", "blocked", "banned", "hidden", "archived", "paid", "done", "completed"}))
            if col.fam == "bool" or flaglike:
                col.category = "bool"
            elif col.fam == "other" and has(tk, CATEGORY):
                col.category = "named"
            elif small_int and not keyish and col.time is None and col.measure in ("number", None):
                col.category = "named" if has(tk, CATEGORY) else "typed"
            elif (short or (col.fam == "int" and col.measure in (None, "number", "score"))) and not keyish and col.time is None:
                if has(tk, GEO):
                    col.category = "geo"
                elif has(tk, CATEGORY):
                    col.category = "named"
                elif few and (col.fam == "text" or col.measure is None):
                    col.category = "measured"
                elif short and col.distinct is None and not has(tk, LABEL) and not has(tk, PERSON_PART):
                    col.category = "typed"
            if col.category and col.distinct is not None and (col.distinct <= 1 or col.distinct > 200):
                col.category = None
            if col.fam == "int" and col.category in ("typed",) and not small_int:
                col.category = None
            # labels
            if col.fam == "text" and not col.pk and (length(col.type_name) or 255) <= 255:
                if has(tk, PERSON_PART) and has(tk, {"name"}) or norm(col.name) in ("firstname", "lastname", "namefirst", "namelast"):
                    col.label = "first" if has(tk, {"first", "given", "fore"}) else "last" if has(tk, {"last", "family", "sur"}) else "person"
                elif (has(tk, LABEL) or any(t.endswith("name") and len(t) <= 8 for t in tk)) and not has(tk, {"email", "mail", "url"}):
                    col.label = "name"
        # a table's label: a name, or a person's first and last names
        firsts = [c for c in tb.columns if c.label == "first"]
        lasts = [c for c in tb.columns if c.label == "last"]
        names_ = sorted((c for c in tb.columns if c.label == "name"),
                        key=lambda c: (0 if c.tk[-1:] == ["name"] and len(c.tk) <= 2 else 1 if "name" in c.tk else 2 if "title" in c.tk else 3, c.c))
        tb.person = has(tb.tk, PERSON_TABLE) or bool(firsts and lasts)
        if names_:
            tb.label = names_[0].c
        else:
            texts = [c for c in tb.columns if c.fam == "text" and not c.pk and c.fk is None and not c.key]
            if len(texts) == 1 and len([c for c in tb.columns if not c.pk]) <= 3:
                tb.label = texts[0].c
        if firsts and lasts:
            tb.first_last = (firsts[0].c, lasts[0].c)
            if tb.label is None or tb.person:
                tb.label = lasts[0].c


def _subjects(schema: Schema) -> None:
    """What the database is about: the tables with a time of their own that
    record what happened — headers with their lines hanging on them, and
    flat tables of events — the most activity first."""
    for tb in schema.tables:
        cols = tb.columns
        out = [p for _, p in tb.out if p != tb.t]
        measures = [c for c in cols if c.measure in ("money", "quantity")]
        tb.timed = any(c.time in ("created", "event", "int") for c in cols)
        non_key = [c for c in cols if not (c.pk or c.fk is not None)]
        if len(out) == 2 and len(non_key) <= 1 and not any(c.measure for c in cols):
            tb.role = "link"
        elif has(tb.tk, SNAPSHOT):
            tb.role = "snapshot"
        elif len(out) >= 2 or (out and (tb.timed or measures)):
            tb.role = "fact" if (measures or len(out) >= 2) else "event"
        elif tb.timed:
            tb.role = "event"
        elif tb.referenced_by and len(non_key) <= 3 and not measures:
            tb.role = "lookup"
        else:
            tb.role = "entity"
    # subjects: timed facts and events, their activity their rows and the
    # rows of the untimed facts hanging on them
    hanging: dict[int, list[int]] = {}
    for tb in schema.tables:
        if tb.timed or tb.role not in ("fact", "snapshot"):
            continue
        parents = [p for _, p in tb.out if p != tb.t and schema.tables[p].timed]
        if parents:
            hanging.setdefault(parents[0], []).append(tb.t)
            tb.detail_of = parents[0]
    candidates = [tb for tb in schema.tables if tb.timed and tb.role in ("fact", "event", "snapshot") and not tb.boilerplate]

    def activity(tb: Table) -> float:
        rows = [tb.rows] + [schema.tables[h].rows for h in hanging.get(tb.t, [])]
        if all(r is not None for r in rows):
            return sum(math.log1p(r) for r in rows)
        return 1 + len(hanging.get(tb.t, [])) + 0.5 * len(tb.out) + 0.5 * len(tb.referenced_by)

    for rank, tb in enumerate(sorted(candidates, key=lambda x: (-activity(x), x.t))):
        tb.subject = rank


# ---------------------------------------------------------------- questions

@dataclass(frozen=True)
class Measure:
    agg: str                 # count | sum | avg | distinct | late
    cols: tuple = ()         # ((t, c), ...) over the base's row
    kind: str = "rows"       # rows | column | product | net | gap | parents | late

    def columns(self) -> frozenset:
        return frozenset(self.cols)


@dataclass(frozen=True)
class Dim:
    path: tuple              # ((table, fk column), ...) from the base
    t: int
    c: int                   # the column, or -1 for a parent's identity (its label)
    kind: str                # time: created | event | int ; breakdown: own | label | person | key | geo | bool | named | measured | typed


@dataclass(frozen=True)
class Question:
    base: int
    measure: Measure
    time: Dim | None = None
    by: Dim | None = None
    shape: str = "series"    # series | breakdown | top | split | number

    def key(self, schema: Schema) -> tuple:
        return identity(schema, self)

    def title(self, schema: Schema) -> str:
        b = schema.tables[self.base].name
        m = self.measure
        cname = lambda tc: f"{schema.tables[tc[0]].name}.{schema.tables[tc[0]].columns[tc[1]].name}"
        if m.kind == "rows":
            what = f"rows of {b}"
        elif m.kind == "column":
            what = f"{'sum' if m.agg == 'sum' else 'average'} of {cname(m.cols[0])}"
        elif m.kind in ("product", "net"):
            what = f"sum of {' × '.join(cname(x) for x in m.cols[:2])}" + (f" × (1 − {cname(m.cols[2])})" if m.kind == "net" else "")
        elif m.kind == "parents":
            fk = schema.col(m.cols[0]).fk
            what = f"distinct {schema.tables[fk].name if fk is not None else cname(m.cols[0])} in {b}"
        elif m.kind == "flagged":
            what = f"rows of {b} with {cname(m.cols[0])}"
        elif m.kind == "gap":
            what = f"average days from {cname(m.cols[0])} to {cname(m.cols[1])}"
        elif m.kind == "per":
            what = f"rows of {b} per {schema.tables[schema.col(m.cols[0]).fk].name}"
        else:
            what = f"rows of {b} with {cname(m.cols[1])} after {cname(m.cols[0])}"
        parts = [what]
        if self.time:
            parts.append(f"by {dim_name(schema, self.time)}")
        if self.by:
            parts.append(("split by " if self.time else "by ") + dim_name(schema, self.by))
        if self.shape == "top":
            parts.append("the twelve most")
        return ", ".join(parts) if self.shape == "top" else " ".join(parts)

    def alive(self, schema: Schema) -> list:
        return alive(schema.tables[self.base])

    def sql(self, schema: Schema, engine: str | None = None, bucket: str | None = None) -> str:
        return compile_sql(schema, self, engine or schema.engine, bucket)


def alive(tb: Table) -> list:
    """The columns that say a row was deleted, and how a live row reads them:
    a time of deletion left empty, a flag left off."""
    out = []
    for c in tb.columns:
        if c.time == "deleted":
            out.append((c.c, "null"))
        elif (c.fam == "bool" or c.category == "bool") and has(c.tk, DELETED):
            out.append((c.c, "off"))
    return out


def named(tb: Table) -> bool:
    """Whether a table's rows have names to group them by: a customer's, a
    genre's, a track's. A table of what happened — with a time of its own,
    or a header's — has none, though a column may say `name`: Northwind's
    orders keep the name to ship to, which two orders share, and grouped by
    it each order stood alone under a customer's name, as two blind judges
    found."""
    return (tb.label is not None or tb.first_last is not None) and not tb.timed and tb.detail_of is None


def dim_name(schema: Schema, d: Dim) -> str:
    tb = schema.tables[d.t]
    if d.c == -1:
        if d.kind == "key" or not named(tb):
            return f"{tb.name} (by key)"
        if tb.first_last:
            return f"{tb.name} (named)"
        return f"{tb.name}.{tb.columns[tb.label].name}"
    return f"{tb.name}.{tb.columns[d.c].name}"


def identity(schema: Schema, q: Question) -> tuple:
    """What a question asks, as gold questions are matched against it: the
    base, the measure's aggregate and columns, the time column, and the
    breakdown as a column or as the parent it names."""
    m = q.measure
    measure = (m.agg if m.kind != "parents" else "distinct", frozenset(m.cols))
    time = (q.time.t, q.time.c) if q.time else None
    by = None
    if q.by:
        by = ("entity", q.by.t) if q.by.c == -1 else ("col", q.by.t, q.by.c)
    return (q.base, measure, time, by)


def _paths(schema: Schema, base: int, depth: int) -> list[tuple]:
    """Every chain of foreign keys from the base, many to one, without a
    table twice: ((table, column), ...) where each step leaves `table` by
    `column`."""
    out = [()]
    frontier = [((), base)]
    for _ in range(depth):
        nxt = []
        for path, t in frontier:
            seen = {base} | {schema.tables[s[0]].columns[s[1]].fk for s in path}
            for c, p in schema.tables[t].out:
                if p in seen:
                    continue
                step = path + ((t, c),)
                out.append(step)
                nxt.append((step, p))
        frontier = nxt
    return out


def _end(schema: Schema, base: int, path: tuple) -> int:
    return schema.tables[path[-1][0]].columns[path[-1][1]].fk if path else base


def _few(col: Column, limit: float) -> bool | None:
    return None if col.distinct is None else col.distinct <= limit


def generate(schema: Schema, max_depth: int = 3) -> list[Question]:
    qs: list[Question] = []
    seen = set()

    def add(q: Question):
        k = identity(schema, q) + (q.shape,)
        if k not in seen:
            seen.add(k)
            qs.append(q)

    for base in schema.tables:
        if base.rows == 0:
            continue
        b = base.t
        paths = _paths(schema, b, max_depth)
        if base.role == "link":
            # films per actor: a link counted by the names of what it links
            for path in paths:
                if len(path) == 1:
                    p = _end(schema, b, path)
                    pt = schema.tables[p]
                    kind = ("person" if pt.person else "label") if named(pt) else "key"
                    add(Question(b, Measure("count"), None, Dim(path, p, -1, kind), "top"))
                    for c in pt.columns:
                        if c.category and c.category != "typed":
                            add(Question(b, Measure("count"), None, Dim(path, p, c.c, c.category), "breakdown"))
            continue
        # measures over the base's row
        measures = [Measure("count")]
        cols = base.columns
        money = [c for c in cols if c.measure == "money"]
        qty = [c for c in cols if c.measure == "quantity"]
        disc = [c for c in cols if c.measure == "ratio" and has(c.tk, {"discount"})]
        for c in cols:
            if c.measure in ("money", "quantity", "number"):
                measures.append(Measure("sum", ((b, c.c),), "column"))
            if c.measure in ("money", "quantity", "number", "ratio", "score"):
                measures.append(Measure("avg", ((b, c.c),), "column"))
        # a line's amount: price × quantity, less its discount
        for m in money:
            for q in qty:
                if has(q.tk, UNITS) and has(m.tk, {"price", "cost", "fee", "rate", "charge", "value"}):
                    # where the row keeps a discount, its amount is net of it
                    if disc:
                        for d in disc:
                            measures.append(Measure("sum", ((b, m.c), (b, q.c), (b, d.c)), "net"))
                    else:
                        measures.append(Measure("sum", ((b, m.c), (b, q.c)), "product"))
        # a line's quantity × its product's price, when the line keeps no price
        if qty and not money:
            for c, p in base.out:
                for pm in schema.tables[p].columns:
                    if p != b and pm.measure == "money" and has(pm.tk, {"price", "cost", "fee", "rate", "value"}):
                        for q in qty:
                            if has(q.tk, UNITS):
                                measures.append(Measure("sum", ((b, q.c), (p, pm.c)), "product"))
        # parents counted once: distinct customers in the orders
        for c, p in base.out:
            if p != b and schema.tables[p].role != "lookup" and not schema.tables[p].boilerplate:
                measures.append(Measure("distinct", ((b, c),), "parents"))
        if base.timed:
            for c in cols:
                if c.fk is None and c.key and not c.pk and c.fam in ("int", "text", "uuid") and norm(c.name).endswith("id"):
                    measures.append(Measure("distinct", ((b, c.c),), "parents"))
        # rows with a flag set: published posts, refused answers
        flags = [c for c in cols if c.fam == "bool" or (c.category == "bool")]
        for fl in flags[:3]:
            measures.append(Measure("count_if", ((b, fl.c),), "flagged"))
        # how many of this table each parent has: tickets per booking
        for c, p in base.out:
            if p != b and not schema.tables[p].boilerplate:
                add(Question(b, Measure("per", ((b, c),), "per"), None, None, "distribution"))
        # two times of a row: how long between them, and how many late
        times = [c for c in cols if c.time in ("created", "event") and c.fam in ("timestamp", "date")]
        for x in times:
            for y in times:
                if x.c == y.c:
                    continue
                if has(x.tk, PLAN) and has(y.tk, ACTUAL) and not has(x.tk, ACTUAL):
                    measures.append(Measure("avg", ((b, x.c), (b, y.c)), "gap"))
                    if has(x.tk, {"required", "due", "scheduled", "expected", "promised", "planned", "target"}):
                        measures.append(Measure("late", ((b, x.c), (b, y.c)), "late"))

        # times: the base's own, else the nearest parent's
        own = [Dim((), b, c.c, c.time) for c in cols if c.time in ("created", "event", "int")]
        tdims = own
        if not own:
            # a line's time is its header's: the times of the parents that
            # record what happened, one step away, else two; never an
            # entity's, as an employee's hire date is no order's time
            for depth in (1, 2):
                for path in paths:
                    if len(path) != depth:
                        continue
                    p = _end(schema, b, path)
                    if schema.tables[p].role not in ("fact", "event", "snapshot"):
                        continue
                    tdims += [Dim(path, p, c.c, c.time) for c in schema.tables[p].columns if c.time in ("created", "event", "int")]
                if tdims:
                    break
        # breakdowns: the base's categories; for each table reached, its
        # name or its key, and its categories
        bdims: list[Dim] = []
        for c in cols:
            if c.category:
                bdims.append(Dim((), b, c.c, c.category))
            elif c.label and base.role in ("fact", "event", "snapshot"):
                bdims.append(Dim((), b, c.c, "own_label"))
        for path in paths:
            if not path:
                continue
            p = _end(schema, b, path)
            pt = schema.tables[p]
            if pt.boilerplate:
                continue
            if named(pt):
                bdims.append(Dim(path, p, -1, "person" if pt.person else "label"))
            elif len(path) <= 2:
                bdims.append(Dim(path, p, -1, "key"))
            if len(path) <= 2:
                for c in pt.columns:
                    if c.category and (c.category != "typed" or len(path) == 1):
                        bdims.append(Dim(path, p, c.c, c.category))
        # an identifier with no table behind it: ccs's GasStationID before a key is guessed
        for c in cols:
            if c.fk is None and c.key and not (c.pk and len(base.pk) == 1) and c.fam in ("int", "text") and norm(c.name).endswith("id"):
                bdims.append(Dim((), b, c.c, "key"))

        for m in measures:
            for td in tdims:
                add(Question(b, m, td, None, "series"))
            for bd in bdims:
                col = schema.tables[bd.t].columns[bd.c] if bd.c >= 0 else None
                many = bd.c == -1 or bd.kind in ("key", "own_label") or (col is not None and (col.distinct or 0) > 12)
                add(Question(b, m, None, bd, "top" if many else "breakdown"))
                # over time, split by a few values: a status, a currency, a
                # lookup's handful of names
                pt = schema.tables[bd.t]
                small_lookup = bd.c == -1 and bd.kind == "label" and pt.role == "lookup" and (pt.rows is None or pt.rows <= 12)
                few_values = bd.c >= 0 and bd.kind in ("named", "measured", "bool", "geo", "typed") and _few(col, 8) is not False
                if m.kind in ("rows", "column", "product", "net", "flagged", "parents") and (few_values or small_lookup):
                    for td in tdims[:2]:
                        add(Question(b, m, td, bd, "split"))
    return qs


# ---------------------------------------------------------------- SQL


def _q(engine: str, name: str) -> str:
    return f"`{name}`" if engine == "mysql" else '"' + name.replace('"', '""') + '"'


def compile_sql(schema: Schema, q: Question, engine: str, bucket_name: str | None = None) -> str:
    E = lambda n: _q(engine, n)
    base = schema.tables[q.base]
    aliases = {(): "b"}
    joins = []

    def alias(path: tuple) -> str:
        if path in aliases:
            return aliases[path]
        parent = alias(path[:-1])
        t, c = path[-1]
        p = schema.tables[t].columns[c].fk
        pt = schema.tables[p]
        a = f"p{len(aliases)}"
        aliases[path] = a
        key = pt.columns[pt.pk[0]].name if pt.pk else pt.columns[0].name
        joins.append(f"LEFT JOIN {E(pt.name)} {a} ON {a}.{E(key)} = {parent}.{E(schema.tables[t].columns[c].name)}")
        return a

    def colref(path: tuple, t: int, c: int) -> str:
        return f"{alias(path)}.{E(schema.tables[t].columns[c].name)}"

    def bucket(d: Dim) -> str:
        col = schema.tables[d.t].columns[d.c]
        ref = colref(d.path, d.t, d.c)
        if d.kind == "int" or col.fam == "int":
            return ref
        unit = bucket_name or "month"
        if engine == "mysql":
            return {
                "day": f"DATE({ref})",
                "week": f"DATE({ref} - INTERVAL WEEKDAY({ref}) DAY)",
                "month": f"DATE_FORMAT({ref}, '%Y-%m-01')",
                "year": f"DATE_FORMAT({ref}, '%Y-01-01')",
            }[unit]
        return f"date_trunc('{unit}', {ref})::date"

    def label(d: Dim) -> str:
        if d.c >= 0:
            return colref(d.path, d.t, d.c)
        pt = schema.tables[d.t]
        if d.kind != "key" and named(pt) and pt.first_last:
            f, l = pt.first_last
            return f"CONCAT_WS(' ', {colref(d.path, d.t, f)}, {colref(d.path, d.t, l)})"
        if d.kind != "key" and named(pt):
            return colref(d.path, d.t, pt.label)
        # the key the base holds, with no join
        t, c = d.path[-1]
        return colref(d.path[:-1], t, c)

    m = q.measure
    b = lambda tc: colref((), tc[0], tc[1])
    if m.kind == "per":
        # how many parents have one child, two, three: a distribution
        k = b(m.cols[0])
        return (f"SELECT n AS g, COUNT(*) AS v FROM (SELECT {k} AS k, COUNT(*) AS n FROM {E(base.name)} b "
                f"WHERE {k} IS NOT NULL GROUP BY {k}) x GROUP BY n ORDER BY n LIMIT 200")
    if m.kind == "rows":
        value = "COUNT(*)"
    elif m.kind == "column":
        value = f"{m.agg.upper()}({b(m.cols[0])})"
    elif m.kind == "product":
        x, y = m.cols
        if y[0] != q.base:
            # the parent's price, one step away
            step = next(((q.base, c) for c, p in base.out if p == y[0]), None)
            yref = colref((step,), y[0], y[1])
        else:
            yref = b(y)
        value = f"SUM({b(x)} * {yref})"
    elif m.kind == "flagged":
        # a flag may be a boolean or an integer 0 or 1, as BIRD's are; both
        # cast to an integer on PostgreSQL, which takes no integer as a truth
        value = f"SUM(CASE WHEN {b(m.cols[0])}::int <> 0 THEN 1 ELSE 0 END)" if engine != "mysql" else f"SUM({b(m.cols[0])} <> 0)"
    elif m.kind == "net":
        value = f"SUM({b(m.cols[0])} * {b(m.cols[1])} * (1 - COALESCE({b(m.cols[2])}, 0)))"
    elif m.kind == "parents":
        value = f"COUNT(DISTINCT {b(m.cols[0])})"
    elif m.kind == "gap":
        x, y = b(m.cols[0]), b(m.cols[1])
        value = (f"AVG(TIMESTAMPDIFF(SECOND, {x}, {y})) / 86400" if engine == "mysql"
                 else f"AVG(EXTRACT(EPOCH FROM ({y}::timestamp - {x}::timestamp))) / 86400")
    else:
        x, y = b(m.cols[0]), b(m.cols[1])
        value = f"SUM(CASE WHEN {y} > {x} THEN 1 ELSE 0 END)"

    select, group, order, where = [], [], [], []
    for c, how in alive(base):
        ref = colref((), q.base, c)
        where.append(f"{ref} IS NULL" if how == "null" else f"COALESCE({ref}, 0) = 0" if engine == "mysql" else f"NOT COALESCE({ref}::int::boolean, false)")
    if q.time:
        t = bucket(q.time)
        select.append(f"{t} AS t")
        group.append(t)
        order.append("1")
        where.append(f"{colref(q.time.path, q.time.t, q.time.c)} IS NOT NULL")
    if q.by:
        g = label(q.by)
        select.append(f"{g} AS g")
        group.append(g)
        if q.by.c == -1 and q.by.kind != "key" and named(schema.tables[q.by.t]):
            # two parents of one name stay two
            pt = schema.tables[q.by.t]
            if pt.pk and q.by.path:
                group.append(colref(q.by.path, q.by.t, pt.pk[0]))
    select.append(f"{value} AS v")
    sql = f"SELECT {', '.join(select)} FROM {E(base.name)} b"
    if joins:
        sql += " " + " ".join(joins)
    if where:
        sql += " WHERE " + " AND ".join(where)
    if group:
        sql += " GROUP BY " + ", ".join(group)
    if q.shape == "top":
        sql += " ORDER BY v DESC LIMIT 12"
    elif q.time:
        sql += " ORDER BY " + ", ".join(order)
    else:
        sql += " ORDER BY v DESC LIMIT 50"
    return sql
