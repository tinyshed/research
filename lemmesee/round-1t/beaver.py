"""BEAVER schemas with declared keys, prepared as a held-out test of the key model.

Source: the Hugging Face dataset `target-benchmark/beaver-corpus`, a table
corpus built from BEAVER (Chen et al., "BEAVER: an enterprise benchmark for
text-to-sql", arXiv 2409.02038): six enterprise databases, one parquet row a
table, with `primary_key`, `foreign_keys` and `column_types` in each row's
`context`. Only the card and that parquet are fetched: no query logs, no
sqlite files, no example rows are kept.

Pinned revision: 8b683135f567ed6bef6a6d74cc90dc8de37fc997 (last modified
2025-06-26).

Licence, as each source states it:
  - the card of target-benchmark/beaver-corpus: no licence in its metadata and
    none in its text;
  - the official release (github.com/peterbaile/beaver, LICENSE file, and the
    metadata of the Hugging Face dataset beaverbench/beaver-table): MIT.
The records carry LICENCE below, which says both.

Commands (run with the interpreter of `corpus/round-1t/.venv-beaver`, which
has pyarrow; `fetch` needs only the standard library):

    python -I beaver.py fetch   # download card + parquet into raw/, write sha256.txt
    python -I beaver.py parse   # raw/ -> hidden.jsonl and summary.json

If connections reset, set HTTPS_PROXY=http://localhost:2080 first.

Output goes to `lemmesee/corpus/round-1t/heldout-beaver/` (git-ignored). The
schemas are held out: nothing here prints a table or column name.
"""

import hashlib
import json
import os
import re
import sys
import urllib.request
from collections import defaultdict
from pathlib import Path

REPO = "target-benchmark/beaver-corpus"
REVISION = "8b683135f567ed6bef6a6d74cc90dc8de37fc997"
FILES = ["README.md", "data/test-00000-of-00001.parquet"]
LICENCE = (
    "none stated by the target-benchmark/beaver-corpus card; "
    "MIT in the official BEAVER release (peterbaile/beaver, beaverbench/beaver-table)"
)

OUT = Path(__file__).resolve().parent.parent / "corpus" / "round-1t" / "heldout-beaver"
RAW = OUT / "raw"


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def fetch():
    RAW.mkdir(parents=True, exist_ok=True)
    lines = []
    for name in FILES:
        url = f"https://huggingface.co/datasets/{REPO}/resolve/{REVISION}/{name}"
        dest = RAW / Path(name).name
        # urllib honours HTTPS_PROXY from the environment.
        with urllib.request.urlopen(url, timeout=300) as r, open(dest, "wb") as f:
            while True:
                block = r.read(1 << 20)
                if not block:
                    break
                f.write(block)
        lines.append(f"{sha256(dest)}  raw/{dest.name}")
        print("fetched", dest.name, dest.stat().st_size, "bytes")
    (OUT / "sha256.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")


def family(type_text):
    """Map a SQL type to int / uuid / text / other."""
    t = type_text.strip().lower()
    base = re.split(r"[\s(]", t, maxsplit=1)[0]
    if base == "uuid":
        return "uuid"
    if base in ("int", "integer", "bigint", "smallint", "tinyint", "mediumint", "int4", "int8"):
        return "int"
    if base in ("number", "numeric", "decimal"):
        # number without scale is an integer; number(p,s) with s > 0 is not.
        m = re.match(r"\w+\s*\(\s*(\d+)\s*(?:,\s*(-?\d+)\s*)?\)", t)
        if m is None:
            return "int" if base == "number" else "other"
        return "int" if int(m.group(2) or 0) == 0 else "other"
    if base in ("char", "varchar", "varchar2", "nvarchar", "nvarchar2", "nchar",
                "text", "string", "tinytext", "mediumtext", "longtext"):
        return "text"
    return "other"


def parse():
    import pyarrow.parquet as pq

    # The files must be the ones the hashes name.
    for line in (OUT / "sha256.txt").read_text(encoding="utf-8").splitlines():
        digest, rel = line.split("  ", 1)
        if sha256(OUT / rel) != digest:
            sys.exit(f"hash mismatch: {rel}")

    rows = pq.read_table(
        RAW / "test-00000-of-00001.parquet",
        columns=["database_id", "table_id", "context"],
    ).to_pylist()
    dbs = defaultdict(dict)
    for r in rows:
        dbs[r["database_id"]][r["table_id"]] = r["context"]

    n_tables = n_columns = n_one = n_comp_groups = n_comp_entries = n_unres = n_dup = 0
    types_present = True
    records = []
    for db in sorted(dbs):
        tables, fks = [], []
        seen = set()
        for tname, ctx in dbs[db].items():
            names = ctx["column_names_cleaned"]
            types = ctx["column_types"]
            pk = list(ctx["primary_key"] or [])
            if len(types) != len(names) or not any(t for t in types):
                types_present = False
            cols = []
            for i, c in enumerate(names):
                ty = (types[i] if i < len(types) and types[i] else "").lower()
                cols.append({
                    "name": c,
                    "family": family(ty) if ty else "other",
                    "type": ty,
                    # BEAVER states no NOT NULL: only key columns are non-null.
                    "nullable": c not in pk,
                    "unique": False,
                    "primary": c in pk,
                    "index": False,
                    "position": i,
                })
            tables.append({"name": tname, "pk": pk, "columns": cols})
            n_tables += 1
            n_columns += len(cols)
        # The corpus has no constraint ids. Entries of one table to one
        # referenced table that name different referenced columns are read as
        # one composite key; the rest are separate one-column keys.
        for tname, ctx in dbs[db].items():
            cols = set(ctx["column_names_cleaned"])
            groups = defaultdict(list)
            for f in ctx["foreign_keys"] or []:
                groups[f["referenced_table_name"]].append(f)
            for rt, group in groups.items():
                ok = rt in dbs[db] and all(f["column_name"] in cols for f in group)
                if not ok:
                    n_unres += len(group)
                    continue
                if len({f["referenced_column_name"] for f in group}) > 1:
                    n_comp_groups += 1
                    n_comp_entries += len(group)
                    continue
                for f in group:
                    key = (tname, f["column_name"], rt)
                    if key in seen:
                        n_dup += 1
                        continue
                    seen.add(key)
                    fks.append(list(key))
                    n_one += 1
        records.append({
            "id": f"beaver/{db}",
            "repo": f"beaver/{db}",
            "source": "beaver",
            "licence": LICENCE,
            "split": "external",
            "novel": False,
            "closed": [],
            "tables": tables,
            "fk": fks,
        })

    with open(OUT / "hidden.jsonl", "w", encoding="utf-8", newline="\n") as f:
        for rec in records:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
    summary = {
        "databases": len(records),
        "tables": n_tables,
        "columns": n_columns,
        "one_column_keys": n_one,
        "composite_keys": n_comp_groups,
        "composite_key_entries": n_comp_entries,
        "unresolved_keys": n_unres,
        "duplicate_one_column_entries_dropped": n_dup,
        "types_present": types_present,
        "revision": REVISION,
        "source": f"huggingface.co/datasets/{REPO}",
        "licence": LICENCE,
        "composite_rule": "same table, same referenced table, more than one distinct referenced column",
    }
    (OUT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in summary.items() if isinstance(v, (int, bool))}))


if __name__ == "__main__":
    if len(sys.argv) != 2 or sys.argv[1] not in ("fetch", "parse"):
        sys.exit("usage: beaver.py fetch|parse")
    os.chdir(Path(__file__).resolve().parent)
    {"fetch": fetch, "parse": parse}[sys.argv[1]]()
