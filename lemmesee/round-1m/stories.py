"""Round 1m's bench: application databases that live, made from SchemaPile
schemas no teacher of the ranker saw, each with stories planted in its last
weeks and decoys beside them, so that a five is counted by what it finds
rather than by a judge's taste.

    python -I stories.py pick <schemapile-perm.json.gz> <round 1c teacher dir> <out> [<n>]
    python -I stories.py prompts <out> <schemas an agent>
    python -I stories.py check <out>
    python -I stories.py build <out> <key>...

pick writes <out>/<key>/schema.json (round 1c's snapshot of the schema) and
text.txt; prompts writes <out>/prompts/agent-<k>.txt, each asking an agent to
write <out>/<key>/spec.json for its schemas: what each table is, how its
columns are distributed, the stories and the decoys. check reads every spec
against its schema. build makes PostgreSQL's story_<key> from a spec, ninety
days ending on 29 October 2026, seeded, loaded and analysed, with round 1's
read-only role; round 1's see.sh then reads it like any fixture.
"""

from __future__ import annotations

import csv
import gzip
import hashlib
import io
import json
import math
import random
import re
import subprocess
import sys
from datetime import datetime, timedelta
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT / "round-1c"))
import teacher  # noqa: E402

SEED = 20261030
DAYS = 90
END = datetime(2026, 10, 29, 23, 59, 59)
START = END - timedelta(days=DAYS) + timedelta(seconds=1)
CONTAINER = "lemmesee-round-1-postgres-1"
MONEY = re.compile(r"(price|amount|total|cost|fee|salary|balance|revenue|payment|budget|wage|charge|fare|spend)", re.I)
STATUS = re.compile(r"(status|state|stage|result|outcome|resolution)", re.I)


def pick(source: Path, taught: Path, out: Path, n: int = 12) -> None:
    """Schemas that look like applications, as round 1c's teachers' did, with
    money, a status and two times to tell a story with, from repositories
    none of round 1c's 480 came from."""
    seen = {json.loads(line)["repo"] for line in (taught / "schemas.jsonl").open(encoding="utf-8")}
    corpus = json.load(gzip.open(source, "rt", encoding="utf-8"))
    ids = sorted(corpus)
    random.Random(SEED).shuffle(ids)
    chosen, signatures = [], set()
    for sid in ids:
        s = corpus[sid]
        if not teacher.looks_like_an_application(s) or not 5 <= len(s["TABLES"]) <= 14:
            continue
        repo = teacher.repository(s["INFO"].get("URL") or sid)
        sig = hashlib.sha256(json.dumps(sorted(t.lower() for t in s["TABLES"])).encode()).hexdigest()[:16]
        if repo in seen or sig in signatures:
            continue
        cols = [(c, v["TYPE"].lower()) for t in s["TABLES"].values() for c, v in t["COLUMNS"].items()]
        times = sum(1 for c, ty in cols if ty in teacher.TIME_TYPES)
        if times < 2 or not any(MONEY.search(c) for c, _ in cols) or not any(STATUS.search(c) for c, _ in cols):
            continue
        seen.add(repo)
        signatures.add(sig)
        chosen.append((sid, repo, s))
        if len(chosen) == n:
            break
    for k, (sid, repo, s) in enumerate(chosen):
        key = f"m{k:02d}"
        snap = teacher.snapshot(sid, s)
        snap["key"], snap["repo"] = key, repo
        (out / key).mkdir(parents=True, exist_ok=True)
        (out / key / "schema.json").write_text(json.dumps(snap), encoding="utf-8")
        (out / key / "text.txt").write_text(teacher.text(snap), encoding="utf-8")
        print(key, repo, len(snap["tables"]), "tables")


