"""Round 1c's teacher data: SchemaPile-Perm schemas chosen to look like
applications, as round 1's snapshots (tier S only, declared keys kept) and as
the plain text a teacher reads, and the teacher's answers gathered back.

    python -I teacher.py select <schemapile-perm.json.gz> <out> [n]
    python -I teacher.py batches <out> <per batch>
    python -I teacher.py gather <out>

select writes <out>/schemas.jsonl, one snapshot a line, and <out>/text/<id>.txt;
batches writes <out>/batches/<k>.txt, the prompt for one teacher agent;
gather reads <out>/answers/*.json and writes <out>/teacher.jsonl.
"""

import gzip
import hashlib
import json
import random
import re
import sys
from pathlib import Path

SEED = 20261028

# Tables a framework makes for itself: not what an application is about
BOILERPLATE = re.compile(
    r"^(django_|auth_|oauth|migrations?$|schema_migrations|ar_internal|failed_jobs|password_resets?|"
    r"personal_access_tokens|sessions?$|cache|jobs$|job_batches|flyway|databasechangelog|sqlite_|"
    r"qrtz_|act_|wp_options|wp_usermeta|permissions?$|role_has|model_has|users_roles|spring_session)",
    re.I,
)
TIME_TYPES = {"timestamp", "datetime", "date", "datetime2", "smalldatetime", "timestamptz", "timestampwithtimezone"}
NUMERIC = {"int", "integer", "bigint", "smallint", "tinyint", "decimal", "numeric", "float", "double", "real",
           "money", "unsignedint", "unsignedbigint", "unsignedsmallint", "unsignedtinyint", "doubleprecision"}


def repository(url: str) -> str:
    m = re.match(r"https://github\.com/([^/]+)/([^/]+)/", url)
    return f"{m.group(1)}/{m.group(2)}".lower() if m else url


