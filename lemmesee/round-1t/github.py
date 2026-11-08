#!/usr/bin/env python3
"""Collect a held-out test corpus of real relational schemas from GitHub.

Purpose: a test set for a foreign-key prediction model. The selection rule is
fixed before anyone looks at the data and is applied mechanically; this module
prints counts only and never schema content.

The rule
  1. Candidates: GitHub code search for SQL files declaring foreign keys
     (queries QUERIES below, with the language/extension variants, each sliced
     by file size so that more than the API's 1000 results per query can be
     reached; every slice is paged up to the API limit). Collect the repos.
  2. Keep a repo only if its `created_at` is on or after 2025-07-01, it is not
     a fork, not archived, and its table-name set does not overlap >= 50% with
     a well-known sample database (see SAMPLES; names only; overlap is
     |A & B| / |A| >= .5 or |A & B| / |B| >= .5 with |A & B| >= 3).
  3. Per repo choose ONE .sql file at the default branch's HEAD commit (the sha
     is pinned): the one declaring the most foreign keys (count of the
     REFERENCES keyword outside comments); ties go to the shorter path, then
     to the path in alphabetical order. At most MAX_SQL_FILES files per repo
     are read (first by path); blobs over MAX_BLOB bytes are skipped.
  4. Parse it with sqlglot, statement by statement (statements are split here,
     because one unsupported statement must not lose the file). Dialects are
     tried in the order postgres, mysql, sqlite, tsql: the first that parses
     every CREATE TABLE of the file is taken, and if none does, the first
     that yields any table. Handled: CREATE TABLE with inline REFERENCES,
     table-level FOREIGN KEY, inline or table-level PRIMARY KEY, UNIQUE,
     ALTER TABLE ... ADD [CONSTRAINT] FOREIGN KEY / PRIMARY KEY, CREATE INDEX.
     Names are compared case-insensitively, schema qualifiers and quotes are
     stripped, and written lower-cased.
  5. Keep the schema only if it has >= 5 tables and >= 3 foreign keys whose
     referencing column(s) and referenced table resolve (one-column and
     composite ones both count here). Drop exact duplicates (same sorted
     table-name set), keeping the earliest-created repository.
  6. From the kept ones take up to 80: sort by sha256("owner/name") and take
     the first 80, before anything else about them is read.
  7. Write manifest.json, raw/<owner>__<repo>.sql, hidden.jsonl, summary.json.
  8. Write sha256.txt (hidden.jsonl and every raw file).

Commands (run with the venv python that has sqlglot installed):
  python github.py find    # code search -> work/candidates.json
  python github.py fetch   # repo metadata, HEAD, chosen file -> work/repos.json
  python github.py parse   # steps 4-8 -> manifest/hidden/summary/sha256

Everything lives in lemmesee/corpus/round-1t/heldout-github/ (git-ignored).
Uses the authenticated `gh` CLI; no token is read, printed or written.
If connections reset, `gh` is retried through the local proxy
http://localhost:2080 (HTTPS_PROXY / HTTP_PROXY are set for the process).
"""
import hashlib
import json
import logging
import os
import re
import subprocess
import sys
import threading
import time
import urllib.parse
from concurrent.futures import ThreadPoolExecutor

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(
    HERE, "..", "corpus", "round-1t", "heldout-github"))
WORK = os.path.join(OUT, "work")

CREATED_FROM = "2025-07-01"
MAX_REPOS = 80
MAX_SQL_FILES = 300
MAX_BLOB = 3_000_000
MIN_TABLES = 5
MIN_FKS = 3
DIALECTS = ["postgres", "mysql", "sqlite", "tsql"]

QUERIES = [
    '"CREATE TABLE" "REFERENCES" language:SQL',
    '"FOREIGN KEY" "CREATE TABLE" language:SQL',
    '"REFERENCES" "PRIMARY KEY" extension:sql',
    '"CREATE TABLE" "REFERENCES" language:PLpgSQL',
    '"FOREIGN KEY" "CREATE TABLE" language:PLpgSQL',
    '"CREATE TABLE" "REFERENCES" language:TSQL',
    '"FOREIGN KEY" "CREATE TABLE" language:TSQL',
]
SIZE_SLICES = ["<1000", "1000..2000", "2000..3000", "3000..5000",
               "5000..8000", "8000..12000", "12000..20000",
               "20000..40000", "40000..100000", ">100000"]

