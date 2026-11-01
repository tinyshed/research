"""Round 1q: what PostgreSQL's ANALYZE would keep of every column of CTU's
databases, sampled from CTU's server as stats.py samples SQLite's files.

    .venv/Scripts/python -I ctu_stats.py <host> <port> <databases.txt> <out>

CTU's public account, `guest`, as round-1/fixtures/ctu.sh uses it; the
password is read from CTU_PWD. Writes <out>/ctu/<database>.json in
stats.py's format: about 30 000 rows a table in one scan, by RAND().
"""

import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import pymysql  # noqa: E402

from stats import SAMPLE, analyze, typed  # noqa: E402

BLOB = {"blob", "tinyblob", "mediumblob", "longblob", "binary", "varbinary", "geometry", "point", "polygon"}


def database(con, db: str) -> dict:
    out = {}
    with con.cursor() as cur:
        cur.execute("SELECT table_name, table_rows FROM information_schema.tables "
                    "WHERE table_schema = %s AND table_type = 'BASE TABLE'", (db,))
        for name, _ in cur.fetchall():
            cur.execute("SELECT column_name, data_type FROM information_schema.columns "
                        "WHERE table_schema = %s AND table_name = %s ORDER BY ordinal_position", (db, name))
            cols = [(c, t.lower()) for c, t in cur.fetchall()]
            keep = [c for c, t in cols if t not in BLOB]
            q = ", ".join(f"`{c}`" for c in keep)
            cur.execute(f"SELECT COUNT(*) FROM `{db}`.`{name}`")
            total = cur.fetchone()[0]
            if not keep:
                out[name] = {"rows": total, "columns": {}}
                continue
            if total <= SAMPLE:
                cur.execute(f"SELECT {q} FROM `{db}`.`{name}`")
            else:
                cur.execute(f"SELECT {q} FROM `{db}`.`{name}` WHERE RAND() < {SAMPLE / total}")
            rows = cur.fetchall()
            table = {"rows": total, "columns": {}}
            for i, c in enumerate(keep):
                vals = []
                for r in rows:
                    v = r[i]
                    if v is not None and not isinstance(v, (int, float, str, bytes)):
                        v = str(v)  # dates, decimals: as the text they print as
                    vals.append(typed(v))
                table["columns"][c] = analyze(vals, total) if rows else {
                    "null_frac": 0.0, "n_distinct": 0, "mcv": [], "mcf": [], "hist": []}
            out[name] = table
    return out


def main(host: str, port: int, listing: Path, out: Path) -> None:
    dbs = [d.strip() for d in listing.read_text(encoding="utf-8").splitlines() if d.strip()]
    for db in dbs:
        dest = out / "ctu" / f"{db}.json"
        if dest.exists():
            continue
        t0 = time.time()
        try:
            con = pymysql.connect(host=host, port=port, user="guest", password=os.environ["CTU_PWD"],
                                  connect_timeout=30, read_timeout=900, charset="utf8mb4")
            try:
                stats = database(con, db)
            finally:
                con.close()
        except Exception as e:  # noqa: BLE001 — a database that fails is reported and left out
            print(f"ctu/{db} failed: {str(e)[:150]}", flush=True)
            continue
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(json.dumps(stats, ensure_ascii=False, default=str), encoding="utf-8")
        print(f"ctu/{db} {len(stats)} tables {time.time() - t0:.1f} s", flush=True)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(sys.argv[1], int(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]))
