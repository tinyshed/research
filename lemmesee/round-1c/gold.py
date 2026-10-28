"""Gold questions — round 1's, Haiku's blind ones and the teachers' — read as
analytical questions, so a candidate answers one when it asks the same
thing: the same base, the same measure's aggregate and columns, the same
time column, and the same breakdown, as a column or as the parent it names.
"""

from __future__ import annotations

import re

from aq import Schema, alive, norm, singular

REF = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)\.([A-Za-z_][A-Za-z0-9_]*)")


def table_of(schema: Schema, name: str | None) -> int | None:
    if not name:
        return None
    n = norm(name.split(".")[-1])
    for tb in schema.tables:
        if norm(tb.name) == n:
            return tb.t
    for tb in schema.tables:
        if norm(singular(tb.name)) == norm(singular(n)):
            return tb.t
    return None


def column_of(schema: Schema, t: int | None, name: str) -> int | None:
    if t is None:
        return None
    n = norm(name)
    for c in schema.tables[t].columns:
        if norm(c.name) == n:
            return c.c
    return None


def ref(schema: Schema, text: str | None, default: int | None = None):
    """The first `table.column` in a text, as (t, c); a bare column on the
    default table."""
    if not text:
        return None
    m = REF.search(text)
    if m:
        t = table_of(schema, m.group(1))
        c = column_of(schema, t, m.group(2))
        return (t, c) if c is not None else None
    if default is not None:
        c = column_of(schema, default, text.strip())
        return (default, c) if c is not None else None
    return None


def refs(schema: Schema, text: str, default: int | None) -> list:
    out = []
    for m in REF.finditer(text):
        t = table_of(schema, m.group(1))
        c = column_of(schema, t, m.group(2))
        if c is not None:
            out.append((t, c))
    if not out and default is not None:
        for w in re.findall(r"[A-Za-z_][A-Za-z0-9_]*", text):
            c = column_of(schema, default, w)
            if c is not None:
                out.append((default, c))
    return out


def breakdown(schema: Schema, base: int, text: str | None):
    if not text:
        return None
    r = ref(schema, text, base)
    if r is None:
        # a table named alone, or first: "customer", "track, named by track.name"
        t = table_of(schema, re.split(r"[,\s]", text.strip())[0])
        return ("entity", t) if t is not None else "unknown"
    t, c = r
    tb = schema.tables[t]
    col = tb.columns[c]
    if t == base:
        if col.fk is not None and col.fk != base:
            return ("entity", col.fk)
        if col.pk and len(tb.pk) == 1:
            return ("entity", t)
        return ("col", t, c)
    # a parent's name or key names the parent, and so does any of its
    # columns that is no category: dept.dname
    if col.pk or col.unique or col.label or c == tb.label or col.key or (tb.first_last and c in tb.first_last) or not col.category:
        return ("entity", t)
    return ("col", t, c)


def soft_delete(schema: Schema, base: int, filt: str) -> bool:
    """A filter that only leaves out rows deleted softly, which every
    question of round 1c leaves out already."""
    m = re.fullmatch(r"(?i)\s*([\w.]+)\s*(?:is\s+null|=\s*(?:0|false|'n'))\s*", filt)
    if not m:
        return False
    r = ref(schema, m.group(1), base)
    return r is not None and r[0] == base and any(c == r[1] for c, _ in alive(schema.tables[base]))


def identity(schema: Schema, q: dict):
    """A gold question as (base, (aggregate, columns), time, breakdown), or
    None with why when it names what the schema does not hold."""
    base = table_of(schema, q.get("base"))
    if base is None:
        return None, "no such base table"
    m = (q.get("measure") or "").strip()
    per = re.match(r"(?is)^count\s*\(\s*\*\s*\)\s+per\s+(\w+)", m)
    if per:
        p = table_of(schema, per.group(1)) or table_of(schema, per.group(1) + "s")
        fk = [c for c in schema.tables[base].columns if c.fk is not None and c.fk == p]
        if not fk:
            return None, f"no key from {q.get('base')} to {per.group(1)}"
        return (base, ("per", frozenset({(base, fk[0].c)})), None, None), None
    mm = re.match(r"(?is)^(count|sum|avg|average|min|max)\s*\((.*)\)\s*$", m)
    if not mm:
        return None, f"measure not understood: {m}"
    agg = mm.group(1).lower().replace("average", "avg")
    inner = mm.group(2).strip()
    distinct = inner.lower().startswith("distinct")
    if distinct:
        inner = inner[8:].strip()
    cols = [] if inner == "*" or inner == "1" else refs(schema, inner, base)
    if inner not in ("*", "1") and not cols:
        return None, f"measure columns not found: {m}"
    filt = (q.get("filter") or "").strip()
    flag = re.fullmatch(r"(?i)\s*([\w.]+)\s*(?:(?:=|==)\s*(?:1|true|'y'|'yes'|'t')|is\s+true)?\s*", filt) if filt else None
    flag_ref = ref(schema, flag.group(1), base) if flag else None
    if distinct:
        measure = ("distinct", frozenset(cols))
    elif agg == "count" and flag_ref is not None and schema.tables[flag_ref[0]].columns[flag_ref[1]].fam in ("bool", "int"):
        measure = ("count_if", frozenset({flag_ref}))
        filt = ""
    elif agg == "count" and filt and ">" in filt:
        two = refs(schema, filt, base)
        if len(two) == 2 and "and" not in filt.lower():
            measure = ("late", frozenset(two))
            filt = ""
        else:
            measure = ("count", frozenset())
    elif agg == "count":
        measure = ("count", frozenset())
    else:
        measure = (agg, frozenset(cols))
    if filt and not re.fullmatch(r"(?i)[\w.]+\s+is\s+not\s+null", filt) and not soft_delete(schema, base, filt):
        return None, "a filter on the rows"
    t = q.get("time")
    tcol = t.get("column") if isinstance(t, dict) else t
    time = ref(schema, tcol, base) if tcol else None
    if tcol and time is None:
        return None, f"time column not found: {tcol}"
    by = breakdown(schema, base, q.get("breakdown"))
    if by == "unknown":
        return None, f"breakdown not found: {q.get('breakdown')}"
    return (base, measure, time, by), None
