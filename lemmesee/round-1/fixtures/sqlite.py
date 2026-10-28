"""A SQLite database as one PostgreSQL script: each table created with the
type its declaration names when every value fits it, and text when one does
not, as BIRD's dates kept in text columns stay text; its rows by COPY; its
primary key when the rows keep it; its foreign keys NOT VALID, as SQLite
never checked them, when the parent's key exists and the types meet.

    python -I sqlite.py <file.sqlite> | psql -v ON_ERROR_STOP=1 -d <database>

Only the standard library; the file is read only.
"""

from __future__ import annotations

import datetime as dt
import re
import sqlite3
import sys


def q(name: str) -> str:
    return '"' + name.replace('"', '""') + '"'


def declared(decl: str) -> str:
    d = (decl or "").upper()
    if "INT" in d:
        return "bigint"
    if any(k in d for k in ("REAL", "FLOA", "DOUB", "NUMERIC", "DECIMAL")):
        return "double precision"
    if "DATETIME" in d or "TIMESTAMP" in d:
        return "timestamp"
    if "DATE" in d:
        return "date"
    if "BOOL" in d:
        return "smallint"
    if "BLOB" in d:
        return "bytea"
    return "text"


DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


def fits(kind: str, v) -> bool:
    if v is None:
        return True
    if kind in ("bigint", "smallint"):
        if isinstance(v, int) and not isinstance(v, bool):
            return kind == "bigint" or -32768 <= v <= 32767
        if isinstance(v, float) and v.is_integer():
            return True
        return isinstance(v, str) and re.fullmatch(r"-?\d{1,18}", v.strip()) is not None
    if kind == "double precision":
        if isinstance(v, (int, float)):
            return True
        try:
            float(v)
            return v.strip().lower() not in ("nan", "inf", "-inf", "infinity", "-infinity")
        except (TypeError, ValueError):
            return False
    if kind == "date":
        if not isinstance(v, str) or not DATE.match(v):
            return False
        try:
            dt.date.fromisoformat(v)
            return True
        except ValueError:
            return False
    if kind == "timestamp":
        if not isinstance(v, str) or not re.match(r"^\d{4}-\d{2}-\d{2}", v):
            return False
        try:
            dt.datetime.fromisoformat(v.replace("Z", "+00:00"))
            return True
        except ValueError:
            return False
    if kind == "bytea":
        return isinstance(v, (bytes, bytearray))
    return True


def field(kind: str, v) -> str:
    if v is None:
        return ""
    if isinstance(v, (bytes, bytearray)):
        return '"\\x' + bytes(v).hex() + '"' if kind == "bytea" else '"' + bytes(v).hex() + '"'
    if kind in ("bigint", "smallint") and isinstance(v, float):
        return str(int(v))
    if kind == "timestamp" and isinstance(v, str):
        v = v.replace("Z", "+00:00")
    if isinstance(v, (int, float)) and not isinstance(v, bool) and kind != "text":
        return repr(v) if isinstance(v, float) else str(v)
    s = str(v)
    return '"' + s.replace('"', '""') + '"'


def main(path: str) -> None:
    con = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    con.text_factory = lambda b: b.decode("utf-8", errors="replace")
    tables = [r[0] for r in con.execute("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")]
    by_lower = {t.lower(): t for t in tables}
    kinds: dict[str, dict[str, str]] = {}
    keys: dict[str, list[str]] = {}
    out = sys.stdout
    out.write("SET client_encoding = 'UTF8';\n")
    for t in tables:
        info = con.execute(f"PRAGMA table_info({q(t)})").fetchall()
        cols = [r[1] for r in info]
        kind = {r[1]: declared(r[2]) for r in info}
        # a declaration is kept only when every value fits it
        for c in cols:
            if kind[c] == "text":
                continue
            for (v,) in con.execute(f"SELECT {q(c)} FROM {q(t)}"):
                if not fits(kind[c], v):
                    kind[c] = "double precision" if kind[c] in ("bigint", "smallint") and fits("double precision", v) and all(
                        fits("double precision", w) for (w,) in con.execute(f"SELECT {q(c)} FROM {q(t)}")) else "text"
                    break
        kinds[t] = kind
        pk = [r[1] for r in sorted(info, key=lambda r: r[5]) if r[5] > 0]
        if pk:
            where = " AND ".join(f"{q(c)} IS NOT NULL" for c in pk)
            n, distinct = con.execute(
                f"SELECT (SELECT COUNT(*) FROM {q(t)}), (SELECT COUNT(*) FROM (SELECT DISTINCT {', '.join(q(c) for c in pk)} FROM {q(t)} WHERE {where}))").fetchone()
            if n != distinct:
                pk = []
        keys[t] = pk
        defs = [f"{q(c)} {kind[c]}" for c in cols] + ([f"PRIMARY KEY ({', '.join(q(c) for c in pk)})"] if pk else [])
        out.write(f"CREATE TABLE {q(t)} (\n  " + ",\n  ".join(defs) + "\n);\n")
        out.write(f"COPY {q(t)} ({', '.join(q(c) for c in cols)}) FROM STDIN WITH (FORMAT csv);\n")
        for row in con.execute(f"SELECT {', '.join(q(c) for c in cols)} FROM {q(t)}"):
            out.write(",".join(field(kind[c], v) for c, v in zip(cols, row)) + "\n")
        out.write("\\.\n")
    n_fk = 0
    for t in tables:
        groups: dict[int, list] = {}
        for r in con.execute(f"PRAGMA foreign_key_list({q(t)})").fetchall():
            groups.setdefault(r[0], []).append(r)
        for rows in groups.values():
            parent = by_lower.get(rows[0][2].lower())
            if parent is None:
                continue
            child_cols = [r[3] for r in sorted(rows, key=lambda r: r[1])]
            parent_cols = [r[4] for r in sorted(rows, key=lambda r: r[1])]
            if any(c is None for c in parent_cols):
                parent_cols = keys[parent]
            pcols = {c.lower(): c for c in kinds[parent]}
            ccols = {c.lower(): c for c in kinds[t]}
            parent_cols = [pcols.get((c or "").lower()) for c in parent_cols]
            child_cols = [ccols.get((c or "").lower()) for c in child_cols]
            if None in parent_cols or None in child_cols or [c.lower() for c in parent_cols] != [c.lower() for c in keys[parent]]:
                continue
            if any(kinds[t][a] != kinds[parent][b] for a, b in zip(child_cols, parent_cols)):
                continue
            out.write(f"ALTER TABLE {q(t)} ADD FOREIGN KEY ({', '.join(q(c) for c in child_cols)}) "
                      f"REFERENCES {q(parent)} ({', '.join(q(c) for c in parent_cols)}) NOT VALID;\n")
            n_fk += 1
    print(f"-- {len(tables)} tables, {n_fk} foreign keys", file=sys.stderr)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8", newline="\n")
    main(sys.argv[1])
