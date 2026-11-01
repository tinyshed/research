"""Round 1p: the key model on the sets Nexus (Cong et al., arXiv
2602.08186) reports its metadata-only XGBoost on — TPC-H, TPC-DS and BIRD's
eleven development databases pooled — for the comparison its Table 3 asks.

    .venv/Scripts/python -I nexus.py <models> <round 1p out> <corpus> [<variant>]

TPC-H and TPC-DS are CTU's `tpch` and `tpcds`; BIRD's development databases
are the names in its dev archive. Nexus scores an undirected join graph at a
threshold tuned on each set; we give F1 over declared one-column keys at
round 1b's validation threshold and, as Nexus tuned, at the best threshold of
each set. Whether each set's tables occur in SchemaPile-Perm, which the model
was trained on, is counted beside it.
"""

import json
import sys
import zipfile
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import numpy as np  # noqa: E402

import external  # noqa: E402

NEXUS = {"tpch": 0.35, "tpcds": 0.44, "bird dev": 0.10}  # XGBoost alone, Table 3


def main(models: Path, out: Path, corpus: Path, variant: str | None) -> None:
    build = None
    if variant:
        import widen  # noqa: E402
        build = lambda job: widen.build((*job, variant))  # noqa: E731
    with zipfile.ZipFile(corpus / "bird/raw/dev/dev_databases.zip") as z:
        dev = {Path(n).stem for n in z.namelist() if n.endswith(".sqlite") and not n.startswith("__MACOSX")}
    sets = {"tpch": {"ctu/tpch"}, "tpcds": {"ctu/tpcds"}, "bird dev": {f"bird/{d}" for d in dev}}
    schemas = [json.loads(line) for line in open(out / "hidden.jsonl", encoding="utf-8")]
    chosen = [s for s in schemas if any(s["id"] in ids for ids in sets.values())]
    run, graph = external.scorer(models, build)
    import keys  # noqa: E402
    scores, meta = run(chosen, {})
    kept = keys.one_target(scores, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
    threshold = json.load(open(models / "graph-final.json", encoding="utf-8"))["threshold"]
    perm = external.perm_strings(corpus)
    result = {}
    for name, ids in sets.items():
        mine = [s for s in chosen if s["id"] in ids]
        truth = {(s["id"], a, c, b) for s in mine for a, c, b in s["fk"]}
        rows = [i for i, m in enumerate(meta) if m[graph.SID] in ids]
        cand = {(meta[i][graph.SID], meta[i][graph.TABLE], meta[i][graph.COLUMN], meta[i][graph.TARGET]): kept[i]
                for i in rows}

        def f1_at(t):
            drawn = {k for k, v in cand.items() if v >= t}
            tp = len(drawn & truth)
            p = tp / len(drawn) if drawn else 0.0
            r = tp / len(truth) if truth else 0.0
            return p, r, 2 * p * r / (p + r) if p + r else 0.0

        best = max((f1_at(t) + (t,) for t in sorted(set(cand.values())) if t > 0), key=lambda x: x[2],
                   default=(0, 0, 0, 0))
        tables = [t for s in mine for t in s["tables"]]
        in_perm = sum(1 for t in tables if external.table_string(t["name"], [c["name"] for c in t["columns"]], True)
                      in perm)
        result[name] = {
            "databases": len(mine), "tables": len(tables), "tables occurring in Perm": in_perm,
            "one-column keys declared": len(truth),
            "at round 1b's threshold": dict(zip(("precision", "recall", "f1"), f1_at(threshold))),
            "at this set's best threshold": dict(zip(("precision", "recall", "f1", "threshold"), best)),
            "Nexus's XGBoost, F1": NEXUS[name],
        }
    name = f"nexus-{variant.replace(' ', '-')}.json" if variant else "nexus.json"
    (out / name).write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4] if len(sys.argv) > 4 else None)
