"""Round 1k: GitTables as a school for the look. Tables from CSV files in
public GitHub repositories, each read as a database of one table: its
snapshot computed by DuckDB, its questions written by round 1c's generator
and ordered by its ranker, the best run by DuckDB, exactly as round 1d's
pool runs them on PostgreSQL.

    python -I gittables.py survey <raw dir> <out>
    python -I gittables.py select <out> <tables>
    python -I gittables.py build <out> [<at most>]

survey reads every table's GitTables metadata into <out>/survey.jsonl;
select keeps tables of permissive licences, of 50 rows to a million, not
repeating another's columns, at most five a repository, and none whose
columns are half a judged database's table's, then draws <tables> of them
into <out>/selected.json; build writes <out>/see/<fixture>/snapshot.json and
<out>/look/<fixture>/pool.json. Nothing connects to a server.
"""

from __future__ import annotations

import datetime as dt
import decimal
import glob
import json
import math
import random
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
SEED = 20261102
TOP = 10
PER_REPO = 5
PERMISSIVE = re.compile(r"\b(MIT|Apache|BSD|ISC|Unlicense|CC0|Creative Commons Zero|CC-BY-4|Creative Commons Attribution 4"
                        r"|zlib|Boost|Do What The F)", re.I)
UNNAMED = re.compile(r"^(Unnamed: \d+|index|level_0)$", re.I)
NOTE = "A table from a CSV file in a public GitHub repository, {repo}, as GitTables serves it: {path}."


def names(columns: list[str]) -> frozenset[str]:
    return frozenset(re.sub(r"[^a-z0-9]", "", c.lower()) for c in columns if not UNNAMED.match(c))


def survey(raw: Path, out: Path) -> None:
    import duckdb
    con = duckdb.connect()
    files = sorted(glob.glob(str(raw / "*" / "*.parquet")))
    out.mkdir(parents=True, exist_ok=True)
    with (out / "survey.jsonl").open("w", encoding="utf-8") as f:
        for at in range(0, len(files), 2000):
            part = files[at:at + 2000]
            meta = {name: json.loads(bytes(value).decode("utf-8")) for name, value in con.execute(
                "SELECT file_name, value FROM parquet_kv_metadata(?) WHERE key = 'gittables'", [part]).fetchall()}
            cols: dict[str, list] = {}
            for name, col, kind in con.execute(
                    "SELECT file_name, name, type FROM parquet_schema(?) WHERE num_children IS NULL "
                    "OR num_children = 0", [part]).fetchall():
                cols.setdefault(name, []).append([col, kind])
            for name in part:
                m = meta.get(name) or {}
                f.write(json.dumps({"file": str(Path(name).relative_to(raw)).replace("\\", "/"),
                                    "license": m.get("license"), "csv_url": m.get("csv_url"),
                                    "rows": m.get("number_rows"), "columns": [c for c, _ in cols.get(name, [])
                                                                              if c != "__index_level_0__"]},
                                   ensure_ascii=False) + "\n")
            print(f"{at + len(part)} of {len(files)}", flush=True)


def judged_columns() -> list[frozenset[str]]:
    """Every table of the databases the judges count, as a set of names."""
    out = []
    for snap in (ROOT / "corpus" / "see").glob("*/snapshot.json"):
        for t in json.loads(snap.read_text(encoding="utf-8"))["tables"]:
            s = names([c["name"] for c in t["columns"]])
            if len(s) >= 2:
                out.append(s)
    return out


def repo_of(url: str | None) -> str:
    m = re.match(r"https://github\.com/([^/]+/[^/]+)/", url or "")
    return m.group(1) if m else "?"