PROMPT = """You are designing test data for a dashboard generator. Do not run any command and do not read any file other than this one; when done, write each schema's answer with the Write tool, once, to the path given beside it.

For each schema below, imagine the application it belongs to running for the last 90 days, day 0 to day 89, day 89 being 29 October 2026. Describe its data so that a program can make it, and plant in its last weeks what the person running that application would need to see on the screen they open every week: two or three stories. Beside them, plant decoys a careless dashboard would show.

Write one JSON object a schema, exactly in this form:
{"about": "one sentence: what the application is and who uses it",
 "tables": {
   "<table>": {
     "role": "entity" (things that exist: users, products) | "event" (things that happen: orders, payments, logins) | "lookup" (a short fixed list) | "technical" (bookkeeping of the software: migrations, sessions, logs, settings) | "dead" (filled once by an import and never again),
     "rows": an integer from 1 to 20000 — how many rows exist on day 89,
     "time": "<column>" or null — the column that says when the row was made or happened,
     "growth": a number from -0.02 to 0.03 — how much the daily number of new rows grows each day,
     "follows": null or {"table": "<another table>", "per": how many rows of this table each row of that one makes on average, "minutes": the average delay} — for a table whose rows each come from one of another's, as a payment comes from an order,
     "columns": {"<column>": one of
        {"values": {"<value>": <weight>, ...}} — a category, a status, a type, a flag ("true"/"false"),
        {"money": [<median>, <spread, a factor such as 3>]},
        {"number": [<lowest>, <highest>]},
        {"after": "<a time column of this table>", "hours": [<least>, <most>]},
        {"empty": <the share of rows left null, 0 to 1>}}
   }, ...},
 "stories": [
   {"id": "<short name>", "says": "what happened, in a sentence", "table": "<table>", "from_day": an integer from 60 to 84,
    "change": one of
       {"column": "<column>", "values": {"<value>": <weight>, ...}} — the column's values are drawn so from that day,
       {"column": "<a money column>", "times": <a factor>, "where": null or {"<column>": "<value>"}} — the money is that many times what it was, for the rows where the column holds the value,
       {"rate": <a factor>, "where": null or {"<column>": "<value>"}} — that many times as many rows a day, of those holding the value,
    "seen_in": [{"table": "<table>", "column": "<column>"}, ...] — the table and column a chart must read to show it: the status or category it is broken down by, the money it sums, or the time it is drawn over}],
 "decoys": [{"id": "<short name>", "says": "why it is not worth a place", "tables": ["<table>", ...], "kind": "dead" | "bookkeeping" | "mirror"}]}

Rules:
- Use only the schema's own tables and columns, spelled as given. Describe in "columns" only the columns whose values matter: statuses, categories, kinds, money, quantities, times after another time. Keys, ids and names are made by the program; leave them out.
- Make the stories ones that matter to the business and that a chart can show: failures or cancellations rising, a channel or a kind of customer taking over, money growing or falling for one kind, a sudden rise in the rate of something. Not a change in an id.
- A decoy is "dead" (a table imported once, its time all on one day; give that table role "dead"), "bookkeeping" (a technical table) or "mirror" (two tables counting the same activity, one following the other: give the second "follows" the first). Plant one or two, with tables that are not in any story's seen_in.
- Event tables have most rows; lookups have few. Totals across tables under 60000.
"""


def prompts(out: Path, per: int) -> None:
    keys = sorted(p.parent.name for p in out.glob("m*/schema.json"))
    (out / "prompts").mkdir(exist_ok=True)
    for k in range(0, len(keys), per):
        body = [PROMPT]
        for key in keys[k:k + per]:
            path = (out / key / "spec.json").resolve()
            body.append(f"\n=== schema {key}: write its answer to {path} ===\n" + (out / key / "text.txt").read_text(encoding="utf-8"))
        (out / "prompts" / f"agent-{k // per}.txt").write_text("\n".join(body), encoding="utf-8")
    print(f"{(len(keys) + per - 1) // per} prompts")


# --- reading a spec against its schema -------------------------------------------------------

def tables_of(snap: dict) -> dict:
    return {t["name"]: t for t in snap["tables"]}