# Table-name sets of well-known sample databases (compared by names only).
SAMPLES = {
    "sakila_pagila_dvdrental": "actor address category city country customer "
    "film film_actor film_category film_text inventory language payment "
    "rental staff store",
    "northwind": "categories customers employees employeeterritories "
    "orderdetails order_details orders products region shippers suppliers "
    "territories customerdemographics customercustomerdemo",
    "chinook": "album artist customer employee genre invoice invoiceline "
    "mediatype playlist playlisttrack track",
    "adventureworks": "address addresstype billofmaterials contacttype "
    "countryregion currency customer department document employee "
    "employeedepartmenthistory employeepayhistory illustration "
    "jobcandidate location product productcategory productcosthistory "
    "productdescription productdocument productinventory "
    "productlistpricehistory productmodel productphoto productreview "
    "productsubcategory salesorderdetail salesorderheader salesperson "
    "salesterritory shift shipmethod specialoffer stateprovince store "
    "transactionhistory unitmeasure vendor workorder",
    "classicmodels": "customers employees offices orderdetails orders "
    "payments productlines products",
    "employees_sample": "employees departments dept_emp dept_manager titles "
    "salaries",
    "world": "city country countrylanguage",
    "hr_oracle": "regions countries locations departments jobs employees "
    "job_history",
    "oe_oracle": "customers warehouses order_items orders inventories "
    "product_information product_descriptions promotions",
    "wideworldimporters": "application_people buyinggroups cities colors "
    "countries customers customercategories customertransactions "
    "deliverymethods invoicelines invoices orderlines orders packagetypes "
    "paymentmethods people purchaseorderlines purchaseorders "
    "stockgroups stockitems stockitemholdings stockitemstockgroups "
    "stateprovinces supplier suppliers suppliercategories "
    "suppliertransactions systemparameters transactiontypes",
    "pubs": "authors publishers titles titleauthor stores sales roysched "
    "discounts employee jobs pub_info",
    "university_textbook": "student course takes instructor department "
    "section classroom time_slot advisor teaches prereq",
    "school_sample": "students teachers classes courses enrollments grades "
    "subjects departments",
}
SAMPLES = {k: set(v.split()) for k, v in SAMPLES.items()}

logging.getLogger("sqlglot").setLevel(logging.CRITICAL)


# ---------------------------------------------------------------- gh helper

class GhError(Exception):
    def __init__(self, msg):
        super().__init__(msg)
        self.msg = msg


_proxy_on = False
_lock = threading.Lock()


def _enable_proxy():
    global _proxy_on
    with _lock:
        if not _proxy_on:
            os.environ["HTTPS_PROXY"] = "http://localhost:2080"
            os.environ["HTTP_PROXY"] = "http://localhost:2080"
            _proxy_on = True
            print("  [net] switched to the local proxy", flush=True)


def gh(args, raw=False, tries=8):
    """Run `gh api ...`; sleep on rate limits, retry on resets.

    Returns parsed JSON, or bytes when raw is true.
    """
    net_fail = 0
    for attempt in range(tries):
        p = subprocess.run(["gh", "api"] + args, capture_output=True)
        if p.returncode == 0:
            return p.stdout if raw else json.loads(p.stdout or b"null")
        err = p.stderr.decode("utf-8", "replace") + p.stdout.decode(
            "utf-8", "replace")[:500]
        low = err.lower()
        if "rate limit" in low or "secondary" in low or "abuse" in low or \
                "http 429" in low:
            print("  [rate] sleeping 65 s", flush=True)
            time.sleep(65)
            continue
        if any(s in low for s in ("404", "409", "422", "451", "410", "403")):
            raise GhError(re.sub(r"\s+", " ", err)[:120])
        net_fail += 1
        if net_fail >= 2:
            _enable_proxy()
        time.sleep(4 * net_fail)
    raise GhError("gave up: " + re.sub(r"\s+", " ", err)[:120])


