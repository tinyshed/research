"""Round 1x: every row of Spider's and BIRD's databases read once, for the
row methods of the round's README.

    python -I rows.py profile <out folder> [spider|bird] [database ...]

For each database of round 1v's kept sets that declares a one-column key,
writes <out>/profile/<source>/<db>.json:

- the rows: each table's rows, each column's non-null values, its storage
  class, whether the schema gives it as a key (K) and whether the data do (U);
- every unary inclusion into a column of K and into a column of U, found
  exactly (README, "Inclusion"), each with what the methods read from the
  values: distinct counts, Zhang et al.'s quantile earth mover's distance,
  HoPF's Bhattacharyya coefficient, whether the dependent values are
  consecutive among the referenced ones, Rostin et al.'s F7 and F8;
- each declared one-column key: whether its column is empty and whether the
  data keep it;
- for the two hybrid rules, the inclusion of each column's best-scored
  candidate of lemmekey (`eleven--wide`'s kept scores);
- wall time of each phase and the rows read.

A database is skipped when its file is already written. BIRD's files not
unpacked in corpus/bird/raw/ are extracted to <out>/tmp one at a time and
deleted. Files are opened immutable; nothing is written beside them.
"""

import bisect
import json
import math
import pickle
import re
import shutil
import sqlite3
import sys
import time
import zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent / "corpus"
sys.path.insert(0, str(HERE.parent / "round-1v"))

import numpy as np  # noqa: E402

import whole  # noqa: E402  (round 1v's reading of a kept set)

MODEL = "eleven--wide"
QUANTILES = 256  # Zhang et al., section 5: 256 quantiles for single-column keys
BUCKETS = 20     # Jiang and Naumann, section 4.2
PREFIX = 10_000  # rows looked at before a column is counted whole for uniqueness
COPY_FROM = 1_000  # a referenced column with no index is copied past this many rows


def q(name: str) -> str:
    return '"' + name.replace('"', '""') + '"'


# --- where a database's file is ----------------------------------------------

def bird_members() -> dict:
    out = {}
    for part in ("train/train_databases.zip", "dev/dev_databases.zip"):
        z = ROOT / "bird/raw" / part
        with zipfile.ZipFile(z) as zf:
            for info in zf.infolist():
                if info.filename.startswith("__MACOSX") or not info.filename.endswith(".sqlite"):
                    continue
                out[Path(info.filename).stem] = (z, info.filename, info.file_size)
    return out


def locate(source: str, db: str, members: dict, tmp: Path) -> tuple[Path, bool, int]:
    """The file, whether it was extracted for this run, and its size."""
    if source == "spider":
        p = ROOT / "spider/spider_data/test_database" / db / f"{db}.sqlite"
        return p, False, p.stat().st_size
    z, member, size = members[db]
    p = z.parent / member
    if p.exists() and p.stat().st_size == size:
        return p, False, size
    tmp.mkdir(parents=True, exist_ok=True)
    out = tmp / f"{db}.sqlite"
    with zipfile.ZipFile(z) as zf, zf.open(member) as src, open(out, "wb") as dst:
        shutil.copyfileobj(src, dst, 1 << 24)
    return out, True, size


# --- values as the methods order them ----------------------------------------

def number(s: str):
    try:
        f = float(s)
    except ValueError:
        return None
    if math.isnan(f) or math.isinf(f):
        return None
    return int(f) if f == int(f) and abs(f) < 2 ** 53 else f


def canon(v, numeric: bool):
    """A value as a sortable pair: numbers numerically before strings
    lexicographically before blobs. A string that reads as a number is one
    when either column is numeric, as SQLite's comparison makes it."""
    if isinstance(v, (int, float)):
        return (0, v)
    if isinstance(v, str):
        if numeric:
            n = number(v)
            if n is not None:
                return (0, n)
        return (1, v)
    return (2, bytes(v))


def as_text(v) -> str:
    if isinstance(v, bytes):
        return v.decode("utf-8", "replace")
    return str(v)


# --- one database -------------------------------------------------------------