def check_one(snap: dict, spec: dict) -> list[str]:
    tabs = tables_of(snap)
    cols = {t: {c["name"] for c in tb["columns"]} for t, tb in tabs.items()}
    bad = []
    for t, d in spec.get("tables", {}).items():
        if t not in tabs:
            bad.append(f"no table {t}")
            continue
        if d.get("time") and d["time"] not in cols[t]:
            bad.append(f"no time column {t}.{d['time']}")
        f = d.get("follows")
        if f and f.get("table") not in tabs:
            bad.append(f"{t} follows no table {f.get('table')}")
        for c, how in (d.get("columns") or {}).items():
            if c not in cols[t]:
                bad.append(f"no column {t}.{c}")
            elif "after" in how and how["after"] not in cols[t]:
                bad.append(f"{t}.{c} after no column {how['after']}")
    for s in spec.get("stories", []):
        if s.get("table") not in tabs:
            bad.append(f"story {s.get('id')}: no table {s.get('table')}")
            continue
        ch = s.get("change") or {}
        if "column" in ch and ch["column"] not in cols[s["table"]]:
            bad.append(f"story {s.get('id')}: no column {s['table']}.{ch['column']}")
        for w in (ch.get("where") or {}):
            if w not in cols[s["table"]]:
                bad.append(f"story {s.get('id')}: no column {s['table']}.{w}")
        for e in s.get("seen_in", []):
            if e.get("table") not in tabs or e.get("column") not in cols.get(e.get("table"), ()):
                bad.append(f"story {s.get('id')}: seen in no column {e}")
    for d in spec.get("decoys", []):
        for t in d.get("tables", []):
            if t not in tabs:
                bad.append(f"decoy {d.get('id')}: no table {t}")
    return bad


def check(out: Path) -> None:
    for p in sorted(out.glob("m*/schema.json")):
        spec_p = p.with_name("spec.json")
        if not spec_p.exists():
            print(p.parent.name, "no spec")
            continue
        try:
            spec = json.loads(spec_p.read_text(encoding="utf-8"))
        except json.JSONDecodeError as e:
            print(p.parent.name, "unreadable:", e)
            continue
        bad = check_one(json.loads(p.read_text(encoding="utf-8")), spec)
        print(p.parent.name, f"{len(spec.get('stories', []))} stories, {len(spec.get('decoys', []))} decoys;",
              "ok" if not bad else "; ".join(bad))


# --- making the data ------------------------------------------------------------------------

def pg_type(ty: str) -> str:
    ty = ty.lower()
    if re.match(r"(tiny|small|medium)?int|integer|serial|unsigned(tiny|small|medium)?int", ty):
        return "integer"
    if re.match(r"(big|unsignedbig)(int|serial)", ty):
        return "bigint"
    if re.match(r"(decimal|numeric|money|smallmoney|float|double|real|number)", ty):
        return "numeric"
    if re.match(r"(bool|boolean|bit)$", ty):
        return "boolean"
    if ty == "date":
        return "date"
    if ty in teacher.TIME_TYPES:
        return "timestamp"
    return "text"


def ddl(snap: dict) -> list[str]:
    out = []
    for t in snap["tables"]:
        cols = [f'"{c["name"]}" {pg_type(c["type_name"])}' for c in t["columns"]]
        pk = next((k["columns"] for k in t["constraints"] if k["kind"] == "p"), None)
        if pk:
            cols.append("PRIMARY KEY (" + ", ".join(f'"{c}"' for c in pk) + ")")
        out.append(f'CREATE TABLE "{t["name"]}" (' + ", ".join(cols) + ");")
    return out


def foreign_keys(snap: dict) -> list[str]:
    out = []
    for t in snap["tables"]:
        for k in t["constraints"]:
            if k["kind"] == "f" and k["ref_columns"]:
                ref = k["ref_table"].split(".")[-1]
                out.append(f'ALTER TABLE "{t["name"]}" ADD CONSTRAINT "{k["name"]}" FOREIGN KEY ('
                           + ", ".join(f'"{c}"' for c in k["columns"]) + f') REFERENCES "{ref}" ('
                           + ", ".join(f'"{c}"' for c in k["ref_columns"]) + ");")
    return out


