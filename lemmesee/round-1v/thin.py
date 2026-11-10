"""Round 1v: how much of a schema's keys its declarations hold, by the
number of its tables, read two ways on SchemaPile-Perm's training split.

    python -I thin.py <out.json>

- **the name rules** (round 1's, as round 1b kept their guesses): of the
  columns the rules name a key — a column named after a table and `id`, the
  plainest keys there are — the share the schema declares, and the share of
  schemas that declare every one of them;
- **the two teachers** (teach.py, where both answered): the keys both point
  at the same table that no declaration names, beside the declared keys.

Schemas that declare at least one key, as the models are trained on them.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent / "corpus"
BINS = [(0, 5), (5, 10), (10, 20), (20, 40), (40, 101)]


def main(out: Path) -> None:
    guesses = defaultdict(dict)
    for line in open(ROOT / "round-1b" / "guesses.jsonl", encoding="utf-8"):
        g = json.loads(line)
        if g.get("fk"):
            guesses[g["schema"]][(g["table"], g["column"])] = g["fk"]
    taught = json.load(open(ROOT / "round-1v" / "lm" / "taught-more.json", encoding="utf-8"))
    rows = {b: defaultdict(int) for b in BINS}
    with open(ROOT / "round-1b" / "hidden.jsonl", encoding="utf-8") as f:
        for line in f:
            s = json.loads(line)
            if s["split"] != "train" or not s["fk"]:
                continue
            n = len(s["tables"])
            r = rows[next(b for b in BINS if b[0] <= n < b[1])]
            declared = {(a, c): g for a, c, g in s["fk"]}
            named = guesses.get(s["id"], {})
            so = sum(1 for k, g in named.items() if declared.get(k) == g)
            r["schemas"] += 1
            r["declared keys"] += len(declared)
            r["rule-named columns"] += len(named)
            r["rule-named columns declared so"] += so
            r["schemas declaring every rule-named column"] += so == len(named)
            t = taught.get(s["id"])
            if t is not None:
                r["schemas both teachers answered"] += 1
                r["declared keys there"] += len(declared)
                r["keys both teachers add"] += len(t["both"])
                r["columns one teacher names"] += len(t["one"])
    result = {}
    for b, r in rows.items():
        name = f"{b[0]}-{b[1] - 1}"
        e = dict(r)
        e["rule-named columns, declared share"] = r["rule-named columns declared so"] / max(r["rule-named columns"], 1)
        e["schemas declaring every rule-named column, share"] = \
            r["schemas declaring every rule-named column"] / max(r["schemas"], 1)
        if r["schemas both teachers answered"]:
            e["taught keys a declared key"] = r["keys both teachers add"] / max(r["declared keys there"], 1)
            # with the judge's 202 of 220 taught keys real (judge.py), the share of a schema's keys its declarations hold
            e["declared share of the keys, the taught ones at 0.918"] = r["declared keys there"] / (
                r["declared keys there"] + 0.918 * r["keys both teachers add"])
        result[name] = e
        print(f"tables {name:>6}: schemas {r['schemas']:5d} keys {r['declared keys']:6d} | rule-named declared "
              f"{e['rule-named columns, declared share']:.3f}, schemas declaring all {e['schemas declaring every rule-named column, share']:.3f}"
              + (f" | teachers: {r['schemas both teachers answered']} schemas, {r['keys both teachers add']} taught beside "
                 f"{r['declared keys there']} declared, declared share {e['declared share of the keys, the taught ones at 0.918']:.3f}"
                 if r["schemas both teachers answered"] else ""))
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