class Database:
    def __init__(self, path: Path, schema: dict):
        self.con = sqlite3.connect(path.resolve().as_uri() + "?immutable=1", uri=True)
        self.con.execute("PRAGMA temp_store=MEMORY")
        self.con.execute("PRAGMA cache_size=-1048576")
        self.con.text_factory = lambda b: b.decode("utf-8", "replace")
        self.schema = schema
        self.tables = schema["tables"]
        self.info = []       # per table: rows, sql has a collation, columns' statistics
        self.copies = {}     # (table, column) -> a temporary table holding its non-null values, indexed
        self.tests = {}      # (ti, ci, tj, cj) -> "holds" | "fails" | "empty"
        self.counts = {"tests run": 0, "tests pruned by range": 0, "copies": 0}
        self.check()

    def check(self) -> None:
        names = {r[0]: r[1] or "" for r in self.con.execute("SELECT name, sql FROM sqlite_master WHERE type='table'")}
        self.collated = {}
        self.decl = {}
        self.indexed = {}
        for t in self.tables:
            assert t["name"] in names, f"{self.schema['id']}: no table {t['name']} in the file"
            cols = self.con.execute(f"PRAGMA table_info({q(t['name'])})").fetchall()
            assert [c[1] for c in cols] == [c["name"] for c in t["columns"]], \
                f"{self.schema['id']}.{t['name']}: the file's columns are not the kept schema's"
            self.collated[t["name"]] = "collate" in names[t["name"]].lower()
            self.decl[t["name"]] = [c[2] or "" for c in cols]
            # a column leading an index of its own, or the rowid: the test's lookup needs no copy
            lead = set()
            for _, iname, *_ in self.con.execute(f"PRAGMA index_list({q(t['name'])})").fetchall():
                first = self.con.execute(f"PRAGMA index_info({q(iname)})").fetchall()
                if first:
                    lead.add(first[0][2])
            pk = [c for c in cols if c[5]]
            if len(pk) == 1 and (pk[0][2] or "").upper() == "INTEGER":
                lead.add(pk[0][1])
            self.indexed[t["name"]] = lead
        self.extra_tables = sorted(set(names) - {t["name"] for t in self.tables})

    # the rows: one scan a table
    def statistics(self) -> int:
        cells = 0
        for t in self.tables:
            parts = ["COUNT(*)"]
            for c in t["columns"]:
                n = q(c["name"])
                parts += [f"COUNT({n})", f"MIN({n})", f"MAX({n})", f"typeof(MIN({n}))", f"typeof(MAX({n}))"]
            row = self.con.execute(f"SELECT {', '.join(parts)} FROM {q(t['name'])}").fetchone()
            cols = []
            for i, c in enumerate(t["columns"]):
                nn, lo, hi, tlo, thi = row[1 + 5 * i: 6 + 5 * i]
                if nn == 0:
                    cls = "empty"
                elif tlo in ("integer", "real") and thi in ("integer", "real"):
                    cls = "num"
                elif tlo == "text" and thi == "text":
                    cls = "text"
                else:
                    cls = "mixed"
                key = bool(c["unique"]) or (c["primary"] and len(t["pk"]) == 1)
                cols.append({"name": c["name"], "nonnull": nn, "class": cls, "min": lo, "max": hi, "K": bool(key),
                             "U": False})
            self.info.append({"name": t["name"], "rows": row[0], "columns": cols})
            cells += row[0] * len(t["columns"])
        return cells

    # U: the columns the data make keys
    def unique_columns(self) -> None:
        for t in self.info:
            for c in t["columns"]:
                if t["rows"] == 0 or c["nonnull"] != t["rows"]:
                    continue
                if c["K"]:
                    c["U"] = True  # a declared key with no null cannot repeat
                    continue
                n, tn = q(c["name"]), q(t["name"])
                if t["rows"] > PREFIX:
                    seen = self.con.execute(f"SELECT COUNT(DISTINCT {n}) FROM (SELECT {n} FROM {tn} LIMIT {PREFIX})").fetchone()[0]
                    if seen < PREFIX:
                        continue
                c["U"] = self.con.execute(f"SELECT COUNT(DISTINCT {n}) FROM {tn}").fetchone()[0] == t["rows"]

    def reference(self, tj: int, cj: int) -> tuple[str, str, bool]:
        """What the test looks a value up in: the table itself when the
        column has an index or is small, else an indexed temporary copy."""
        t, c = self.info[tj], self.info[tj]["columns"][cj]
        nulls = c["nonnull"] != t["rows"]
        if c["name"] in self.indexed[t["name"]] or t["rows"] <= COPY_FROM:
            return q(t["name"]), q(c["name"]), nulls
        if (tj, cj) not in self.copies:
            name = f"ref_{tj}_{cj}"
            decl = re.sub(r"[^A-Za-z0-9 (),]", "", self.decl[t["name"]][cj])
            self.con.execute(f"CREATE TEMP TABLE {name} (k {decl})")
            self.con.execute(f"INSERT INTO {name} SELECT {q(c['name'])} FROM {q(t['name'])} WHERE {q(c['name'])} IS NOT NULL")
            self.con.execute(f"CREATE INDEX temp.{name}_i ON {name} (k)")
            self.copies[(tj, cj)] = name
            self.counts["copies"] += 1
        return "temp." + self.copies[(tj, cj)], "k", False

    def included(self, ti: int, ci: int, tj: int, cj: int) -> str:
        key = (ti, ci, tj, cj)
        if key in self.tests:
            return self.tests[key]
        a, b = self.info[ti]["columns"][ci], self.info[tj]["columns"][cj]
        if a["nonnull"] == 0:
            res = "empty"
        elif b["nonnull"] == 0:
            res = "fails"
        elif (a["class"] == b["class"] and a["class"] in ("num", "text")
              and not (a["class"] == "text" and (self.collated[self.info[ti]["name"]] or self.collated[self.info[tj]["name"]]))
              and (a["min"] < b["min"] or a["max"] > b["max"])):
            res = "fails"
            self.counts["tests pruned by range"] += 1
        else:
            rt, rc, nulls = self.reference(tj, cj)
            tn, an = q(self.info[ti]["name"]), q(a["name"])
            sub = f"SELECT {rc} FROM {rt}" + (f" WHERE {rc} IS NOT NULL" if nulls else "")
            row = self.con.execute(f"SELECT 1 FROM {tn} AS dep WHERE dep.{an} IS NOT NULL AND dep.{an} NOT IN ({sub}) LIMIT 1").fetchone()
            res = "fails" if row else "holds"
            self.counts["tests run"] += 1
        self.tests[key] = res
        return res

    def inclusions(self, which: str, skip: set) -> list:
        """Every inclusion into a referenced column of K or of U."""
        refs = [(tj, cj) for tj, t in enumerate(self.info) for cj, c in enumerate(t["columns"]) if c[which]]
        out = []
        for ti, t in enumerate(self.info):
            for ci, c in enumerate(t["columns"]):
                if c["nonnull"] == 0:
                    continue
                for tj, cj in refs:
                    if (ti, ci) == (tj, cj):
                        continue
                    if (ti, ci, tj, cj) in skip:
                        out.append((ti, ci, tj, cj))
                    elif self.included(ti, ci, tj, cj) == "holds":
                        out.append((ti, ci, tj, cj))
        return out

    # what the methods read from the values of an inclusion
    def values(self, pairs: list) -> dict:
        out = {}
        by_ref = {}
        for ti, ci, tj, cj in pairs:
            by_ref.setdefault((tj, cj), []).append((ti, ci))
        deps = {}
        fetched = 0

        def dep_values(ti, ci):
            nonlocal fetched
            if (ti, ci) not in deps:
                t, c = self.info[ti], self.info[ti]["columns"][ci]
                n = q(c["name"])
                rows = self.con.execute(f"SELECT {n}, COUNT(*) FROM {q(t['name'])} WHERE {n} IS NOT NULL GROUP BY {n}").fetchall()
                fetched += len(rows)
                deps[(ti, ci)] = rows
            return deps[(ti, ci)]

        dep_len = {}
        for (tj, cj), ds in by_ref.items():
            t, c = self.info[tj], self.info[tj]["columns"][cj]
            n = q(c["name"])
            raw = [r[0] for r in self.con.execute(f"SELECT DISTINCT {n} FROM {q(t['name'])} WHERE {n} IS NOT NULL")]
            fetched += len(raw)
            ref_len = float(np.mean([len(as_text(v)) for v in raw]))
            sorted_ref = {}
            for ti, ci in ds:
                a = self.info[ti]["columns"][ci]
                numeric = "num" in (a["class"], c["class"])
                if numeric not in sorted_ref:
                    ref = sorted({canon(v, numeric) for v in raw})
                    ints = all(k[0] == 0 and isinstance(k[1], int) and abs(k[1]) < 2 ** 62 for k in ref)
                    sorted_ref[numeric] = (ref, np.array([k[1] for k in ref], dtype=np.int64) if ints else None)
                dep = {}
                for v, k in dep_values(ti, ci):
                    cv = canon(v, numeric)
                    dep[cv] = dep.get(cv, 0) + k
                if (ti, ci) not in dep_len:
                    dep_len[(ti, ci)] = float(np.mean([len(as_text(v)) for v, _ in dep_values(ti, ci)]))
                out[(ti, ci, tj, cj)] = features(*sorted_ref[numeric], dep, ref_len, dep_len[(ti, ci)])
        self.counts["values fetched"] = self.counts.get("values fetched", 0) + fetched
        return out

    def close(self) -> None:
        self.con.close()


