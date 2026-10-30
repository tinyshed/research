"""Round 1n's row of numbers against the gold rows Haiku wrote for round 1m's
bench (gold.py): a gold number is found when the row holds a number of the
same kind over the same table — its counted column too, where both name one,
and its window, where the gold gives one. A share is found by its top.

    python -I match.py <bench dir>

runs the row on each bench database (round 1's containers) and writes
<bench dir>/match-1o.json.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
for d in ("round-1c", "round-1n"):
    sys.path.insert(0, str(ROOT / d))
import kpi  # noqa: E402
import see  # noqa: E402
from aq import load  # noqa: E402

KINDS = {"count": {"count"}, "count distinct": {"count distinct"}, "two or more": {"two or more"},
         "sum": {"sum"}, "average": {"sum"}, "share": {"share"}}


def found(g: dict, tiles: list) -> bool:
    for t in tiles:
        if t.what not in KINDS.get(g.get("what"), set()) or t.table != g.get("table"):
            continue
        if g.get("what") in ("count distinct", "two or more", "sum", "average") and g.get("column") and t.column \
                and t.column != g["column"]:
            continue
        if g.get("days") and t.days != g["days"] and g.get("what") != "share":
            continue
        return True
    return False


def main(bench: Path) -> None:
    per, total, hit, ours, used = {}, 0, 0, 0, 0
    for spec in sorted(bench.glob("m*/numbers.json")):
        key = spec.parent.name
        gold = json.loads(spec.read_text(encoding="utf-8"))["numbers"]
        snap = json.loads((ROOT / "corpus" / "see" / f"story{key}-postgres" / "snapshot.json").read_text(encoding="utf-8"))
        tiles = [t for t in kpi.row(load(snap, True), snap["engine"], snap["database"], see.run_sql) if t.what != "funnel"]
        marks = [found(g, tiles) for g in gold]
        used_here = sum(1 for t in tiles if any(found(g, [t]) for g in gold))
        per[key] = {"gold": len(gold), "found": sum(marks), "ours": len(tiles), "ours in gold": used_here,
                    "missed": [f"{g.get('what')} {g.get('table')}" + (f".{g['column']}" if g.get("column") else "")
                               + (f", {g['days']} days" if g.get("days") else "") for g, m in zip(gold, marks) if not m]}
        total, hit, ours, used = total + len(gold), hit + sum(marks), ours + len(tiles), used + used_here
        print(f"{key}: {sum(marks)} of {len(gold)} gold numbers found; {used_here} of our {len(tiles)} in the gold; "
              f"missed: {'; '.join(per[key]['missed'][:4])}", flush=True)
    result = {"gold numbers found": round(hit / max(total, 1), 3), "of": total,
              "our numbers in the gold": round(used / max(ours, 1), 3), "ours": ours, "per database": per}
    (bench / "match-1o.json").write_text(json.dumps(result, indent=1, ensure_ascii=False), encoding="utf-8")
    print(f"\nfound {hit} of {total} gold numbers ({hit / max(total, 1):.0%}); {used} of our {ours} numbers in the gold "
          f"({used / max(ours, 1):.0%})")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
