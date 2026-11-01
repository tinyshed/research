"""Round 1q: the key models on the owner's two production databases, from
the snapshots round 1k took (schema and statistics, read-only, nothing a
user wrote), their declared keys hidden. Reported as counts only: no
table's or column's name is printed or written.

    .venv/Scripts/python -I prod.py <prod corpus> <round 1p out> <out> <database>...

writes <out>/prod.json: for each database and each variant, keys declared,
found, drawn wrongly, and precision, recall and F1.
"""

import json
import sys
from pathlib import Path

ROUND_1P = Path(__file__).resolve().parent.parent / "round-1p"
sys.path.insert(0, str(ROUND_1P))

import external  # noqa: E402
import keys  # noqa: E402
import widen  # noqa: E402

VARIANTS = {"reported": ("models-1524905", None), "both": ("variants/both", "both"),
            "both, roles": ("variants/both,-roles", "both, roles")}


def hidden(snapshot: dict, name: str) -> dict:
    """A snapshot as round 1p's hidden schema: names, types, nullability,
    one-column uniqueness and primary keys; no index, no declaration."""
    tables, fk = [], []
    for t in snapshot["tables"]:
        if t["kind"] != "table":
            continue
        cons = t.get("constraints", [])
        pk = next((c["columns"] for c in cons if c["kind"] == "p"), [])
        unique = {c["columns"][0] for c in cons if c["kind"] == "u" and len(c["columns"]) == 1}
        columns = []
        for i, c in enumerate(t["columns"]):
            fam, typ = external.sql_family(c["base_type"], "postgres")
            columns.append({"name": c["name"], "family": fam, "type": typ, "nullable": c["nullable"],
                            "unique": c["name"] in unique and c["name"] not in pk, "primary": c["name"] in pk,
                            "index": False, "position": i})
        for c in cons:
            if c["kind"] == "f" and len(c["columns"]) == 1:
                fk.append([t["name"], c["columns"][0], c["ref_table"].split(".")[-1]])
        tables.append({"name": t["name"], "pk": pk, "columns": columns})
    return {"id": f"prod/{name}", "repo": f"prod/{name}", "source": "prod", "split": "external",
            "novel": True, "tables": tables, "fk": fk, "closed": []}


def main(prod: Path, variants_root: Path, out: Path, databases: list[str]) -> None:
    schemas = [hidden(json.load(open(prod / db / "snapshot.json", encoding="utf-8")), db) for db in databases]
    result = {}
    for label, (folder, variant) in VARIANTS.items():
        models = variants_root / folder
        build = (lambda job, v=variant: widen.build((*job, v))) if variant else None
        run, graph = external.scorer(models, build)
        threshold = json.load(open(models / "graph-final.json", encoding="utf-8"))["threshold"]
        scores, meta = run(schemas, {})
        kept = keys.one_target(scores, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
        for s in schemas:
            truth = {tuple(k) for k in s["fk"]}
            drawn = {(m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]) for m, k in zip(meta, kept)
                     if m[graph.SID] == s["id"] and k >= threshold}
            covered = {(m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]) for m in meta
                       if m[graph.SID] == s["id"] and m[graph.LABEL]}
            tp, fp = len(drawn & truth), len(drawn - truth)
            p = tp / len(drawn) if drawn else 0.0
            r = tp / len(truth) if truth else 0.0
            result[f"{s['id']}, {label}"] = {
                "tables": len(s["tables"]), "keys declared": len(truth), "keys among candidates": len(covered),
                "found": tp, "drawn wrongly": fp, "precision": p, "recall": r,
                "f1": 2 * p * r / (p + r) if p + r else 0.0}
    out.mkdir(parents=True, exist_ok=True)
    (out / "prod.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    for k, v in result.items():
        print(f"{k:28} declared {v['keys declared']:3} candidates {v['keys among candidates']:3} found {v['found']:3} "
              f"wrong {v['drawn wrongly']:3} P {v['precision']:.3f} R {v['recall']:.3f} F1 {v['f1']:.3f}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4:])
