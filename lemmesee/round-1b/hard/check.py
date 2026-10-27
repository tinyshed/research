"""Checks a file of hard cases: valid JSON, every key points at a table of
its schema whose primary key is one column of a compatible type.

    python -I check.py <file.json>
"""
import json
import sys

TYPES = {"int", "bigint", "varchar", "char", "text", "uuid", "decimal", "float", "timestamp", "date", "boolean"}
KIND = {"int": "int", "bigint": "int", "varchar": "text", "char": "text", "text": "text", "uuid": "uuid"}

data = json.load(open(sys.argv[1], encoding="utf-8"))
errors = []
ids = set()
for s in data["schemas"]:
    sid = s["id"]
    if sid in ids:
        errors.append(f"{sid}: id repeated")
    ids.add(sid)
    tables = {t["name"]: t for t in s["tables"]}
    for t in s["tables"]:
        cols = {c[0]: c[1] for c in t["columns"]}
        for name, typ in cols.items():
            if typ not in TYPES:
                errors.append(f"{sid}.{t['name']}.{name}: type {typ} not in {sorted(TYPES)}")
        for p in t["pk"]:
            if p not in cols:
                errors.append(f"{sid}.{t['name']}: primary key {p} is not a column")
    for table, column, target in s.get("fk", []):
        if table not in tables or column not in {c[0] for c in tables[table]["columns"]}:
            errors.append(f"{sid}: key {table}.{column} is not a column")
            continue
        if target not in tables:
            errors.append(f"{sid}: key {table}.{column} points at {target}, not a table")
            continue
        pk = tables[target]["pk"]
        if len(pk) != 1:
            errors.append(f"{sid}: {target} has no one-column primary key")
            continue
        a = KIND.get(dict(map(tuple, tables[table]["columns"]))[column])
        b = KIND.get(dict(map(tuple, tables[target]["columns"]))[pk[0]])
        if a is None or a != b:
            errors.append(f"{sid}: {table}.{column} and {target}.{pk[0]} are not of one kind")
    for table, column in s.get("not_fk", []):
        if table not in tables or column not in {c[0] for c in tables[table]["columns"]}:
            errors.append(f"{sid}: decoy {table}.{column} is not a column")
print(f"{len(data['schemas'])} schemas, {sum(len(s.get('fk', [])) for s in data['schemas'])} keys, "
      f"{sum(len(s.get('not_fk', [])) for s in data['schemas'])} decoys, {len(errors)} errors")
for e in errors[:30]:
    print(" ", e)
sys.exit(1 if errors else 0)