def features(ref: list, ref_ints, dep: dict, ref_length: float, dep_length: float) -> dict:
    """ref: the referenced column's distinct values, sorted (and as an
    array when all are integers); dep: the dependent column's distinct
    values with their rows."""
    nb, na = len(ref), len(dep)
    keys = list(dep)
    weights = np.array([dep[k] for k in keys], dtype=np.float64)
    if ref_ints is not None and all(k[0] == 0 and isinstance(k[1], int) and abs(k[1]) < 2 ** 62 for k in keys):
        ranks = np.searchsorted(ref_ints, np.array([k[1] for k in keys], dtype=np.int64))
    else:
        ranks = np.array([bisect.bisect_left(ref, k) for k in keys], dtype=np.int64)
    ranks = np.minimum(ranks, nb - 1)
    # Zhang et al.: P's equal-depth histogram, F's distinct values in P's cells, the quantile distance
    lam = min(QUANTILES, nb)
    p = np.bincount((np.arange(nb) * lam) // nb, minlength=lam) / nb
    f = np.bincount((ranks * lam) // nb, minlength=lam) / na
    emd = float(np.abs(np.cumsum(f) - np.cumsum(p)).sum() / lam)
    # HoPF: twenty buckets over the referenced column's value range, every row of the dependent column
    numeric = all(k[0] == 0 for k in ref) and all(k[0] == 0 for k in dep)
    if numeric:
        lo, hi = float(ref[0][1]), float(ref[-1][1])
        width = hi - lo

        def bucket(vals):
            if width <= 0:
                return np.zeros(len(vals), dtype=np.int64)
            x = (np.array(vals, dtype=np.float64) - lo) / width * BUCKETS
            return np.clip(np.floor(x), 0, BUCKETS - 1).astype(np.int64)

        hp = np.bincount(bucket([k[1] for k in ref]), minlength=BUCKETS).astype(np.float64)
        hf = np.bincount(bucket([k[1] for k in keys]), weights=weights, minlength=BUCKETS)
    else:
        hp = np.bincount((np.arange(nb) * BUCKETS) // nb, minlength=BUCKETS).astype(np.float64)
        hf = np.bincount((ranks * BUCKETS) // nb, weights=weights, minlength=BUCKETS)
    bc = float(np.sqrt((hp / hp.sum()) * (hf / hf.sum())).sum())
    lo_rank, hi_rank = int(ranks.min()), int(ranks.max())
    return {"dependent distinct": na, "referenced distinct": nb, "dependent rows": float(weights.sum()),
            "emd": emd, "bhattacharyya": bc, "histogram by": "value" if numeric else "rank",
            "consecutive": hi_rank - lo_rank + 1 == na,
            "f7 length difference": dep_length - ref_length,
            "f8 out of range": (lo_rank + (nb - 1 - hi_rank)) / nb}


def candidates(name: str) -> dict:
    """lemmekey's best-scored candidate of each column: {schema id: [(table, column, target, key column, score)]}."""
    sc = whole.Scored(MODEL, name)
    out = {}
    for si, (a, b) in sc.spans.items():
        best = {}
        for i in range(a, b):
            k = (int(sc.table[i]), int(sc.column[i]))
            if k not in best or sc.second[i] > sc.second[best[k]]:
                best[k] = i
        out[sc.schemas[si]["id"]] = [(ti, ci, int(sc.target[i]), int(sc.key[i]), float(sc.second[i]))
                                     for (ti, ci), i in best.items()]
    return out


def profile(path: Path, schema: dict, cands: list) -> dict:
    times = {}
    t0 = time.perf_counter()
    db = Database(path, schema)
    cells = db.statistics()
    in_k = db.inclusions("K", set())
    times["statistics and inclusions into K"] = time.perf_counter() - t0
    counts_k = dict(db.counts)

    t0 = time.perf_counter()
    db.unique_columns()
    in_u = db.inclusions("U", set(in_k))
    times["unique columns and inclusions into U"] = time.perf_counter() - t0

    t0 = time.perf_counter()
    feats = db.values(sorted(set(in_k) | set(in_u)))
    times["values for the distributions and features"] = time.perf_counter() - t0

    t0 = time.perf_counter()
    before = db.counts["tests run"]
    hybrid = [[ti, ci, tj, kj, score, db.included(ti, ci, tj, kj)] for ti, ci, tj, kj, score in cands]
    times["tests of lemmekey's best candidates"] = time.perf_counter() - t0
    hybrid_tests = db.counts["tests run"] - before

    # the labels: is a declared key kept by the data
    index = {t["name"]: i for i, t in enumerate(db.info)}
    declared = []
    for ti, t in enumerate(db.info):
        groups = {}
        for r in db.con.execute(f"PRAGMA foreign_key_list({q(t['name'])})").fetchall():
            groups.setdefault(r[0], []).append(r)
        for rows in groups.values():
            if len(rows) != 1:
                continue
            _, _, target, col, to = rows[0][:5]
            tj = next((j for n, j in index.items() if n.lower() == target.lower()), None)
            ci = next((i for i, c in enumerate(t["columns"]) if c["name"].lower() == col.lower()), None)
            entry = {"table": t["name"], "column": col, "target": target, "referenced": to}
            if tj is None or ci is None:
                entry["state"] = "the target is not a table of the schema"
            else:
                tt = db.info[tj]
                if to is None:
                    pk = schema["tables"][tj]["pk"]
                    to = pk[0] if len(pk) == 1 else None
                cj = next((i for i, c in enumerate(tt["columns"]) if to and c["name"].lower() == to.lower()), None)
                entry.update({"target": tt["name"], "column": t["columns"][ci]["name"],
                              "dependent non-null": t["columns"][ci]["nonnull"]})
                if cj is None:
                    entry["state"] = "no referenced column"
                else:
                    entry["referenced"] = tt["columns"][cj]["name"]
                    entry["referenced in K"], entry["referenced in U"] = tt["columns"][cj]["K"], tt["columns"][cj]["U"]
                    entry["state"] = db.included(ti, ci, tj, cj)
            declared.append(entry)

    def name(ti, ci):
        return [db.info[ti]["name"], db.info[ti]["columns"][ci]["name"]]

    def listed(pairs):
        return [[ti, ci, tj, cj] for ti, ci, tj, cj in pairs]

    out = {"id": schema["id"], "bytes": path.stat().st_size,
           "tables": [{"name": t["name"], "rows": t["rows"],
                       "columns": [{k: c[k] for k in ("name", "nonnull", "class", "K", "U")} for c in t["columns"]]}
                      for t in db.info],
           "tables of the file outside the kept schema": db.extra_tables,
           "rows": sum(t["rows"] for t in db.info), "cells": cells,
           "inclusions into K": listed(in_k), "inclusions into U": listed(in_u),
           "features": [[ti, ci, tj, cj, f] for (ti, ci, tj, cj), f in feats.items()],
           "declared": declared,
           "lemmekey's best candidates": hybrid,
           "seconds": times, "counts": {**db.counts, "tests run for K": counts_k["tests run"],
                                        "tests pruned for K": counts_k["tests pruned by range"],
                                        "tests run for the hybrid rules alone": hybrid_tests}}
    db.close()
    return out


def run(out: Path, sources: list, only: list) -> None:
    members = bird_members()
    tmp = out / "tmp"
    for source in sources:
        schemas = [s for s in pickle.load(open(ROOT / "round-1v/sets" / f"{source}.pkl", "rb"))["schemas"] if s["fk"]]
        cands = candidates(source)
        sized = []
        for s in schemas:
            db = s["id"].split("/", 1)[1]
            if only and db not in only:
                continue
            size = members[db][2] if source == "bird" else locate(source, db, members, tmp)[2]
            sized.append((size, db, s))
        for size, db, s in sorted(sized, key=lambda x: (x[0], x[1])):
            dest = out / "profile" / source / f"{db}.json"
            if dest.exists():
                continue
            path, extracted, _ = locate(source, db, members, tmp)
            started = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
            t0 = time.perf_counter()
            try:
                res = profile(path, s, cands.get(s["id"], []))
            finally:
                if extracted:
                    path.unlink(missing_ok=True)
            res["started"], res["extracted for the run"] = started, extracted
            dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_text(json.dumps(res, default=lambda o: o.decode("utf-8", "replace") if isinstance(o, bytes) else str(o)) + "\n",
                            encoding="utf-8")
            print(f"{started} {s['id']:44} {size / 1e6:9.1f} MB {res['rows']:11d} rows "
                  f"{len(res['inclusions into K']):5d} K {len(res['inclusions into U']):6d} U "
                  f"{time.perf_counter() - t0:8.2f} s  " + " ".join(f"{v:.2f}" for v in res["seconds"].values()), flush=True)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    assert sys.argv[1] == "profile"
    srcs = [sys.argv[3]] if len(sys.argv) > 3 else ["spider", "bird"]
    run(Path(sys.argv[2]), srcs, sys.argv[4:])
