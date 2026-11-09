"""Round 1t: the style set's keys sorted by the shape of their names, and
each model's recall on each shape; added on 9 November 2026, after the one
reading of the set, which is development data from then on.

    python -I shapes.py

A key's shape, from its column's name and its target's (letters and digits,
lower-cased): a key word first (`id_pessoa`, `idpais`, `fk_projet`,
`cod_x`) when the name begins with one and does not end in `id`; ending in
`id`; the target's name (the column the target's name, its singular, or a
part of it); another key word last (`ref`, `key`, `code`, `no` ...); any
other. Writes corpus/round-1t/errors/githubstyle-shapes.json, through
errors.py's sorting of every key of `1s`, `seven-restyled` and
`seven-bounded` (corpus/round-1t/errors/<model>--githubstyle-all.json).
"""

import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import cues  # noqa: E402,F401  (first, as in fit.py)
import letters  # noqa: E402

import errors  # noqa: E402
import fit  # noqa: E402
import graph  # noqa: E402

MODELS = ("1s", "seven-restyled", "seven-bounded")


def shape(col: str, target: str) -> str:
    n = re.sub(r"[^a-z0-9]", "", col.lower())
    t = re.sub(r"[^a-z0-9]", "", target.lower())
    if re.match(r"^(id|fk|cod|cd|code|key|ref|num|nr|no)[_]?", col.lower()) and not n.endswith("id"):
        return "key word first (id_x, fk_x, cod_x)"
    if n.endswith("id"):
        return "ends in id"
    if n == t or n == t.rstrip("s") or t.endswith(n) or n.endswith(t.rstrip("s")):
        return "the target's name"
    if re.search(r"(key|code|ref|no|num|nr|fk|uuid|uid)$", n):
        return "another key word last"
    return "other"


def main() -> None:
    schemas = [json.loads(line) for line in open(fit.OUT / "heldout-github-style" / "hidden.jsonl", encoding="utf-8")]
    errors.SHOWN = 10 ** 7
    found, rows, outside = {}, [], set()
    with letters.pool(graph.WORKERS) as pool:
        for n in MODELS:
            errors.one(n, "githubstyle-all", schemas, "external", {}, pool)
            e = json.load(open(fit.OUT / "errors" / f"{n}--githubstyle-all.json", encoding="utf-8"))
            found[n] = {(x["database"], x["table"], x["column"]) for x in e["shown"]["found"]}
            if n == "1s":
                rows = [x for k in ("found", "below the threshold", "won by another target", "not a candidate")
                        for x in e["shown"][k]]
                outside = {(x["database"], x["table"], x["column"]) for x in e["shown"]["not a candidate"]}
    by = defaultdict(Counter)
    for x in rows:
        s = shape(x["column"], x["target"])
        k = (x["database"], x["table"], x["column"])
        by[s]["keys"] += 1
        by[s]["not a candidate"] += k in outside
        for n in found:
            by[s][n] += k in found[n]
    out = {s: {"keys": c["keys"], "share": round(c["keys"] / len(rows), 3), "not a candidate": c["not a candidate"],
               **{f"recall, {n}": round(c[n] / c["keys"], 3) for n in found}}
           for s, c in sorted(by.items(), key=lambda kv: -kv[1]["keys"])}
    print(json.dumps(out, indent=1))
    (fit.OUT / "errors" / "githubstyle-shapes.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    assert letters.is_sorted()
    main()