def jload(name, default):
    p = os.path.join(WORK, name)
    if os.path.exists(p):
        with open(p, encoding="utf-8") as f:
            return json.load(f)
    return default


def jsave(name, obj):
    os.makedirs(WORK, exist_ok=True)
    p = os.path.join(WORK, name)
    with open(p + ".tmp", "w", encoding="utf-8", newline="\n") as f:
        json.dump(obj, f, ensure_ascii=False)
    os.replace(p + ".tmp", p)


# ---------------------------------------------------------------- find

def search_pages(key, cands, done, last):
    """Page one query up to the API limit (10 pages of 100); returns total."""
    total, seen = None, 0
    for page in range(1, 11):
        wait = 6.5 - (time.time() - last[0])
        if wait > 0:
            time.sleep(wait)
        last[0] = time.time()
        try:
            r = gh(["-X", "GET", "search/code", "-f", "q=" + key,
                    "-f", "per_page=100", "-f", "page=%d" % page])
        except GhError as e:
            print("  search stop: %s" % e.msg[:80], flush=True)
            break
        total = r.get("total_count", 0)
        items = r.get("items", [])
        for it in items:
            c = cands.setdefault(it["repository"]["full_name"], {"hits": 0})
            c["hits"] += 1
        seen += len(items)
        if len(items) < 100 or seen >= total:
            break
    done[key] = {"total": total, "seen": seen}
    jsave("candidates.json", cands)
    jsave("search-progress.json", done)
    print("%s -> total %s seen %d, repos so far %d" % (
        key, total, seen, len(cands)), flush=True)
    return total or 0


def cmd_find():
    """Each query is paged as it is; a query with more than 1000 results is
    also run once per file-size slice, to reach beyond the API's limit."""
    cands = jload("candidates.json", {})
    done = jload("search-progress.json", {})
    last = [0.0]
    for q in QUERIES:
        if q in done:
            total = done[q]["total"] or 0
        else:
            total = search_pages(q, cands, done, last)
        if total <= 1000:
            continue
        for sl in SIZE_SLICES:
            key = q + " size:" + sl
            if key not in done:
                search_pages(key, cands, done, last)
    print("repos found: %d" % len(cands))


# ---------------------------------------------------------------- fetch

REF_RE = re.compile(r"\bREFERENCES\b", re.I)
COMMENT_RE = re.compile(r"--[^\n]*|/\*.*?\*/", re.S)


def decode(b):
    if b[:2] in (b"\xff\xfe", b"\xfe\xff"):
        return b.decode("utf-16", "replace")
    return b.decode("utf-8-sig", "replace")


GQL_FIELDS = ("nameWithOwner createdAt isFork isArchived "
              "licenseInfo { spdxId } "
              "defaultBranchRef { name target { oid } }")


def graphql_meta(names):
    """Repo metadata for many repos in one GraphQL query (1 point)."""
    parts = []
    for i, fn in enumerate(names):
        o, n = fn.split("/", 1)
        parts.append('r%d: repository(owner: "%s", name: "%s") { %s }' % (
            i, o, n, GQL_FIELDS))
    q = "query { %s }" % " ".join(parts)
    for attempt in range(8):
        p = subprocess.run(["gh", "api", "graphql", "-f", "query=" + q],
                           capture_output=True)
        try:
            data = json.loads(p.stdout)["data"]
            return [data.get("r%d" % i) for i in range(len(names))]
        except Exception:
            low = (p.stderr + p.stdout).decode("utf-8", "replace").lower()
            if "rate limit" in low or "secondary" in low:
                print("  [rate] sleeping 65 s", flush=True)
                time.sleep(65)
            else:
                if attempt >= 1:
                    _enable_proxy()
                time.sleep(4 * (attempt + 1))
    raise GhError("graphql gave up")


