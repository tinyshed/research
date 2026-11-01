"""Round 1q: what PostgreSQL's ANALYZE would keep of every column of Spider's
and BIRD's SQLite files, at default_statistics_target 100.

    .venv/Scripts/python -I stats.py <corpus> <out>

writes <out>/<source>/<database>.json: for each table its row count, and for
each column the null fraction, the distinct count as ANALYZE estimates it
(negative as a share of the rows, −1 for every row distinct), the most common
values with their frequencies and the histogram bounds of the rest — the
fields of `pg_stats` a later step may read, and nothing else.

The sample is every row of a table of at most 30 000 rows, else about 30 000
drawn at random in one scan; SQLite's loose types are read as PostgreSQL's
typed columns would hold them, a text of digits as the integer it spells.
"""

import json
import random
import sqlite3
import sys
import time
from collections import Counter
from pathlib import Path

SAMPLE = 30000  # 300 x default_statistics_target
TARGET = 100  # most common values kept, and histogram buckets


def typed(v):
    """A value as a typed column would hold it; None for a blob."""
    if isinstance(v, bytes):
        return None
    if isinstance(v, str):
        s = v.strip()
        if s.lstrip("-").isdigit() and len(s) < 19:
            return int(s)
        return v
    if isinstance(v, float) and v.is_integer() and abs(v) < 2 ** 53:
        return int(v)
    return v


def order(v):
    """Numbers before text, each in its own order."""
    return (0, v, "") if isinstance(v, (int, float)) else (1, 0, v)


def analyze(values: list, total: int) -> dict:
    """pg_stats' fields for one column from its sample, as ANALYZE derives them."""
    n = len(values)
    present = [v for v in values if v is not None]
    if not present:
        return {"null_frac": 1.0, "n_distinct": 0, "mcv": [], "mcf": [], "hist": []}
    counts = Counter(present)
    d, f1, nn = len(counts), sum(1 for c in counts.values() if c == 1), len(present)
    if f1 == nn:
        n_distinct = -1.0  # every sampled value distinct: assumed unique
    elif f1 == 0 and nn == n == total:
        n_distinct = float(d)
    else:
        # Haas and Stokes' Duj1, as analyze.c computes it
        est = nn * d / (nn - f1 + f1 * nn / max(total * nn / n, nn))
        est = min(max(est, d), total)
        n_distinct = -est / total if est > 0.1 * total else float(round(est))
    mean = nn / d
    common = [(v, c) for v, c in counts.most_common() if c > 1 and c >= 1.25 * mean]
    if f1 == 0 and d <= TARGET:
        common = counts.most_common()  # every value fits the list
    common = common[:TARGET]
    kept = {v for v, _ in common}
    rest = sorted((v for v in present if v not in kept), key=order)
    hist = []
    k = min(TARGET + 1, len(set(rest)))
    if k >= 2:
        # equi-depth bounds of the rest, each value once, as pg_stats lists them
        bounds = [rest[round(i * (len(rest) - 1) / (k - 1))] for i in range(k)]
        hist = list(dict.fromkeys(bounds))
    return {"null_frac": 1 - nn / n, "n_distinct": n_distinct,
            "mcv": [v for v, _ in common], "mcf": [c / n for _, c in common], "hist": hist}


def database(path: Path) -> dict:
    con = sqlite3.connect(path.resolve().as_uri() + "?immutable=1", uri=True)
    con.text_factory = lambda b: b.decode("utf-8", errors="replace")
    out = {}
    try:
        for (name,) in con.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall():
            if name.lower().startswith("sqlite_"):
                continue
            q = name.replace('"', '""')
            cols = [c[1] for c in con.execute(f'PRAGMA table_info("{q}")').fetchall()]
            total = con.execute(f'SELECT count(*) FROM "{q}"').fetchone()[0]
            if total <= SAMPLE:
                rows = con.execute(f'SELECT * FROM "{q}"').fetchall()
            else:
                rows = con.execute(f'SELECT * FROM "{q}" WHERE abs(random()) % {total} < {SAMPLE}').fetchall()
            table = {"rows": total, "columns": {}}
            for i, c in enumerate(cols):
                vals = [typed(r[i]) for r in rows]
                if any(isinstance(r[i], bytes) for r in rows[:200]):
                    continue
                table["columns"][c] = analyze(vals, total) if rows else {
                    "null_frac": 0.0, "n_distinct": 0, "mcv": [], "mcf": [], "hist": []}
            out[name] = table
    finally:
        con.close()
    return out


def main(corpus: Path, out: Path) -> None:
    random.seed(20261102)
    files = [("spider", p) for p in sorted((corpus / "spider/spider_data/test_database").glob("*/*.sqlite"))]
    for part in ("dev/dev_databases", "train/train_databases"):
        files += [("bird", p) for p in sorted((corpus / "bird/raw" / part).glob("*/*.sqlite"))]
    for src, p in files:
        dest = out / src / f"{p.stem}.json"
        if dest.exists():
            continue
        t0 = time.time()
        try:
            stats = database(p)
        except sqlite3.DatabaseError as e:
            print(f"{src}/{p.stem} failed: {e}", flush=True)
            continue
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(json.dumps(stats, ensure_ascii=False, default=str), encoding="utf-8")
        print(f"{src}/{p.stem} {len(stats)} tables {time.time() - t0:.1f} s", flush=True)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]))
