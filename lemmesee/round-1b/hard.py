"""The hard cases three language-model agents wrote, with their relations
known, converted for the rules and the key model. They are never trained
on nor used to choose a model; graph.py's `final` measures both on them.

    .venv/Scripts/python -I hard.py prepare <out>   hard/*.json to <out>/hard-hidden.jsonl and hard-snapshots.jsonl
"""

import json
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))


from prepare import snapshot  # noqa: E402

FAMILY = {"int": "int", "bigint": "int", "varchar": "text", "char": "text", "text": "longtext", "uuid": "uuid",
          "decimal": "decimal", "float": "float", "timestamp": "time", "date": "date", "boolean": "bool"}


def convert(path: Path) -> list[dict]:
    data = json.load(open(path, encoding="utf-8"))
    out = []
    for s in data["schemas"]:
        tables = []
        for t in s["tables"]:
            columns = [{
                "name": name, "family": FAMILY[typ], "type": typ, "nullable": None, "unique": False,
                "primary": name in t["pk"], "index": False, "position": i,
            } for i, (name, typ) in enumerate(t["columns"])]
            tables.append({"name": t["name"], "pk": t["pk"], "columns": columns})
        out.append({
            "id": s["id"], "repo": f"synthetic/{data['style']}", "style": data["style"], "domain": s.get("domain"),
            "split": "hard", "novel": True, "signature": s["id"], "tables": tables,
            "fk": s.get("fk", []), "closed": [], "decoys": s.get("not_fk", []),
        })
    return out


def prepare(out: Path) -> None:
    schemas = [s for f in sorted((Path(__file__).parent / "hard").glob("*.json")) for s in convert(f)]
    with open(out / "hard-hidden.jsonl", "w", encoding="utf-8") as h, \
            open(out / "hard-snapshots.jsonl", "w", encoding="utf-8") as sn:
        for s in schemas:
            h.write(json.dumps(s, ensure_ascii=False) + "\n")
            sn.write(json.dumps(snapshot(s), ensure_ascii=False) + "\n")
    by_style = Counter(s["style"] for s in schemas)
    print(f"{len(schemas)} schemas, {sum(len(s['fk']) for s in schemas)} keys, "
          f"{sum(len(s['decoys']) for s in schemas)} decoys: {dict(by_style)}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    {"prepare": prepare}[sys.argv[1]](Path(sys.argv[2]))