def meta_record(fn, m):
    """Apply the metadata filters of rule step 2 to one GraphQL answer."""
    if m is None:
        return {"full_name": fn, "drop": "repo_unavailable"}
    lic = (m.get("licenseInfo") or {}).get("spdxId")
    if lic in ("NOASSERTION", ""):
        lic = None
    dbr = m.get("defaultBranchRef")
    rec = {"full_name": m["nameWithOwner"], "created_at": m["createdAt"],
           "licence": lic, "fork": m["isFork"], "archived": m["isArchived"]}
    if m["createdAt"][:10] < CREATED_FROM:
        rec["drop"] = "created_before_2025-07-01"
    elif m["isFork"]:
        rec["drop"] = "fork"
    elif m["isArchived"]:
        rec["drop"] = "archived"
    elif not dbr:
        rec["drop"] = "no_head_or_tree"
    else:
        rec["default_branch"] = dbr["name"]
        rec["commit"] = dbr["target"]["oid"]
    return rec


def blob_texts(fn, entries):
    """File texts by blob sha: batched GraphQL (many blobs a query), and the
    REST blob endpoint for the few that GraphQL gives no text for."""
    o, n = fn.split("/", 1)
    out, batch, size = {}, [], 0
    groups = []
    for e in entries:
        if batch and (len(batch) >= 40 or size + e["size"] > 2_000_000):
            groups.append(batch)
            batch, size = [], 0
        batch.append(e)
        size += e["size"]
    if batch:
        groups.append(batch)
    for g in groups:
        parts = ['b%d: object(oid: "%s") { ... on Blob { text } }' % (
            i, e["sha"]) for i, e in enumerate(g)]
        q = 'query { repository(owner: "%s", name: "%s") { %s } }' % (
            o, n, " ".join(parts))
        data = None
        for attempt in range(6):
            p = subprocess.run(["gh", "api", "graphql", "-f", "query=" + q],
                               capture_output=True)
            try:
                data = json.loads(p.stdout)["data"]["repository"]
                break
            except Exception:
                low = (p.stderr + p.stdout).decode("utf-8", "replace").lower()
                if "rate limit" in low or "secondary" in low:
                    print("  [rate] sleeping 65 s", flush=True)
                    time.sleep(65)
                else:
                    if attempt >= 1:
                        _enable_proxy()
                    time.sleep(3 * (attempt + 1))
        for i, e in enumerate(g):
            t = ((data or {}).get("b%d" % i) or {}).get("text")
            if t and chr(0) not in t:
                out[e["sha"]] = t.lstrip(chr(0xfeff))
            else:  # binary to GraphQL (e.g. UTF-16) or failed: REST
                try:
                    b = gh(["-H", "Accept: application/vnd.github.raw+json",
                            "repos/%s/git/blobs/%s" % (fn, e["sha"])],
                           raw=True)
                    out[e["sha"]] = decode(b)
                except GhError:
                    pass
    return out


def fetch_files(rec):
    """Pin HEAD, list its .sql files and choose the one with most REFERENCES."""
    fn = rec["full_name"]
    try:
        t = gh(["repos/%s/git/trees/%s?recursive=1" % (fn, rec["commit"])])
    except GhError:
        rec["drop"] = "no_head_or_tree"
        return rec
    rec["tree_truncated"] = bool(t.get("truncated"))
    files = sorted((e for e in t["tree"] if e["type"] == "blob"
                    and e["path"].lower().endswith(".sql")),
                   key=lambda e: e["path"])
    rec["sql_files"] = len(files)
    if not files:
        rec["drop"] = "no_sql_file"
        return rec
    best = None
    cand = [e for e in files[:MAX_SQL_FILES]
            if 0 < e.get("size", 0) <= MAX_BLOB]
    texts = blob_texts(fn, cand)
    for e in cand:
        text = texts.get(e["sha"])
        if text is None:
            continue
        n = len(REF_RE.findall(COMMENT_RE.sub(" ", text)))
        key = (-n, len(e["path"]), e["path"])
        if best is None or key < best[0]:
            best = (key, e["path"], e["sha"], text, n)
    if best is None or best[4] == 0:
        rec["drop"] = "no_sql_with_references"
        return rec
    rec.update(path=best[1], blob=best[2], refs=best[4])
    safe = fn.replace("/", "__")
    os.makedirs(os.path.join(WORK, "files"), exist_ok=True)
    with open(os.path.join(WORK, "files", safe + ".sql"), "w",
              encoding="utf-8", newline="") as f:
        f.write(best[3])
    return rec


