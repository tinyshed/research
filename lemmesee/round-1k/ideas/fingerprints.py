"""Memorise rather than generalise: databases are built of the same tables
again and again, so an answer written once for a table seen in public
schemas could be carried as a table of fingerprints. Before anything is
written, how often does a table have a near copy among SchemaPile-Perm's?

    python -I fingerprints.py <out>

A table is the set of its column names, lower case, letters and digits
only; two tables are near by the Jaccard of those sets. Every table of the
judged databases, of the GitTables pilot and of the two production
snapshots (counted, never named) is matched against every table of the
22 989 SchemaPile-Perm schemas. No model is fitted.
"""

from __future__ import annotations

import gzip
import json
import re
import sys
from pathlib import Path

import numpy as np
from scipy import sparse

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
BANDS = (0.5, 0.7, 0.9)


def norm(columns) -> frozenset[str]:
    return frozenset(x for x in (re.sub(r"[^a-z0-9]", "", c.lower()) for c in columns) if x)


def schemapile() -> list[frozenset[str]]:
    path = next((ROOT / "corpus" / "schemapile-perm").rglob("*.json.gz"))
    data = json.load(gzip.open(path, "rt", encoding="utf-8"))
    return [s for v in data.values() for t in (v.get("TABLES") or {}).values()
            if len(s := norm((t.get("COLUMNS") or {}).keys())) >= 3]


def snapshots(paths) -> list[list[frozenset[str]]]:
    out = []
    for p in paths:
        snap = json.loads(Path(p).read_text(encoding="utf-8"))
        out.append([s for t in snap["tables"] if len(s := norm(c["name"] for c in t["columns"])) >= 3])
    return out


def best(index, vocab, sizes, query: frozenset[str]) -> float:
    cols = [vocab[w] for w in query if w in vocab]
    if not cols:
        return 0.0
    q = np.zeros(len(vocab), dtype=np.float32)
    q[cols] = 1
    overlap = index @ q
    jac = overlap / (sizes + len(query) - overlap)
    return float(jac.max())


def main(out: Path) -> None:
    pile = schemapile()
    vocab = {w: i for i, w in enumerate(sorted({w for s in pile for w in s}))}
    rows = np.repeat(np.arange(len(pile)), [len(s) for s in pile])
    cols = np.fromiter((vocab[w] for s in pile for w in s), dtype=np.int64)
    index = sparse.csr_matrix((np.ones(len(cols), dtype=np.float32), (rows, cols)), shape=(len(pile), len(vocab)))
    sizes = np.asarray([len(s) for s in pile], dtype=np.float32)
    groups = {
        "judged databases": sorted((ROOT / "corpus" / "see").glob("*/snapshot.json")),
        "GitTables pilot": sorted((ROOT / "corpus" / "round-1k" / "gt" / "see").glob("*/snapshot.json")),
        "production": sorted((ROOT / "corpus" / "prod").rglob("snapshot.json")),
    }
    result = {"schemapile tables": len(pile)}
    for name, paths in groups.items():
        per_db = [[best(index, vocab, sizes, t) for t in tables] for tables in snapshots(paths)]
        flat = [x for db in per_db for x in db]
        if not flat:
            continue
        result[name] = {"databases": len(per_db), "tables": len(flat),
                        **{f"tables with a neighbour at {b}": round(float(np.mean([x >= b for x in flat])), 3) for b in BANDS},
                        **{f"databases with half their tables at {b}": round(float(np.mean([np.mean([x >= b for x in db]) >= 0.5
                                                                                            for db in per_db if db])), 3)
                           for b in BANDS}}
    out.mkdir(parents=True, exist_ok=True)
    (out / "fingerprints.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
