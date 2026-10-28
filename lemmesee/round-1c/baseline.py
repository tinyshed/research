"""Round 1's rules measured as round 1c's ranker is: their five for each
teacher schema, as analytical questions, against the teachers' ten.

    python -I baseline.py <rules binary> <teacher dir> <out> [<first key> [<last key>]]
"""

from __future__ import annotations

import json
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import gold  # noqa: E402
import learn  # noqa: E402
from aq import load  # noqa: E402


def ident(schema, c: dict):
    """A round 1 candidate as (base, measure, time, breakdown)."""
    base = gold.table_of(schema, c["table"])
    if base is None or c["template"] in ("T7", "T8"):
        return None
    m = c.get("measure")
    if c["template"] == "T5" and not m and c.get("join"):
        f = c["join"]["from"]
        r = gold.ref(schema, f"{f['table']}.{f['column']}")
        measure = ("per", frozenset({r})) if r else ("count", frozenset())
    elif m:
        r = gold.ref(schema, f"{m['table']}.{m['column']}")
        measure = (c["aggregate"], frozenset({r})) if r else None
    else:
        measure = ("count", frozenset())
    t = c.get("time")
    time = gold.ref(schema, f"{t['table']}.{t['column']}") if t else None
    g = c.get("category")
    by = gold.breakdown(schema, base, f"{g['table']}.{g['column']}") if g else None
    if by == "unknown":
        by = None
    return (base, measure, time, by)


def main(binary: Path, tdir: Path, out: Path, first: str = "", last: str = "~") -> None:
    out.mkdir(parents=True, exist_ok=True)
    answers = learn.teacher_answers(tdir)
    res = defaultdict(list)
    for line in (tdir / "schemas.jsonl").open(encoding="utf-8"):
        snap = json.loads(line)
        key = snap["key"]
        if not first <= key < last:
            continue
        have = {t: answers[t][key] for t in learn.TEACHERS if key in answers[t]}
        if not have:
            continue
        path = out / f"{key}.json"
        path.write_text(json.dumps(snap), encoding="utf-8")
        d = out / key
        subprocess.run([str(binary), "--snapshot", str(path), "--out", str(d)], check=True, capture_output=True)
        cands = {c["id"]: c for c in json.loads((d / "candidates.json").read_text(encoding="utf-8"))["candidates"]}
        panels = [p["id"] for p in json.loads((d / "panels-A.json").read_text(encoding="utf-8"))["panels"]]
        schema = load(snap, reads_n=False)
        ours = {ident(schema, cands[i]) for i in panels} - {None}
        every = {ident(schema, c) for c in cands.values()} - {None}
        for t, tq in have.items():
            asked = [gold.identity(schema, q)[0] for q in tq]
            ten = {g for g in asked if g is not None}
            res[f"{t} questions round 1 can say"].append(sum(1 for g in asked if g in every) / len(asked))
            res[f"{t} precision"].append(len(ours & ten) / 5)
            res[f"{t} recall"].append(sum(1 for g in asked[:5] if g in ours) / 5)
    print(json.dumps({k: sum(v) / len(v) for k, v in sorted(res.items())} | {"schemas": len(res["haiku precision"]) or len(res["gemini precision"])}, indent=1))


if __name__ == "__main__":
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), *sys.argv[4:6])