def ascii_name(s: str) -> bool:
    return bool(re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", s))


def looks_like_an_application(schema: dict) -> bool:
    tables = schema["TABLES"]
    names = list(tables)
    if not 4 <= len(names) <= 25 or not all(ascii_name(n) for n in names):
        return False
    own = [n for n in names if not BOILERPLATE.match(n)]
    if len(own) < 4 or len(own) < 0.6 * len(names):
        return False
    columns = [(n, c, v) for n in own for c, v in tables[n]["COLUMNS"].items()]
    if len(columns) > 160 or not all(ascii_name(c) for _, c, _ in columns):
        return False
    fks = sum(len(tables[n]["FOREIGN_KEYS"] or []) for n in own)
    times = sum(1 for _, c, v in columns if v["TYPE"].lower() in TIME_TYPES or re.search(r"year", c, re.I))
    numbers = sum(
        1 for _, c, v in columns
        if v["TYPE"].lower() in NUMERIC and not v["IS_PRIMARY"] and not re.search(r"(^|_)id$|Id$|ID$", c)
    )
    return fks >= 2 and times >= 1 and numbers >= 1


def snapshot(sid: str, schema: dict) -> dict:
    """Round 1's snapshot of a SchemaPile schema: tier S, keys declared."""
    tables = []
    names = {n.lower(): n for n in schema["TABLES"]}
    for name, t in schema["TABLES"].items():
        cols = t["COLUMNS"]
        pk = [c for c, v in cols.items() if v["IS_PRIMARY"]] or list(t.get("PRIMARY_KEYS") or [])
        constraints = []
        if pk:
            constraints.append({"name": f"{name}_pkey", "kind": "p", "columns": pk})
        for c, v in cols.items():
            if v["UNIQUE"] and c not in pk:
                constraints.append({"name": f"{name}_{c}_key", "kind": "u", "columns": [c]})
        for i, f in enumerate(t["FOREIGN_KEYS"] or []):
            target = names.get((f["FOREIGN_TABLE"] or "").lower())
            if target is None or not f["COLUMNS"]:
                continue
            ref = f.get("REFERRED_COLUMNS") or []
            constraints.append({
                "name": f"{name}_fk{i}", "kind": "f", "columns": list(f["COLUMNS"]),
                "ref_table": f"public.{target}", "ref_columns": list(ref),
            })
        tables.append({
            "schema": "public", "name": name, "kind": "table", "can_select": True, "features": {},
            "columns": [
                {"name": c, "position": i + 1, "type_name": v["TYPE"].lower(), "base_type": v["TYPE"].lower(),
                 "nullable": bool(v["NULLABLE"]) if v["NULLABLE"] is not None else True, "can_select": True,
                 "features": {}}
                for i, (c, v) in enumerate(cols.items())
            ],
            "constraints": constraints,
            "indexes": [],
        })
    return {"version": 1, "engine": "schemapile", "server_version": "", "database": sid, "role": "", "taken_at": "",
            "settings": {}, "workload": {}, "tables": tables, "queries": []}


def text(snap: dict) -> str:
    """The schema as a teacher reads it: tables, columns, types, keys."""
    out = []
    for t in snap["tables"]:
        pk = next((k["columns"] for k in t["constraints"] if k["kind"] == "p"), [])
        refs = {}
        for k in t["constraints"]:
            if k["kind"] == "f":
                target = k["ref_table"].split(".")[-1]
                for i, c in enumerate(k["columns"]):
                    r = k["ref_columns"][i] if i < len(k["ref_columns"]) else "?"
                    refs[c] = f"{target}.{r}"
        out.append(t["name"])
        for c in t["columns"]:
            bits = [c["type_name"]]
            if c["name"] in pk:
                bits.append("primary key" if len(pk) == 1 else "part of the primary key")
            if c["name"] in refs:
                bits.append(f"references {refs[c['name']]}")
            if c["nullable"]:
                bits.append("nullable")
            out.append(f"  {c['name']}: {', '.join(bits)}")
        out.append("")
    return "\n".join(out)


def select(source: Path, out: Path, n: int) -> None:
    corpus = json.load(gzip.open(source, "rt", encoding="utf-8"))
    seen_repos, seen_signatures, chosen = set(), set(), []
    ids = sorted(corpus)
    random.Random(SEED).shuffle(ids)
    for sid in ids:
        s = corpus[sid]
        if not looks_like_an_application(s):
            continue
        repo = repository(s["INFO"].get("URL") or sid)
        signature = hashlib.sha256(json.dumps(sorted(n.lower() for n in s["TABLES"])).encode()).hexdigest()[:16]
        if repo in seen_repos or signature in seen_signatures:
            continue
        seen_repos.add(repo)
        seen_signatures.add(signature)
        chosen.append((sid, repo, s))
        if len(chosen) == n:
            break
    (out / "text").mkdir(parents=True, exist_ok=True)
    with (out / "schemas.jsonl").open("w", encoding="utf-8") as f:
        for k, (sid, repo, s) in enumerate(chosen):
            key = f"s{k:03d}"
            snap = snapshot(sid, s)
            snap["key"] = key
            snap["repo"] = repo
            f.write(json.dumps(snap) + "\n")
            (out / "text" / f"{key}.txt").write_text(text(snap), encoding="utf-8")
    print(f"{len(chosen)} schemas")


PROMPT = """You are writing lists of questions from database schemas alone. Do not read, search or open any files and do not run any commands or tools: answer from the text below only.

For each schema below, someone has just been given access to that database and wants a first dashboard that tells them what is going on in it. Write the 10 questions this person would most want answered first, the most important first: your first five are what you would put on the first screen. Each question must be answerable by one SQL query over that schema's tables as they are — a count, sum or average over rows, grouped by a time period and/or one breakdown, or a top 12 — and drawn as one chart. If a schema holds nothing worth a dashboard, still give its best ten.

Rules:
- Use only the tables and columns listed for that schema. Do not invent any.
- It is fine for a question to need joins, an expression (such as price × quantity) or a filter: say so in the fields.
- A time period may be a year or a month held as an integer column: give it as the column with bucket "year" or "month".

Reply with only one JSON object and no other text: {"<schema key>": [10 question objects], ...} for every schema key below. Each question object:
{"question": "a short English question",
 "why": "why it matters, in a few words",
 "base": "the table whose rows are counted or aggregated",
 "measure": "count(*)" or "sum(table.column)" or "avg(table.column)" or "count(distinct table.column)" or "sum(an expression over columns)",
 "time": null or {"column": "table.column", "bucket": "day" | "week" | "month" | "quarter" | "year"},
 "breakdown": null or "table.column" — what the result is grouped by besides time,
 "top": null or 12 — if it lists the top items,
 "filter": null or a SQL condition,
 "joins": ["child_table.column -> parent_table.column", ...] or []}
"""


def batches(out: Path, per: int) -> None:
    keys = [json.loads(line)["key"] for line in (out / "schemas.jsonl").open(encoding="utf-8")]
    (out / "batches").mkdir(exist_ok=True)
    for b in range(0, len(keys), per):
        part = keys[b:b + per]
        body = [PROMPT]
        for key in part:
            body.append(f"=== schema {key} ===\n" + (out / "text" / f"{key}.txt").read_text(encoding="utf-8"))
        (out / "batches" / f"b{b // per:02d}.txt").write_text("\n".join(body), encoding="utf-8")
    print(f"{(len(keys) + per - 1) // per} batches")


def gather(out: Path) -> None:
    rows, bad = [], []
    for p in sorted((out / "answers").glob("*.json")):
        try:
            answer = json.loads(p.read_text(encoding="utf-8"))
        except json.JSONDecodeError as e:
            bad.append(f"{p.name}: {e}")
            continue
        for key, questions in answer.items():
            rows.append({"key": key, "questions": questions})
    with (out / "teacher.jsonl").open("w", encoding="utf-8") as f:
        for r in sorted(rows, key=lambda r: r["key"]):
            f.write(json.dumps(r) + "\n")
    print(f"{len(rows)} schemas answered; {len(bad)} answers unreadable", *bad, sep="\n")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "select":
        select(Path(sys.argv[2]), Path(sys.argv[3]), int(sys.argv[4]) if len(sys.argv) > 4 else 240)
    elif cmd == "batches":
        batches(Path(sys.argv[2]), int(sys.argv[3]))
    elif cmd == "gather":
        gather(Path(sys.argv[2]))
