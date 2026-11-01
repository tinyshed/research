"""Round 1q: how the two variants score CTU's Mooney_Family, whose keys are
named for a role (`name1` → `person`).

    .venv/Scripts/python -I mooney.py <round 1p out> <out>

writes <out>/mooney-family.json: for each variant, the declared keys among
its candidates, how many it puts first, and their scores.
"""

import json
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "round-1p"))

import external  # noqa: E402
import keys  # noqa: E402
import widen  # noqa: E402


def main(data: Path, out: Path) -> None:
    pick = [s for s in map(json.loads, open(data / "hidden.jsonl", encoding="utf-8")) if s["id"] == "ctu/Mooney_Family"]
    result = {}
    for folder, variant in (("both", "both"), ("both,-roles", "both, roles")):
        run, graph = external.scorer(data / "variants" / folder, lambda job, v=variant: widen.build((*job, v)))
        scores, meta = run(pick, {})
        kept = keys.one_target(scores, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
        true = sorted(float(kept[i]) for i, m in enumerate(meta) if m[graph.LABEL])
        result[variant] = {"declared keys among candidates": len(true),
                           "right target first": sum(1 for x in true if x > 0),
                           "median score": round(statistics.median(true), 4),
                           "mean score": round(statistics.mean(true), 4), "max score": round(max(true), 4)}
    out.mkdir(parents=True, exist_ok=True)
    (out / "mooney-family.json").write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]))
