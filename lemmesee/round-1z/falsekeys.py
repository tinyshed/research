"""Round 1z: what a local model's false keys are, by cause.

    python -I falsekeys.py <out.json> <tag folder> [...]

For each model's replies on plain A and style 3 (corpus/round-1z/llm/<tag
folder>/<set>/), every key the reply names that no declaration names is put
in the first class it fits:

    names nothing        a table or column the schema does not hold
    its own primary key  the column is its table's whole primary key and is said to refer to that table
    reversed             the declarations hold a key from the named target into this table
    wrong table          the column is a declared key, into another table
    self-reference       an undeclared column said to refer to its own table
    a primary key column an undeclared column that is (part of) its table's primary key
    not a declared key   any other column no declaration names

and the entries a reply repeats for one column (dropped by the reader, one
key a column) are counted beside. The share of false keys in the ten
schemas with the most of them says whether a few schemas carry the figure.
"""

import json
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1p"))

import gemini  # noqa: E402

CORPUS = HERE.parent / "corpus"
SETS = {"plain-a-db": "plain-a", "style3-db": "style3"}


def classify(d: Path, schemas: list) -> dict:
    c, per_schema, dup, entries, right = Counter(), Counter(), 0, 0, 0
    examples = {}
    for s in schemas:
        raw = d / f"{gemini.db_stem(s['id'])}.raw"
        rec = json.load(open(raw.with_suffix(".json"), encoding="utf-8"))
        obj = gemini.last_object(raw.read_text(encoding="utf-8", errors="replace"), "keys") if rec["finish"] == "stop" else None
        if obj is None:
            continue
        names = {t["name"].lower(): t for t in s["tables"]}
        truth = {tuple(k) for k in s["fk"]}
        by_col = {(k[0], k[1]): k[2] for k in s["fk"]}
        pairs = {(k[0], k[2]) for k in s["fk"]}
        seen = set()
        for e in obj.get("keys") or []:
            if not isinstance(e, dict):
                continue
            entries += 1
            t = names.get(str(e.get("table", "")).lower())
            to = names.get(str(e.get("references", "")).lower())
            col = next((x for x in t["columns"] if x["name"].lower() == str(e.get("column", "")).lower()), None) if t else None
            if not (t and to and col):
                what = "names nothing"
            else:
                k = (t["name"], col["name"], to["name"])
                if (k[0], k[1]) in seen:
                    dup += 1
                    continue
                seen.add((k[0], k[1]))
                if k in truth:
                    right += 1
                    continue
                pk = list(t.get("pk") or [])
                if pk == [col["name"]] and to["name"] == t["name"]:
                    what = "its own primary key"
                elif (to["name"], t["name"]) in pairs:
                    what = "reversed"
                elif (k[0], k[1]) in by_col:
                    what = "wrong table"
                elif to["name"] == t["name"]:
                    what = "self-reference"
                elif col["name"] in pk:
                    what = "a primary key column"
                else:
                    what = "not a declared key"
            c[what] += 1
            per_schema[s["id"]] += 1
            examples.setdefault(what, [])
            if len(examples[what]) < 6:
                examples[what].append([str(e.get("table")), str(e.get("column")), str(e.get("references"))])
    false = sum(c.values())
    top = sum(n for _, n in per_schema.most_common(10))
    return {"entries": entries, "right": right, "false": false, "repeated for one column, dropped": dup,
            "by cause": dict(c.most_common()), "false keys in the ten schemas with the most": top,
            "schemas with a false key": len(per_schema), "examples": examples}


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    out = {}
    for tag in sys.argv[2:]:
        out[tag] = {}
        for folder, which in SETS.items():
            schemas = [json.loads(line) for line in open(CORPUS / "round-1v" / f"heldout-github-{which}" / "hidden.jsonl",
                                                         encoding="utf-8")]
            schemas = [s for s in schemas if s["fk"]]
            out[tag][which] = r = classify(CORPUS / "round-1z" / "llm" / tag / folder, schemas)
            print(tag, which, {k: v for k, v in r.items() if k != "examples"})
    Path(sys.argv[1]).write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