def select(out: Path, count: int) -> None:
    rows = [json.loads(line) for line in (out / "survey.jsonl").read_text(encoding="utf-8").splitlines()]
    judged = judged_columns()
    seen, per_repo, kept = set(), {}, []
    why = {"licence": 0, "size": 0, "columns": 0, "repeat": 0, "repository": 0, "judged": 0}
    rng = random.Random(SEED)
    rng.shuffle(rows)
    for r in rows:
        cols = [c for c in r["columns"] if not UNNAMED.match(c)]
        s = names(cols)
        if not PERMISSIVE.search(r.get("license") or ""):
            why["licence"] += 1
        elif not (50 <= (r.get("rows") or 0) <= 1_000_000):
            why["size"] += 1
        elif not (3 <= len(s) <= 40) or not headed(cols):
            why["columns"] += 1
        elif s in seen:
            why["repeat"] += 1
        elif per_repo.get(repo_of(r["csv_url"]), 0) >= PER_REPO:
            why["repository"] += 1
        elif any(len(s & j) / len(s | j) >= 0.5 for j in judged):
            why["judged"] += 1
        else:
            seen.add(s)
            per_repo[repo_of(r["csv_url"])] = per_repo.get(repo_of(r["csv_url"]), 0) + 1
            kept.append(r)
    chosen = kept[:count]
    for n, r in enumerate(chosen, 1):
        r["fixture"] = f"gt{n:05d}-duckdb"
    (out / "selected.json").write_text(json.dumps({"seed": SEED, "left_out": why, "eligible": len(kept),
                                                   "tables": chosen}, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"{len(rows)} surveyed, left out {why}, {len(kept)} eligible, {len(chosen)} chosen")


# ---- a table as a database

OPAQUE = re.compile(r"^([0-9a-f]{8,}|[0-9a-f-]{20,}|[\d_.\-]+|v?\d+|data|csv|files?|output|raw|tmp)$", re.I)


def table_name(url: str | None, file: str) -> str:
    """The file's own name, or, where it is a hash, a date or a number, the
    nearest folder named in words, as a database would name the table."""
    parts = [p for p in (url or file).split("?")[0].split("/blob/", 1)[-1].split("/")[1:]]
    for part in reversed(parts):
        stem = re.sub(r"\.(csv|tsv|txt)$", "", part, flags=re.I)
        name = re.sub(r"[^a-z0-9]+", "_", stem.lower()).strip("_")
        words = [w for w in name.split("_") if w]
        if name and not OPAQUE.match(stem.replace("_", "")) and sum(len(w) >= 3 and not w.isdigit() for w in words) \
                and not re.search(r"[0-9a-f]{8}", name):
            return (name if not name[0].isdigit() else f"t_{name}")[:40]
    return "data"


def headed(columns: list[str]) -> bool:
    """Whether the first row was a header: a name is not a number, a sentence
    or a line of code."""
    def value_like(c: str) -> bool:
        c = c.strip()
        return (bool(re.fullmatch(r"[-+]?[\d.,eE%$ ]+", c)) or len(c) > 40 or len(c.split()) > 5
                or bool(re.search(r"[{}();<>\\]|//|0x[0-9a-f]", c, re.I)))
    named = [c for c in columns if not UNNAMED.match(c)]
    return bool(named) and sum(value_like(c) for c in named) <= len(named) / 4


PG_TYPE = {"BIGINT": "bigint", "INTEGER": "integer", "SMALLINT": "smallint", "TINYINT": "smallint", "HUGEINT": "numeric",
           "DOUBLE": "double precision", "FLOAT": "real", "VARCHAR": "text", "BOOLEAN": "boolean", "DATE": "date",
           "TIMESTAMP": "timestamp without time zone", "TIMESTAMP WITH TIME ZONE": "timestamp with time zone"}


def pg_type(kind: str) -> str:
    if kind.startswith("DECIMAL"):
        return kind.replace("DECIMAL", "numeric")
    return PG_TYPE.get(kind, "text")


def open_table(con, path: Path, name: str) -> list[tuple[str, str]]:
    """The table as a view, a column of text that reads as times read as
    times, as a database that typed them would hold them."""
    cols = [(c, k) for c, k, *_ in con.execute("DESCRIBE SELECT * FROM read_parquet(?)", [str(path)]).fetchall()
            if not UNNAMED.match(c) and c != "__index_level_0__"]
    select = []
    for c, k in cols:
        q = '"' + c.replace('"', '""') + '"'
        if k == "VARCHAR":
            share, midnight = con.execute(
                f"SELECT avg((TRY_CAST({q} AS TIMESTAMP) IS NOT NULL)::int), "
                f"avg((TRY_CAST({q} AS TIMESTAMP)::time = TIME '00:00:00')::int) "
                f"FROM read_parquet(?) WHERE {q} IS NOT NULL AND length({q}) >= 8", [str(path)]).fetchone()
            total = con.execute(f"SELECT count({q}) FROM read_parquet(?)", [str(path)]).fetchone()[0]
            hits = con.execute(f"SELECT count(*) FROM read_parquet(?) WHERE length({q}) >= 8", [str(path)]).fetchone()[0]
            if total and share is not None and share >= 0.95 and hits >= 0.95 * total:
                kind = "DATE" if (midnight or 0) >= 0.999 else "TIMESTAMP"
                select.append(f"TRY_CAST({q} AS {kind}) AS {q}")
                continue
        select.append(q)
    con.execute(f'CREATE OR REPLACE VIEW "{name}" AS SELECT {", ".join(select)} FROM read_parquet(\'{path.as_posix()}\')')
    return [(c, k) for c, k, *_ in con.execute(f'DESCRIBE "{name}"').fetchall()]


def snapshot(con, fixture: str, name: str, cols: list[tuple[str, str]], note: str) -> dict:
    """The fields round 1's snapshot reads from pg_stats, computed exactly."""
    rows = con.execute(f'SELECT count(*) FROM "{name}"').fetchone()[0]
    columns = []
    for pos, (c, k) in enumerate(cols, 1):
        q = '"' + c.replace('"', '""') + '"'
        distinct, nulls, top = con.execute(
            f'SELECT count(DISTINCT {q}), count(*) - count({q}), '
            f'(SELECT count(*) FROM "{name}" WHERE {q} IS NOT NULL GROUP BY {q} ORDER BY 1 DESC LIMIT 1) FROM "{name}"'
        ).fetchone()
        top_freq = (top / rows) if rows and top and top > 1 and distinct < rows - nulls else None

        def stat(v):
            return {"value": v, "state": "present", "source": "duckdb"}
        columns.append({"name": c, "position": pos, "type_name": pg_type(k), "base_type": pg_type(k).split("(")[0],
                        "nullable": True, "can_select": True,
                        "features": {"n_distinct": stat(distinct), "null_frac": stat(nulls / rows if rows else 0.0),
                                     "mcv_top_freq": stat(top_freq)}})
    return {"version": "round-1k", "engine": "postgresql", "database": fixture, "note": note,
            "tables": [{"schema": "public", "name": name, "kind": "table", "can_select": True,
                        "features": {"rows_estimate": {"value": rows, "state": "present", "source": "duckdb"}},
                        "columns": columns, "constraints": [], "indexes": []}]}


def text(v) -> str | None:
    """A value as psql -At prints it."""
    if v is None:
        return None
    if isinstance(v, bool):
        return "t" if v else "f"
    if isinstance(v, dt.datetime):
        return v.isoformat(sep=" ")
    if isinstance(v, (dt.date, decimal.Decimal, int)):
        return str(v)
    if isinstance(v, float):
        return repr(v) if math.isfinite(v) else ("NaN" if v != v else ("Infinity" if v > 0 else "-Infinity"))
    return str(v)


def build(out: Path, at_most: int | None = None) -> None:
    import duckdb
    look1d = _look1d()
    see = look1d.see
    selected = json.loads((out / "selected.json").read_text(encoding="utf-8"))["tables"][:at_most]
    raw = ROOT / "corpus" / "gittables" / "raw"
    current: dict = {}

    def run_sql(engine: str, database: str, sql: str):
        import time
        t = time.perf_counter()
        try:
            rows = [[text(v) for v in r] for r in current["con"].execute(sql).fetchmany(501)]
            return rows, None, (time.perf_counter() - t) * 1000
        except Exception as e:  # noqa: BLE001 — kept as the panel's error, as psql's would be
            return [], str(e).splitlines()[0][:300], (time.perf_counter() - t) * 1000

    see.run_sql = run_sql
    done = errors = panels = few = 0
    for r in selected:
        fe = r["fixture"]
        if (out / "look" / fe / "pool.json").exists():
            continue
        con = duckdb.connect()
        con.execute("SET threads = 4")
        name = table_name(r["csv_url"], r["file"])
        try:
            cols = open_table(con, raw / r["file"], name)
            m = re.match(r"https://github\.com/([^/]+/[^/]+)/blob/[^/]+/(.*)", r["csv_url"] or "")
            note = NOTE.format(repo=m.group(1), path=m.group(2)) if m else NOTE.format(repo="?", path=r["file"])
            snap = snapshot(con, fe, name, cols, note)
        except Exception as e:  # noqa: BLE001 — a table DuckDB cannot read is left out and said
            print(f"{fe}: unreadable, {str(e).splitlines()[0][:120]}", flush=True)
            con.close()
            continue
        schema = look1d.load(snap, True)
        asked = [q for q in look1d.generate(schema) if look1d.rank.veto(schema, q) is None]
        if len(asked) < 5:
            # a table the generator can ask fewer than five questions of makes no five
            few += 1
            con.close()
            continue
        (out / "see" / fe).mkdir(parents=True, exist_ok=True)
        (out / "see" / fe / "snapshot.json").write_text(json.dumps(snap, ensure_ascii=False), encoding="utf-8")
        (out / "see" / fe / "runs.json").write_text(json.dumps({"note": note, "panels": [], "configs": {}}),
                                                    encoding="utf-8")
        current["con"] = con
        look1d.pool(ROOT / "corpus" / "round-1c" / "learn", ROOT / "corpus" / "round-1d", out / "see",
                    out / "look", [fe], TOP)
        con.close()
        pool = out / "look" / fe / "pool.json"
        if pool.exists():
            ps = json.loads(pool.read_text(encoding="utf-8"))["panels"]
            panels += len(ps)
            errors += sum(1 for p in ps if p.get("error"))
        done += 1
    print(f"{done} tables built, {panels} panels, {errors} with an error; {few} left out with fewer than five questions")


def _look1d():
    spec_path = ROOT / "round-1d" / "look.py"
    import importlib.util
    spec = importlib.util.spec_from_file_location("look1d", spec_path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["look1d"] = mod
    spec.loader.exec_module(mod)
    return mod


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, *args = sys.argv[1:]
    if cmd == "survey":
        survey(Path(args[0]), Path(args[1]))
    elif cmd == "select":
        select(Path(args[0]), int(args[1]))
    elif cmd == "build":
        build(Path(args[0]), int(args[1]) if len(args) > 1 else None)