def day_of(ts: datetime) -> int:
    return max(0, min(DAYS - 1, (ts - START).days))


class Maker:
    """One database's rows, made from its spec."""

    def __init__(self, snap: dict, spec: dict, seed: int):
        self.snap, self.spec, self.rng = snap, spec, random.Random(seed)
        self.tabs = tables_of(snap)
        self.d = {t: spec.get("tables", {}).get(t, {}) for t in self.tabs}
        self.n = {t: max(1, min(20000, int(self.d[t].get("rows") or self.default_rows(t)))) for t in self.tabs}
        self.rows: dict[str, list[dict]] = {}
        self.times: dict[str, list[datetime]] = {}

    def default_rows(self, t: str) -> int:
        return 40 if not any(k["kind"] == "f" for k in self.tabs[t]["constraints"]) else 400

    def pk(self, t: str) -> list[str]:
        return next((k["columns"] for k in self.tabs[t]["constraints"] if k["kind"] == "p"), [])

    def key_value(self, t: str, i: int) -> str:
        col = self.pk(t)[0] if self.pk(t) else None
        c = next((c for c in self.tabs[t]["columns"] if c["name"] == col), None)
        return str(i + 1) if c is None or pg_type(c["type_name"]) in ("integer", "bigint", "numeric") else f"{t}-{i + 1}"

    def weights(self, t: str) -> list[float]:
        g = float(self.d[t].get("growth") or 0.0)
        w = [(1 + g) ** day * (0.75 if (START + timedelta(days=day)).weekday() >= 5 else 1.0) for day in range(DAYS)]
        for s in self.spec.get("stories", []):
            ch = s.get("change") or {}
            if s.get("table") == t and "rate" in ch and not ch.get("where"):
                w = [x * (float(ch["rate"]) if day >= int(s["from_day"]) else 1.0) for day, x in enumerate(w)]
        return w

    def stamps(self, t: str, n: int) -> list[datetime]:
        role = self.d[t].get("role", "entity")
        rng = self.rng
        if role == "dead":
            day = rng.randrange(10, 50)
            return sorted(START + timedelta(days=day, seconds=rng.randrange(3600, 7200)) for _ in range(n))
        if role == "technical":
            days = sorted(rng.sample(range(DAYS), min(5, DAYS)))
            return sorted(START + timedelta(days=rng.choice(days), seconds=rng.randrange(86400)) for _ in range(n))
        if role == "lookup":
            return sorted(START - timedelta(days=rng.randrange(200, 400)) for _ in range(n))
        w = self.weights(t)
        days = rng.choices(range(DAYS), weights=w, k=n)
        return sorted(START + timedelta(days=day, seconds=rng.randrange(86400)) for day in days)

    def order(self) -> list[str]:
        """Tables a table follows first."""
        done, out = set(), []

        def visit(t, stack=()):
            if t in done or t in stack:
                return
            f = self.d[t].get("follows")
            if f and f.get("table") in self.tabs:
                visit(f["table"], stack + (t,))
            done.add(t)
            out.append(t)

        for t in self.tabs:
            visit(t)
        return out

    def make(self) -> None:
        for t in self.order():
            f = self.d[t].get("follows")
            if f and f.get("table") in self.times and f["table"] != t:
                parent_times = self.times[f["table"]]
                per, minutes = float(f.get("per") or 1.0), float(f.get("minutes") or 30.0)
                made = []
                for pi, pt in enumerate(parent_times):
                    k = self.poisson(per)
                    for _ in range(k):
                        made.append((pt + timedelta(minutes=self.rng.expovariate(1 / max(minutes, 1.0))), pi))
                made = [m for m in made if m[0] <= END][:20000] or [(parent_times[0], 0)]
                made.sort()
                self.n[t] = len(made)
                self.times[t] = [m[0] for m in made]
                self.parent_of = getattr(self, "parent_of", {})
                self.parent_of[t] = (f["table"], [m[1] for m in made])
            else:
                self.times[t] = self.stamps(t, self.n[t])
        for t in self.tabs:
            self.rows[t] = [self.row(t, i) for i in range(self.n[t])]
        self.stories()

    def poisson(self, lam: float) -> int:
        if lam <= 0:
            return 0
        x, p, s = 0, math.exp(-lam), math.exp(-lam)
        u = self.rng.random()
        while u > s and x < 50:
            x += 1
            p *= lam / x
            s += p
        return x

    def parent_index(self, ref: str) -> int:
        """A parent's row, many children to a few parents, as in life."""
        n = self.n[ref]
        return min(n - 1, int(n * self.rng.random() ** 2.2))

    def row(self, t: str, i: int) -> dict:
        tb, spec_cols, rng = self.tabs[t], self.d[t].get("columns") or {}, self.rng
        when = self.times[t][i]
        r = {}
        pk = self.pk(t)
        fks = {c: k for k in tb["constraints"] if k["kind"] == "f" for c in k["columns"]}
        uniq = {c for k in tb["constraints"] if k["kind"] == "u" for c in k["columns"]}
        own_time = self.d[t].get("time")
        follows = getattr(self, "parent_of", {}).get(t)
        for c in tb["columns"]:
            name, ty = c["name"], pg_type(c["type_name"])
            how = spec_cols.get(name)
            if name in fks:
                k = fks[name]
                ref = k["ref_table"].split(".")[-1]
                if ref not in self.n:
                    r[name] = None
                    continue
                if follows and follows[0] == ref:
                    pi = follows[1][i]
                elif ref == t:
                    pi = rng.randrange(i) if i else None
                    if pi is None:
                        r[name] = None
                        continue
                else:
                    pi = self.parent_index(ref)
                r[name] = self.key_value(ref, pi)
                continue
            if name in pk:
                r[name] = self.key_value(t, i) if len(pk) == 1 else f"{i + 1}"
                continue
            if how and "empty" in how and rng.random() < float(how["empty"]):
                r[name] = None
                continue
            if how and "values" in how:
                r[name] = self.typed(ty, self.vocab(t, name), self.draw(how["values"]))
            elif how and "money" in how:
                med, spread = (list(how["money"]) + [3])[:2]
                v = float(med) * math.exp(rng.gauss(0, math.log(max(float(spread), 1.01)) / 2))
                r[name] = round(v) if ty in ("integer", "bigint") else round(v, 2)
            elif how and "number" in how:
                lo, hi = how["number"][:2]
                r[name] = rng.randint(int(lo), int(hi)) if ty in ("integer", "bigint") else round(rng.uniform(float(lo), float(hi)), 2)
            elif how and "after" in how:
                r[name] = ("after", how["after"], how.get("hours") or [1, 48])
            elif ty in ("timestamp", "date"):
                low = name.lower()
                if own_time == name or own_time is None and re.search(r"creat|insert|added|registered|made|date$|time$|_at$|at$", low):
                    r[name] = when
                elif re.search(r"delet|remov|archiv", low):
                    r[name] = when + timedelta(days=rng.randrange(1, 30)) if rng.random() < 0.04 else None
                elif re.search(r"updat|modif|chang", low):
                    r[name] = min(END, when + timedelta(hours=rng.randrange(1, 240)))
                else:
                    r[name] = min(END, when + timedelta(hours=rng.randrange(0, 72))) if rng.random() < 0.8 or not c["nullable"] else None
            elif ty == "boolean":
                r[name] = rng.random() < 0.5
            elif ty in ("integer", "bigint"):
                r[name] = rng.randint(1, 5) if re.search(r"(qty|quantity|count|number|amount)", name, re.I) else rng.randint(0, 100)
            elif ty == "numeric":
                r[name] = round(rng.uniform(1, 200), 2)
            else:
                r[name] = self.text(t, name, i, name in uniq)
        for name, v in list(r.items()):
            if isinstance(v, tuple) and v[0] == "after":
                base = r.get(v[1]) if isinstance(r.get(v[1]), datetime) else when
                lo, hi = (list(v[2]) + [48])[:2]
                r[name] = base + timedelta(hours=rng.uniform(float(lo), float(hi)))
                if r[name] > END:
                    r[name] = None
        r["_day"] = day_of(when)
        return r

    def draw(self, values: dict):
        items = [(str(k), float(w)) for k, w in values.items() if float(w) > 0] or [("unknown", 1.0)]
        return self.rng.choices([k for k, _ in items], weights=[w for _, w in items])[0]

    def vocab(self, t: str, c: str) -> list[str]:
        """Every word a column is given, by the spec or by a story, in the
        order first given: one code a word, whoever draws it."""
        words = [str(k) for k in ((self.d[t].get("columns") or {}).get(c) or {}).get("values", {})]
        for s in self.spec.get("stories", []):
            ch = s.get("change") or {}
            if s.get("table") == t and ch.get("column") == c:
                words += [str(k) for k in ch.get("values", {}) if str(k) not in words]
        return words

    def col_type(self, t: str, c: str) -> str:
        return next((pg_type(x["type_name"]) for x in self.tabs[t]["columns"] if x["name"] == c), "text")

    @staticmethod
    def typed(ty: str, words: list[str], v: str):
        """A category as its column can hold it: a word in a column of
        numbers becomes its place in the column's words, a word in a boolean
        whether it is the first."""
        if ty in ("integer", "bigint", "numeric") and not re.fullmatch(r"-?\d+(\.\d+)?", v):
            return words.index(v) + 1 if v in words else 0
        if ty == "boolean" and v.lower() not in ("true", "false", "1", "0", "t", "f", "yes", "no", "y", "n"):
            return words.index(v) == 0 if v in words else False
        return v

    def text(self, t: str, name: str, i: int, unique: bool) -> str:
        low = name.lower()
        if "mail" in low:
            return f"{re.sub('[^a-z]', '', t.lower())[:8] or 'user'}{i + 1}@example.com"
        if re.search(r"name|title|label", low):
            return f"{t.rstrip('s').replace('_', ' ')} {i + 1}"
        if unique:
            return f"{name}-{i + 1}"
        return f"{name} {self.rng.randint(1, 30)}"

    def stories(self) -> None:
        for s in self.spec.get("stories", []):
            t, ch, d0 = s.get("table"), s.get("change") or {}, int(s.get("from_day") or 75)
            if t not in self.rows:
                continue
            rows = self.rows[t]
            where = ch.get("where") or {}
            hit = lambda r: r["_day"] >= d0 and all(str(r.get(k)) == str(v) for k, v in where.items())
            if "values" in ch and ch.get("column"):
                ty, words = self.col_type(t, ch["column"]), self.vocab(t, ch["column"])
                for r in rows:
                    if r["_day"] >= d0:
                        r[ch["column"]] = self.typed(ty, words, self.draw(ch["values"]))
            elif "times" in ch and ch.get("column"):
                for r in rows:
                    if hit(r) and isinstance(r.get(ch["column"]), (int, float)):
                        r[ch["column"]] = round(r[ch["column"]] * float(ch["times"]), 2)
            elif "rate" in ch and where:
                # more rows a day of those holding the value: copies of them, new keys
                late = [r for r in rows if hit(r)]
                extra = int(len(late) * (float(ch["rate"]) - 1))
                if extra > 0 and late:
                    pk = self.pk(t)
                    for _ in range(min(extra, 20000 - len(rows))):
                        r = dict(self.rng.choice(late))
                        i = len(rows)
                        for c in pk:
                            r[c] = self.key_value(t, i) if len(pk) == 1 else f"{i + 1}"
                        rows.append(r)
                elif extra < 0:
                    drop = set(self.rng.sample(range(len(late)), min(len(late), -extra)))
                    ids = {id(r) for k, r in enumerate(late) if k in drop}
                    self.rows[t] = [r for r in rows if id(r) not in ids]
                self.n[t] = len(self.rows[t])

    def csv(self, t: str) -> str:
        cols = [c["name"] for c in self.tabs[t]["columns"]]
        buf = io.StringIO()
        w = csv.writer(buf, lineterminator="\n")
        pk = self.pk(t)
        seen = set()
        for r in self.rows[t]:
            if pk:
                k = tuple(r.get(c) for c in pk)
                if k in seen or any(v is None for v in k):
                    continue
                seen.add(k)
            w.writerow(["" if r.get(c) is None else (r[c].strftime("%Y-%m-%d %H:%M:%S") if isinstance(r[c], datetime)
                                                    else ("true" if r[c] is True else "false" if r[c] is False else r[c]))
                        for c in cols])
        return buf.getvalue()