def cmd_fetch():
    cands = jload("candidates.json", {})
    repos = jload("repos.json", {})
    # stage 1: metadata, 50 repos a GraphQL query; the cheap filters of step 2
    todo = sorted(fn for fn in cands if fn not in repos)
    print("metadata to fetch: %d of %d" % (len(todo), len(cands)), flush=True)
    for i in range(0, len(todo), 50):
        chunk = todo[i:i + 50]
        for fn, m in zip(chunk, graphql_meta(chunk)):
            repos[fn] = meta_record(fn, m)
        if (i // 50) % 10 == 9:
            jsave("repos.json", repos)
            print("  metadata %d/%d" % (i + 50, len(todo)), flush=True)
    jsave("repos.json", repos)
    # stage 2: HEAD tree and the chosen file, only for survivors, in the
    # order of step 6 (sha256 of owner/name), so that processing a prefix of
    # the pool is the same as taking the first repos of the kept ones.
    # `fetch N` stops after the first N survivors; rerun with a larger N
    # when fewer than 80 end up kept.
    limit = int(sys.argv[2]) if len(sys.argv) > 2 else None
    surv = sorted((fn for fn, r in repos.items() if "commit" in r),
                  key=lambda fn: hashlib.sha256(
                      repos[fn]["full_name"].encode()).hexdigest())
    need = [fn for fn in surv[:limit] if "path" not in repos[fn]
            and "drop" not in repos[fn]]
    print("survivors: %d, files to fetch: %d" % (len(surv), len(need)),
          flush=True)

    def one(fn):
        try:
            return fn, fetch_files(repos[fn])
        except Exception as e:
            print("  error %s: %s" % (fn, str(e)[:80]), flush=True)
            return None

    n = 0
    with ThreadPoolExecutor(4) as ex:
        for res in ex.map(one, need):
            if res is None:
                continue
            repos[res[0]] = res[1]
            n += 1
            if n % 10 == 0:
                jsave("repos.json", repos)
                print("  files %d/%d" % (n, len(need)), flush=True)
    jsave("repos.json", repos)
    print("done: %d repos with metadata" % len(repos))


# ---------------------------------------------------------------- parse

TOKEN_RE = re.compile(
    r"(?P<lc>--[^\n]*)|(?P<bc>/\*.*?\*/)|(?P<sq>'(?:[^']|'')*')|"
    r"(?P<dq>\"(?:[^\"]|\"\")*\")|(?P<bt>`[^`]*`)|"
    r"(?P<dl>\$(?P<tag>\w*)\$.*?\$(?P=tag)\$)|(?P<semi>;)|"
    r"(?P<go>^[ \t]*GO[ \t]*$)", re.S | re.M | re.I)


def split_statements(text):
    out, cur, pos = [], [], 0
    for m in TOKEN_RE.finditer(text):
        cur.append(text[pos:m.start()])
        pos = m.end()
        k = m.lastgroup
        if k in ("lc", "bc"):
            cur.append(" ")
        elif k in ("semi", "go"):
            out.append("".join(cur))
            cur = []
        else:
            cur.append(m.group(0))
    cur.append(text[pos:])
    out.append("".join(cur))
    return [s.strip() for s in out if s.strip()]


CREATE_TABLE_RE = re.compile(
    r"^create\s+(?:or\s+replace\s+)?(?:(?:global\s+|local\s+)?"
    r"(?:temporary|temp)\s+|unlogged\s+|virtual\s+)?table\b", re.I)
ALTER_TABLE_RE = re.compile(r"^alter\s+table\b", re.I)
CREATE_INDEX_RE = re.compile(
    r"^create\s+(?:unique\s+)?(?:clustered\s+|nonclustered\s+)?index\b", re.I)


def norm(name):
    return (name or "").strip().strip('`"[]').lower()


def family(typ):
    base = re.sub(r"\(.*?\)", "", typ).strip().lower()
    w = re.split(r"[\s\[]", base)[0] if base else ""
    if w in ("smallint", "integer", "int", "bigint", "serial", "bigserial",
             "smallserial", "tinyint", "mediumint", "int2", "int4", "int8"):
        return "int"
    if w in ("uuid", "uniqueidentifier"):
        return "uuid"
    if w in ("char", "varchar", "text", "citext", "nvarchar", "nchar",
             "string", "character", "tinytext", "mediumtext", "longtext",
             "ntext", "varchar2", "nvarchar2"):
        return "text"
    return "other"


def parse_file(text, dialect_list=DIALECTS):
    """Return (dialect, tables, fks) or None. tables: ordered dict."""
    import sqlglot
    from sqlglot import exp

    stmts = split_statements(text)
    ddl = [s for s in stmts if CREATE_TABLE_RE.match(s)
           or ALTER_TABLE_RE.match(s) or CREATE_INDEX_RE.match(s)]
    n_create = sum(1 for s in ddl if CREATE_TABLE_RE.match(s))
    best = None
    for d in dialect_list:
        parsed = []
        for s in ddl:
            try:
                e = sqlglot.parse_one(s, read=d)
            except Exception:
                continue
            if isinstance(e, (exp.Create, exp.Alter)):
                parsed.append(e)
        n_tab = sum(1 for e in parsed if isinstance(e, exp.Create)
                    and e.args.get("kind") == "TABLE"
                    and isinstance(e.this, exp.Schema))
        if n_tab == 0:
            continue
        if best is None:
            best = (d, parsed)
        if n_tab >= n_create:
            best = (d, parsed)
            break
    if best is None:
        return None
    d, parsed = best
    tables, fks, indexed = {}, [], {}

    def ids(node):
        return [norm(i.name) for i in node.find_all(exp.Identifier)]

    def ref_of(ref):
        t = ref.this
        if isinstance(t, exp.Schema):
            return norm(t.this.name), [norm(c.name) for c in t.expressions]
        if isinstance(t, exp.Table):
            return norm(t.name), []
        return None, []

    def add_constraint(tname, node, tab):
        if isinstance(node, exp.Constraint):
            for x in node.expressions:
                add_constraint(tname, x, tab)
        elif isinstance(node, exp.ForeignKey):
            ref = node.args.get("reference")
            if ref is not None:
                rt, _ = ref_of(ref)
                cols = [norm(c.name) for c in node.expressions]
                if rt and cols:
                    fks.append((tname, tuple(cols), rt))
        elif isinstance(node, exp.PrimaryKey):
            for c in [n for e in node.expressions for n in ids(e)]:
                if c not in tab["pk"]:
                    tab["pk"].append(c)
        elif isinstance(node, exp.UniqueColumnConstraint):
            cols = ids(node.this) if node.this is not None else []
            if len(cols) == 1:
                tab["unique"].add(cols[0])
        elif isinstance(node, exp.IndexColumnConstraint):
            for c in ids(node):
                indexed.setdefault(tname, set()).add(c)

    def add_column(tname, cd, tab):
        c = norm(cd.name)
        if not c or c in tab["cols"]:
            return
        typ = cd.args.get("kind")
        typ = typ.sql(dialect=d).lower() if typ is not None else ""
        col = {"name": c, "type": typ, "nullable": True, "unique": False,
               "primary": False}
        tab["cols"][c] = col
        for cc in cd.args.get("constraints") or []:
            k = cc.args.get("kind")
            if isinstance(k, exp.PrimaryKeyColumnConstraint):
                if c not in tab["pk"]:
                    tab["pk"].append(c)
            elif isinstance(k, exp.NotNullColumnConstraint):
                if not k.args.get("allow_null"):
                    col["nullable"] = False
            elif isinstance(k, exp.UniqueColumnConstraint):
                tab["unique"].add(c)
            elif isinstance(k, exp.Reference):
                rt, _ = ref_of(k)
                if rt:
                    fks.append((tname, (c,), rt))

    for e in parsed:
        if isinstance(e, exp.Create) and e.args.get("kind") == "TABLE":
            sch = e.this
            if not isinstance(sch, exp.Schema) or \
                    not isinstance(sch.this, exp.Table):
                continue
            tname = norm(sch.this.name)
            if not tname or tname in tables:
                continue  # first definition wins
            tab = {"cols": {}, "pk": [], "unique": set()}
            tables[tname] = tab
            for x in sch.expressions:
                if isinstance(x, exp.ColumnDef):
                    add_column(tname, x, tab)
            for x in sch.expressions:
                if not isinstance(x, exp.ColumnDef):
                    add_constraint(tname, x, tab)
        elif isinstance(e, exp.Create) and e.args.get("kind") == "INDEX":
            idx = e.this
            tb = idx.args.get("table") if idx is not None else None
            params = idx.args.get("params") if idx is not None else None
            if tb is not None and params is not None:
                for c in ids(params):
                    indexed.setdefault(norm(tb.name), set()).add(c)
    for e in parsed:  # ALTERs after every CREATE, so order does not matter
        if not isinstance(e, exp.Alter) or not isinstance(e.this, exp.Table):
            continue
        tname = norm(e.this.name)
        if tname not in tables:
            continue
        tab = tables[tname]
        for a in e.args.get("actions") or []:
            subs = a.expressions if isinstance(a, exp.AddConstraint) else [a]
            for x in subs:
                if isinstance(x, exp.ColumnDef):
                    add_column(tname, x, tab)
                else:
                    add_constraint(tname, x, tab)
    for tname, tab in tables.items():
        for c in tab["pk"]:
            if c in tab["cols"]:
                tab["cols"][c]["primary"] = True
                tab["cols"][c]["nullable"] = False
        for c in tab["unique"]:
            if c in tab["cols"]:
                tab["cols"][c]["unique"] = True
        for c in indexed.get(tname, ()):
            if c in tab["cols"]:
                tab["cols"][c]["index"] = True
    # resolve foreign keys
    seen, good = set(), []
    for t, cols, rt in fks:
        if (t, cols, rt) in seen or t not in tables or rt not in tables:
            continue
        if all(c in tables[t]["cols"] for c in cols):
            seen.add((t, cols, rt))
            good.append((t, cols, rt))
    return d, tables, good


def overlaps_sample(names):
    for k, s in SAMPLES.items():
        inter = len(names & s)
        if inter >= 3 and (inter / len(names) >= .5 or inter / len(s) >= .5):
            return k
    return None


def sha256_file(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        h.update(f.read())
    return h.hexdigest()


def cmd_parse():
    cands = jload("candidates.json", {})
    repos = jload("repos.json", {})
    st = {"repos_found": len(cands), "drops": {}}

    def drop(why):
        st["drops"][why] = st["drops"].get(why, 0) + 1

    # unique repos by their canonical full_name
    uniq = {}
    for rec in repos.values():
        uniq.setdefault(rec["full_name"], rec)
    st["repos_with_metadata"] = len(uniq)
    st["repos_without_metadata"] = len(cands) - len(
        {fn for fn in cands if fn in repos})
    stage = []
    st["survivors_not_fetched_beyond_prefix"] = 0
    for fn, rec in uniq.items():
        if "drop" in rec:
            drop(rec["drop"])
        elif "path" not in rec:
            st["survivors_not_fetched_beyond_prefix"] += 1
        else:
            stage.append(rec)
    st["after_repo_filters_with_a_chosen_file"] = len(stage)
    ok = []
    for rec in sorted(stage, key=lambda r: r["full_name"]):
        p = os.path.join(WORK, "files", rec["full_name"].replace("/", "__")
                         + ".sql")
        with open(p, encoding="utf-8", newline="") as f:
            text = f.read()
        try:
            r = parse_file(text)
        except Exception:
            r = None
        if r is None:
            drop("no_table_parsed")
            continue
        d, tables, fks = r
        names = set(tables)
        s = overlaps_sample(names)
        if s:
            drop("sample_database_overlap")
            continue
        if len(tables) < MIN_TABLES:
            drop("fewer_than_%d_tables" % MIN_TABLES)
            continue
        if len(fks) < MIN_FKS:
            drop("fewer_than_%d_resolved_fks" % MIN_FKS)
            continue
        rec = dict(rec, dialect=d, _tables=tables, _fks=fks, _text=text)
        ok.append(rec)
    st["parsed_and_passing_thresholds"] = len(ok)
    # exact duplicates: keep the earliest-created repository
    groups = {}
    for rec in ok:
        groups.setdefault(frozenset(rec["_tables"]), []).append(rec)
    kept = []
    for g in groups.values():
        g.sort(key=lambda r: (r["created_at"], r["full_name"]))
        kept.append(g[0])
        for _ in g[1:]:
            drop("duplicate_table_set")
    st["kept_after_dedupe"] = len(kept)
    # step 6: sha256 of owner/name, first MAX_REPOS
    kept.sort(key=lambda r: hashlib.sha256(
        r["full_name"].encode()).hexdigest())
    chosen = kept[:MAX_REPOS]
    st["not_selected_over_80"] = len(kept) - len(chosen)

    import shutil
    for sub in ("raw",):
        shutil.rmtree(os.path.join(OUT, sub), ignore_errors=True)
        os.makedirs(os.path.join(OUT, sub))
    manifest, lines = [], []
    n_tab = n_fk1 = n_fkc = 0
    for rec in chosen:
        fn = rec["full_name"]
        safe = fn.replace("/", "__")
        tables, fks = rec["_tables"], rec["_fks"]
        with open(os.path.join(OUT, "raw", safe + ".sql"), "w",
                  encoding="utf-8", newline="") as f:
            f.write(rec["_text"])
        manifest.append({
            "full_name": fn, "created_at": rec["created_at"],
            "default_branch": rec["default_branch"], "commit": rec["commit"],
            "path": rec["path"], "licence": rec["licence"],
            "html_url": "https://github.com/%s/blob/%s/%s" % (
                fn, rec["commit"], urllib.parse.quote(rec["path"])),
            "dialect": rec["dialect"]})
        tl = []
        for tn, tab in tables.items():
            cols = []
            for i, (cn, c) in enumerate(tab["cols"].items()):
                cols.append({"name": cn, "family": family(c["type"]),
                             "type": c["type"], "nullable": c["nullable"],
                             "unique": c["unique"], "primary": c["primary"],
                             "index": bool(c.get("index", False)),
                             "position": i})
            tl.append({"name": tn, "pk": [c for c in tab["pk"]
                                          if c in tab["cols"]],
                       "columns": cols})
        one = [[t, c[0], rt] for t, c, rt in fks if len(c) == 1]
        n_fkc += sum(1 for _, c, _ in fks if len(c) > 1)
        n_fk1 += len(one)
        n_tab += len(tl)
        rid = "github2025/" + safe
        lines.append(json.dumps({
            "id": rid, "repo": rid, "source": "github2025",
            "licence": rec["licence"] or "", "split": "external",
            "novel": False, "closed": [], "tables": tl, "fk": one},
            ensure_ascii=False))
    with open(os.path.join(OUT, "hidden.jsonl"), "w", encoding="utf-8",
              newline="\n") as f:
        f.write("".join(l + "\n" for l in lines))
    with open(os.path.join(OUT, "manifest.json"), "w", encoding="utf-8",
              newline="\n") as f:
        json.dump(manifest, f, indent=1, ensure_ascii=False)
    dates = sorted(m["created_at"] for m in manifest)
    lic = {}
    for m in manifest:
        k = m["licence"] or "none"
        lic[k] = lic.get(k, 0) + 1
    st.update(schemas=len(chosen), tables=n_tab, one_column_fks=n_fk1,
              composite_fks=n_fkc,
              created_at_min=dates[0] if dates else None,
              created_at_max=dates[-1] if dates else None, licences=lic)
    with open(os.path.join(OUT, "summary.json"), "w", encoding="utf-8",
              newline="\n") as f:
        json.dump(st, f, indent=1)
    with open(os.path.join(OUT, "sha256.txt"), "w", encoding="utf-8",
              newline="\n") as f:
        f.write("%s  hidden.jsonl\n" % sha256_file(
            os.path.join(OUT, "hidden.jsonl")))
        for fnm in sorted(os.listdir(os.path.join(OUT, "raw"))):
            f.write("%s  raw/%s\n" % (sha256_file(
                os.path.join(OUT, "raw", fnm)), fnm))
    print(json.dumps(st, indent=1))


if __name__ == "__main__":
    cmds = {"find": cmd_find, "fetch": cmd_fetch, "parse": cmd_parse}
    if len(sys.argv) not in (2, 3) or sys.argv[1] not in cmds:
        sys.exit("usage: github.py find | fetch [N] | parse")
    cmds[sys.argv[1]]()
