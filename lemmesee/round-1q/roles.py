"""Round 1q: the variant `both, roles` fitted as round 1p fitted its
variants and measured on validation, beside `both`'s figures from round 1p.

    .venv/Scripts/python -I roles.py <round 1b out> <variants out>
"""

import json
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "round-1p"))

import widen  # noqa: E402
import graph  # noqa: E402
import keys  # noqa: E402

DECLARED = 7636  # one-column keys the validation schemas declare


def every_key_f1(result):
    v = result["validation"]
    tp = v["recall"] * result["validation keys among candidates"]
    r = tp / DECLARED
    return 2 * v["precision"] * r / (v["precision"] + r)


def main(data: Path, out: Path) -> None:
    schemas, guesses = keys.load(data)
    with ProcessPoolExecutor(graph.WORKERS) as pool:
        result, whole, second = widen.fit(pool, schemas, guesses, "both, roles")
    d = out / "both,-roles"
    d.mkdir(parents=True, exist_ok=True)
    whole.save_model(str(d / "keys-first.ubj"))
    second.booster_.save_model(str(d / "keys-second.txt"))
    result["F1 over every declared validation key"] = every_key_f1(result)
    base = json.load(open(out / "both" / "graph-final.json", encoding="utf-8"))
    result["both, for comparison"] = {"F1 over every declared validation key": every_key_f1(base),
                                      "the paper's task": base["validation, the paper's task"]}
    (d / "graph-final.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]))