def psql(db: str, sql: str, stop: bool = True) -> str:
    cmd = ["docker", "exec", "-i", CONTAINER, "psql", "-q", "-U", "postgres", "-d", db]
    if stop:
        cmd += ["-v", "ON_ERROR_STOP=1"]
    p = subprocess.run(cmd, input=sql, capture_output=True, text=True, encoding="utf-8")
    if stop and p.returncode != 0:
        raise SystemExit(f"{db}: {p.stderr.strip()[-400:]}")
    return p.stderr


def build(out: Path, keys: list[str]) -> None:
    for key in keys:
        snap = json.loads((out / key / "schema.json").read_text(encoding="utf-8"))
        spec = json.loads((out / key / "spec.json").read_text(encoding="utf-8"))
        bad = check_one(snap, spec)
        if bad:
            print(key, "spec has problems, left as they are:", "; ".join(bad))
        m = Maker(snap, spec, SEED + int(key[1:]))
        m.make()
        db = f"story_{key}"
        psql("postgres", f"DROP DATABASE IF EXISTS {db};\nCREATE DATABASE {db};\n")
        psql(db, "\n".join(ddl(snap)) + "\n")
        for t in m.tabs:
            data = m.csv(t)
            cols = ", ".join(f'"{c["name"]}"' for c in m.tabs[t]["columns"])
            p = subprocess.run(["docker", "exec", "-i", CONTAINER, "psql", "-q", "-U", "postgres", "-d", db, "-v", "ON_ERROR_STOP=1",
                                "-c", f'\\copy "{t}" ({cols}) FROM STDIN WITH (FORMAT csv)'],
                               input=data, capture_output=True, text=True, encoding="utf-8")
            if p.returncode != 0:
                raise SystemExit(f"{key}.{t}: {p.stderr.strip()[-400:]}")
        errs = psql(db, "\n".join(foreign_keys(snap)) + "\n", stop=False)
        psql(db, f"""CREATE EXTENSION IF NOT EXISTS pg_stat_statements;
DO $$ BEGIN IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'lemmesee') THEN CREATE ROLE lemmesee LOGIN PASSWORD 'lemmesee'; END IF; END $$;
GRANT CONNECT ON DATABASE {db} TO lemmesee;
GRANT USAGE ON SCHEMA public TO lemmesee;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO lemmesee;
ANALYZE;
""")
        fk_lost = errs.count("ERROR")
        print(key, sum(len(m.rows[t]) for t in m.tabs), "rows in", len(m.tabs), "tables;",
              f"{fk_lost} foreign keys not declared" if fk_lost else "every foreign key declared", flush=True)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, *a = sys.argv[1:]
    if cmd == "pick":
        pick(Path(a[0]), Path(a[1]), Path(a[2]), int(a[3]) if len(a) > 3 else 12)
    elif cmd == "prompts":
        prompts(Path(a[0]), int(a[1]))
    elif cmd == "check":
        check(Path(a[0]))
    elif cmd == "build":
        build(Path(a[0]), a[1:])
