"""Semantic layers on GitHub: analysts wrote, by hand, which column of a
table is summed, which averaged, which counted once, which is a dimension
and which a time. LookML views, dbt semantic models and Malloy sources are
that judgment made by people, the one thing every judge of lemmesee lacks.

    python -I semantic.py list <out> [lookml|dbt|malloy]...   # code search: paths only
    python -I semantic.py licence <out>                       # each repository's licence
    python -I semantic.py fetch <out>                         # contents of permissive files
    python -I semantic.py parse <out>                         # measures and dimensions

GitHub's code search answers at most a thousand results a query and ten
queries a minute, so each query is cut by file size until every part fits,
and paced. Contents are read by blob sha through `gh api`, only from
repositories whose licence lets them be used; nothing is committed. GitHub
lets public, non-personal information be used for research whose
publications are open access, and asks that its servers not be burdened.
"""

from __future__ import annotations

import base64
import json
import re
import subprocess
import sys
import time
from pathlib import Path

QUERIES = {
    "lookml": ['extension:lkml "measure:" "type: sum"', 'extension:lkml "measure:" "type: average"',
               'extension:lkml "measure:" "type: count_distinct"'],
    "dbt": ['"semantic_models:" "measures:" extension:yml', '"semantic_models:" "measures:" extension:yaml'],
    "malloy": ['extension:malloy "measure:"'],
}
SIZES = [(0, 1000), (1000, 2000), (2000, 3000), (3000, 5000), (5000, 8000), (8000, 12000), (12000, 20000),
         (20000, 40000), (40000, 100000), (100000, 400000)]
PERMISSIVE = {"MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unlicense", "CC0-1.0", "CC-BY-4.0", "0BSD", "Zlib"}


def gh(path: str, **params) -> dict:
    args = ["gh", "api", "-X", "GET", path] + [x for k, v in params.items() for x in ("-f", f"{k}={v}")]
    for attempt in range(5):
        p = subprocess.run(args, capture_output=True, text=True, encoding="utf-8")
        if p.returncode == 0:
            return json.loads(p.stdout)
        if "rate limit" in p.stderr.lower() or "403" in p.stderr or "429" in p.stderr:
            time.sleep(65)
            continue
        if not p.stderr.strip():
            # a secondary limit answers with no message; wait and ask again
            time.sleep(30)
            continue
        raise RuntimeError(p.stderr.strip()[:200])
    raise RuntimeError("rate limited five times")


def list_files(out: Path, kinds: list[str]) -> None:
    seen = out / "files.jsonl"
    have = {json.loads(l)["sha"] for l in seen.read_text(encoding="utf-8").splitlines()} if seen.exists() else set()
    with seen.open("a", encoding="utf-8") as f:
        for kind in kinds:
            for q in QUERIES[kind]:
                for lo, hi in SIZES:
                    query = f"{q} size:{lo}..{hi - 1}"
                    total = None
                    for page in range(1, 11):
                        d = gh("search/code", q=query, per_page=100, page=page)
                        time.sleep(6.5)  # ten searches a minute
                        total = d.get("total_count") if total is None else total
                        for item in d.get("items", []):
                            if item["sha"] in have:
                                continue
                            have.add(item["sha"])
                            f.write(json.dumps({"kind": kind, "repo": item["repository"]["full_name"], "path": item["path"],
                                                "sha": item["sha"]}) + "\n")
                        if len(d.get("items", [])) < 100:
                            break
                    print(f"{kind} {query}: {total} found{' (more than a query returns)' if (total or 0) > 1000 else ''}",
                          flush=True)


def licences(out: Path) -> None:
    files = [json.loads(l) for l in (out / "files.jsonl").read_text(encoding="utf-8").splitlines()]
    path = out / "licences.json"
    lic = json.loads(path.read_text(encoding="utf-8")) if path.exists() else {}
    for repo in sorted({f["repo"] for f in files} - set(lic)):
        try:
            lic[repo] = (gh(f"repos/{repo}").get("license") or {}).get("spdx_id")
        except RuntimeError:
            lic[repo] = None
        time.sleep(0.8)
        path.write_text(json.dumps(lic, indent=0), encoding="utf-8")
    print(f"{len(lic)} repositories, {sum(v in PERMISSIVE for v in lic.values())} permissive")


