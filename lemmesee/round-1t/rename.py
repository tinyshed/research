"""Round 1t: real SchemaPile training schemas renamed by a language model, their
keys carried over by the map.

    python -I rename.py prepare <out>
    python -I rename.py apply <out>

`prepare` takes 400 of SchemaPile's training schemas with keys (5 to 30
tables, at most 250 columns, one a repository, first by SHA-256 of the
schema's id) and writes, ten a file, prompts that show each schema and its
declared keys and ask for a map of new names in a house style drawn for it
from STYLES; `apply` reads the agents' replies, keeps a map that renames
every table and column to distinct names, and writes the renamed schemas,
each beside its original's repository so that the folds keep them together,
to <out>/renamed-llm.jsonl. The teacher decides no key: the declared keys
follow their columns through the map.

Proposed on 8 November 2026 by a design agent asked how language models could
teach the key model offline; the styles and the prompt are fixed here before
any reply is read.
"""

import hashlib
import json
import random
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent / "corpus"
SEED = 20261108
N, EACH = 400, 10

STYLES = [
    "entities renamed by synonyms a different team would use (driver -> pilot, customer -> client, track -> circuit), references named by the role the referenced row plays (approved_by, owner, home_team, origin)",
    "references named after the referenced table's key with a different ending (key zone_registry, reference zone_ref), entity words shortened as a busy team would (dept, cust, txn, qty)",
    "a legacy enterprise system: names of at most 8 characters, cryptic abbreviations and codes, upper case (CUSTNO, ORDHDR, SHPDT)",
    "run-together lower-case words without separators (custorderref, shipdate), domain jargon instead of plain words",
    "names transliterated into another language's words (German, Spanish, Portuguese or Russian), keys and references in that language's habits",
    "a mix grown over years: each table in its own convention, some references by role, some by synonym, some by the target's key, endings such as ref, link, fk, no, code, key",
]

PROMPT = """You rename relational database schemas so that a key-finding tool can learn other naming habits. For each schema below, choose new names for EVERY table and EVERY column in the house style given for it. Keep the meaning of every table and column (a reader who knows the domain must still be able to tell what each is), keep names distinct within a table and tables distinct within the schema, and rename the foreign-key columns in the same style as the rest (do NOT keep a pattern like <table>_id unless the style asks for it). Do not add or remove anything.

{schemas}

Output one JSON object and nothing else, keyed by schema number:
{{"1": {{"tables": {{"old table": "new table", ...}}, "columns": {{"old table.old column": "new column", ...}}}}, "2": ...}}"""


def text_of(s: dict) -> str:
    lines = []
    for t in s["tables"]:
        cols = ", ".join(f"{c['name']} {c.get('type') or c['family']}" + (" PK" if c["primary"] else "")
                         for c in t["columns"])
        lines.append(f"  {t['name']}({cols})")
    keys = "; ".join(f"{a}.{c} -> {b}" for a, c, b in s["fk"])
    return "\n".join(lines) + f"\n  foreign keys: {keys}"


def chosen():
    seen, out = set(), []
    rows = []
    for line in open(ROOT / "round-1b" / "hidden.jsonl", encoding="utf-8"):
        s = json.loads(line)
        if s["split"] != "train" or not s["fk"]:
            continue
        n_cols = sum(len(t["columns"]) for t in s["tables"])
        if not (5 <= len(s["tables"]) <= 30) or n_cols > 250:
            continue
        rows.append((hashlib.sha256(s["id"].encode()).hexdigest(), s))
    for _, s in sorted(rows, key=lambda r: r[0]):
        if s["repo"] in seen:
            continue
        seen.add(s["repo"])
        out.append(s)
        if len(out) == N:
            break
    return out


def prepare(out: Path) -> None:
    rng = random.Random(SEED)
    schemas = chosen()
    d = out / "rename" / "prompts"
    d.mkdir(parents=True, exist_ok=True)
    plan = []
    for k in range(0, len(schemas), EACH):
        batch = schemas[k:k + EACH]
        parts = []
        for i, s in enumerate(batch, 1):
            style = rng.choice(STYLES)
            plan.append({"file": k // EACH, "n": i, "id": s["id"], "style": style})
            parts.append(f"Schema {i}. House style: {style}.\n{text_of(s)}")
        (d / f"rename-{k // EACH:03d}.txt").write_text(PROMPT.format(schemas="\n\n".join(parts)), encoding="utf-8")
    (out / "rename" / "plan.json").write_text(json.dumps(plan, indent=0) + "\n", encoding="utf-8")
    print(len(schemas), "schemas,", len(range(0, len(schemas), EACH)), "prompts")


def apply(out: Path) -> None:
    by_id = {s["id"]: s for s in chosen()}
    plan = json.load(open(out / "rename" / "plan.json", encoding="utf-8"))
    refused, rows = Counter(), []
    for p in plan:
        f = out / "rename" / "replies" / f"rename-{p['file']:03d}.json"
        if not f.exists():
            refused["no reply"] += 1
            continue
        try:
            m = json.load(open(f, encoding="utf-8")).get(str(p["n"]))
        except (json.JSONDecodeError, AttributeError):
            refused["unreadable reply"] += 1
            continue
        s = by_id[p["id"]]
        if not isinstance(m, dict):
            refused["no map"] += 1
            continue
        tmap, cmap = m.get("tables", {}), m.get("columns", {})
        try:
            tables, cols = {}, {}
            for t in s["tables"]:
                new = str(tmap[t["name"]]).strip()
                tables[t["name"]] = new
                names = []
                for c in t["columns"]:
                    nc = str(cmap[f"{t['name']}.{c['name']}"]).strip()
                    cols[(t["name"], c["name"])] = nc
                    names.append(nc.lower())
                if len(set(names)) < len(names) or not all(names):
                    raise ValueError("two columns alike")
            if len({v.lower() for v in tables.values()}) < len(tables) or not all(tables.values()):
                raise ValueError("two tables alike")
        except (KeyError, ValueError, TypeError) as e:
            refused[f"map incomplete or clashing ({type(e).__name__})"] += 1
            continue
        new_tables = [{**t, "name": tables[t["name"]], "pk": [cols[(t["name"], x)] for x in t["pk"]],
                       "columns": [{**c, "name": cols[(t["name"], c["name"])]} for c in t["columns"]]}
                      for t in s["tables"]]
        fk = [[tables[a], cols[(a, c)], tables[b]] for a, c, b in s["fk"]]
        rows.append({**s, "id": s["id"] + "~llm", "tables": new_tables, "fk": fk, "closed": [],
                     "renamed by": "haiku", "style": p["style"]})
    with open(out / "renamed-llm.jsonl", "w", encoding="utf-8") as w:
        for r in rows:
            w.write(json.dumps(r, ensure_ascii=False) + "\n")
    ends = Counter((r["fk"] and r["fk"][0][1].lower().endswith("id")) for r in rows)
    print(json.dumps({"renamed": len(rows), "refused": refused, "first key ends in id": ends}, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    {"prepare": prepare, "apply": apply}[sys.argv[1]](Path(sys.argv[2]))
