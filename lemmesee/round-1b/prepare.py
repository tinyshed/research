"""Round 1b's data: SchemaPile-Perm's schemas with their declarations hidden,
the labels the declarations give, and the split by repository.

    .venv/Scripts/python -I prepare.py <corpus> <out>

writes into <out>:
  hidden.jsonl     one schema a line: what a guesser may see, its labels, its split
  snapshots.jsonl  the same schemas as round 1's snapshots, for the rules' `guess`
  summary.json     counts, and what was left out and why
"""

import gzip
import hashlib
import json
import random
import re
import sys
from collections import Counter
from pathlib import Path

SEED = 20261027
MAX_TABLES = 100  # larger schemas make most of the pairs; left out and counted

INT = {
    "int", "integer", "bigint", "smallint", "tinyint", "mediumint", "unsignedint", "unsignedbigint",
    "unsignedsmallint", "unsignedtinyint", "unsignedmediumint", "int2", "int4", "int8", "serial",
    "bigserial", "smallserial", "number", "tech_id",
}
LONG_TEXT = {"text", "longtext", "mediumtext", "clob", "nclob", "ntext"}
DECIMAL = {"decimal", "numeric", "money", "smallmoney", "udecimal"}
FLOAT = {"float", "double", "real", "doubleprecision", "float4", "float8", "binary_double", "binary_float"}
BOOL = {"boolean", "bool", "bit", "boolean_char"}


def family(raw: str) -> str:
    t = raw.lower()
    if t in INT:
        return "int"
    if t == "uuid":
        return "uuid"
    if t in LONG_TEXT:
        return "longtext"
    if "char" in t or t in {"enum", "string", "tinytext", "code", "set"}:
        return "text"
    if t in DECIMAL:
        return "decimal"
    if t in FLOAT:
        return "float"
    if t in BOOL:
        return "bool"
    if "time" in t or t in {"date", "datetime", "datetime2", "smalldatetime"}:
        return "date" if t == "date" else "time"
    return "other"


# The base type round 1's snapshot would have shown, so the rules read it
BASE = {
    "int": "integer", "uuid": "uuid", "longtext": "text", "text": "varchar", "decimal": "numeric",
    "float": "double precision", "bool": "boolean", "time": "timestamp", "date": "date",
}
KIND = {"int": "int", "uuid": "uuid", "text": "text"}  # the kinds a key may be of


def repository(url: str) -> str:
    m = re.match(r"https://github\.com/([^/]+)/([^/]+)/", url)
    return f"{m.group(1)}/{m.group(2)}".lower() if m else url


def closed_list(column: dict) -> bool:
    if column["TYPE"].lower() == "enum":
        return True
    for check in column.get("CHECKS") or []:
        c = str(check).lower()
        if re.search(r"\bin\s*\(", c) or "any" in c:
            return True
    return False