def fetch(out: Path) -> None:
    files = [json.loads(l) for l in (out / "files.jsonl").read_text(encoding="utf-8").splitlines()]
    lic = json.loads((out / "licences.json").read_text(encoding="utf-8"))
    store = out / "files"
    store.mkdir(exist_ok=True)
    n = 0
    for f in files:
        if lic.get(f["repo"]) not in PERMISSIVE or (store / f["sha"]).exists():
            continue
        try:
            blob = gh(f"repos/{f['repo']}/git/blobs/{f['sha']}")
            (store / f["sha"]).write_bytes(base64.b64decode(blob["content"]))
            n += 1
        except RuntimeError as e:
            print(f"{f['repo']}/{f['path']}: {e}", flush=True)
        time.sleep(0.8)
    print(f"{n} files fetched")


FIELD_START = re.compile(r"\b(measure|dimension_group|dimension):\s*(\w+)\s*\{")
FIELD_SQL = re.compile(r"sql:\s*(.*?);;", re.S)


def blocks(text: str):
    """Each LookML field's kind, name and body, braces counted: `${col}`
    and Liquid's `{{ }}` close what they open, so the body ends where its
    own brace does."""
    for m in FIELD_START.finditer(text):
        depth, i = 1, m.end()
        while i < len(text) and depth:
            depth += {"{": 1, "}": -1}.get(text[i], 0)
            i += 1
        yield m.group(1), m.group(2), text[m.end():i - 1]
FIELD_TYPE = re.compile(r"type:\s*(\w+)")
REF = re.compile(r"\$\{(?:TABLE\}\.)?\"?`?(\w+)")


def parse(out: Path) -> None:
    """Every measure as (its aggregate, the column it reads, its own name),
    and every dimension as (its type, its column); LookML and dbt only."""
    files = [json.loads(l) for l in (out / "files.jsonl").read_text(encoding="utf-8").splitlines()]
    rows = []
    for f in files:
        p = out / "files" / f["sha"]
        if not p.exists():
            continue
        text = p.read_text(encoding="utf-8", errors="replace")
        if f["kind"] == "lookml":
            for kind, name, body in blocks(text):
                t, s = FIELD_TYPE.search(body), FIELD_SQL.search(body)
                rows.append({"repo": f["repo"], "role": kind, "type": t.group(1) if t else None, "name": name,
                             "columns": REF.findall(s.group(1)) if s else []})
        elif f["kind"] == "dbt":
            # line by line: a list item's name, then its agg and expr before the next item
            item: dict | None = None
            for line in text.splitlines() + ["- name: end"]:
                m = re.match(r"\s*-\s*name:\s*['\"]?(\w+)", line)
                if m:
                    if item and item.get("agg"):
                        rows.append({"repo": f["repo"], "role": "measure", "type": item["agg"], "name": item["name"],
                                     "columns": [item.get("expr") or item["name"]]})
                    item = {"name": m.group(1)}
                    continue
                m = re.match(r"\s*(agg|expr):\s*['\"]?([\w.]+)", line)
                if item is not None and m:
                    item[m.group(1)] = m.group(2)
    (out / "fields.jsonl").write_text("\n".join(json.dumps(r) for r in rows) + "\n", encoding="utf-8")
    from collections import Counter
    print(len(rows), "fields;", Counter((r["role"], r["type"]) for r in rows).most_common(12))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, out, *rest = sys.argv[1:]
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    {"list": lambda: list_files(out, rest or list(QUERIES)), "licence": lambda: licences(out),
     "fetch": lambda: fetch(out), "parse": lambda: parse(out)}[cmd]()