def main(corpus: Path, out: Path) -> None:
    data = json.load(gzip.open(corpus / "schemapile-perm/12682521/schemapile-perm.json.gz", "rt", encoding="utf-8"))
    out.mkdir(parents=True, exist_ok=True)
    left_out = Counter()

    schemas = []
    for sid, s in data.items():
        tables = s["TABLES"]
        if len(tables) > MAX_TABLES:
            left_out["schemas over 100 tables"] += 1
            continue
        hidden_tables, fk, closed = [], [], []
        for tname, t in tables.items():
            pk = list(t["PRIMARY_KEYS"] or [])
            indexed = {c for idx in (t["INDEXES"] or []) for c in (idx if isinstance(idx, list) else [idx])}
            columns = []
            for position, (cname, c) in enumerate(t["COLUMNS"].items()):
                fam = family(c["TYPE"])
                columns.append({
                    "name": cname,
                    "family": fam,
                    "type": c["TYPE"].lower(),
                    "nullable": c["NULLABLE"],
                    "unique": bool(c["UNIQUE"]),
                    "primary": cname in pk,
                    "index": bool(c["IS_INDEX"]) or cname in indexed,
                    "position": position,
                })
                if closed_list(c):
                    closed.append([tname, cname])
            for f in t["FOREIGN_KEYS"] or []:
                if len(f["COLUMNS"]) != 1:
                    left_out["foreign keys over several columns"] += 1
                    continue
                target = next((n for n in tables if n.lower() == (f["FOREIGN_TABLE"] or "").lower()), None)
                if target is None:
                    left_out["foreign keys to no table of the schema"] += 1
                    continue
                fk.append([tname, f["COLUMNS"][0], target])
            hidden_tables.append({"name": tname, "pk": pk, "columns": columns})
        signature = hashlib.sha256(json.dumps(
            sorted([t["name"].lower(), sorted(c["name"].lower() for c in t["columns"])] for t in hidden_tables)
        ).encode()).hexdigest()[:16]
        schemas.append({
            "id": sid, "repo": repository(s["INFO"]["URL"]), "licence": s["INFO"]["LICENSE"],
            "signature": signature, "tables": hidden_tables, "fk": fk, "closed": closed,
        })

    repos = sorted({s["repo"] for s in schemas})
    random.Random(SEED).shuffle(repos)
    n = len(repos)
    split_of = {r: ("train" if i < 0.7 * n else "validation" if i < 0.8 * n else "test") for i, r in enumerate(repos)}
    train_signatures = {s["signature"] for s in schemas if split_of[s["repo"]] == "train"}
    for s in schemas:
        s["split"] = split_of[s["repo"]]
        if s["split"] != "train" and s["signature"] in train_signatures:
            s["split"] = "dropped"
            left_out[f"schemas whose signature is also in training"] += 1

    # A schema is novel when no training schema shares half its tables'
    # names: the template schemas copied from repository to repository —
    # petclinic, a framework's own tables — are then measured apart.
    names = lambda s: {"".join(ch for ch in t["name"].lower() if ch.isalnum()) for t in s["tables"]}
    by_name = {}
    train_names = []
    for s in schemas:
        if s["split"] == "train":
            i = len(train_names)
            train_names.append(names(s))
            for n in train_names[i]:
                by_name.setdefault(n, []).append(i)
    for s in schemas:
        if s["split"] in ("validation", "test"):
            mine = names(s)
            seen = {i for n in mine for i in by_name.get(n, [])}
            best = max((len(mine & train_names[i]) / len(mine | train_names[i]) for i in seen), default=0.0)
            s["novel"] = best < 0.5

    with open(out / "hidden.jsonl", "w", encoding="utf-8") as h, open(out / "snapshots.jsonl", "w", encoding="utf-8") as sn:
        for s in schemas:
            h.write(json.dumps(s, ensure_ascii=False) + "\n")
            sn.write(json.dumps(snapshot(s), ensure_ascii=False) + "\n")

    by_split = Counter(s["split"] for s in schemas)
    summary = {
        "seed": SEED,
        "schemas": len(schemas),
        "repositories": len(repos),
        "by_split": dict(by_split),
        "novel_tests": sum(1 for s in schemas if s["split"] == "test" and s.get("novel")),
        "foreign_keys": sum(len(s["fk"]) for s in schemas),
        "closed_lists": sum(len(s["closed"]) for s in schemas),
        "schemas_declaring_a_foreign_key": sum(1 for s in schemas if s["fk"]),
        "left_out": dict(left_out),
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=2))


def snapshot(s: dict) -> dict:
    """The schema as round 1's snapshot shows it with nothing declared but
    its primary keys and indexes."""
    tables = []
    for t in s["tables"]:
        columns = [{
            "name": c["name"], "position": c["position"] + 1,
            "type_name": BASE.get(c["family"], "unknown"), "base_type": BASE.get(c["family"], "unknown"),
            "nullable": c["nullable"] is not False, "can_select": True, "features": {},
        } for c in t["columns"]]
        constraints = [{"name": f"{t['name']}_pkey", "kind": "p", "columns": t["pk"]}] if t["pk"] else []
        uniques = [{"name": f"{t['name']}_{c['name']}_key", "kind": "u", "columns": [c["name"]]}
                   for c in t["columns"] if c["unique"] and not c["primary"]]
        indexes = [{"name": f"{t['name']}_{c['name']}_idx", "columns": [c["name"]], "unique": False, "primary": False}
                   for c in t["columns"] if c["index"]]
        tables.append({
            "schema": "public", "name": t["name"], "kind": "table", "can_select": True, "features": {},
            "columns": columns, "constraints": constraints + uniques, "indexes": indexes,
        })
    return {
        "version": 1, "engine": "schemapile", "server_version": "", "database": s["id"], "role": "",
        "taken_at": "", "settings": {}, "workload": {}, "tables": tables, "queries": [],
    }


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]))
