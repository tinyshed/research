"""Round 1p: the chosen variant fitted again under other seeds — the
negatives kept, both stages' random states — to say how far one fit is from
another on validation and on round 1p's databases.

    .venv/Scripts/python -I seeds.py <round 1b out> <variants out> <variant> <round 1p out> <seed>...

Each seed is fitted as widen.py fits a variant and saved under
<variants out>/<variant>-seed-<seed>/; its external scores go to round 1p's
out as score-seed-<seed>.json.
"""

import json
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import widen  # noqa: E402  (puts round 1b on the path)
import graph  # noqa: E402
import keys  # noqa: E402


def main(data: Path, out: Path, variant: str, round_1p: Path, seeds: list[int]) -> None:
    import external as ext  # noqa: E402
    schemas, guesses = keys.load(data)
    first, second, seed = dict(graph.FIRST_GPU), dict(graph.SECOND), widen.SEED
    with ProcessPoolExecutor(graph.WORKERS) as pool:
        for s in seeds:
            graph.FIRST_GPU = {**first, "random_state": s}
            graph.SECOND = {**second, "random_state": s}
            widen.SEED = s
            result, whole, model2 = widen.fit(pool, schemas, guesses, variant)
            result["seed"] = s
            d = out / f"{variant.replace(' ', '-')}-seed-{s}"
            d.mkdir(parents=True, exist_ok=True)
            whole.save_model(str(d / "keys-first.ubj"))
            model2.booster_.save_model(str(d / "keys-second.txt"))
            (d / "graph-final.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
            print(json.dumps(result), flush=True)
    graph.FIRST_GPU, graph.SECOND, widen.SEED = first, second, seed
    for s in seeds:
        d = out / f"{variant.replace(' ', '-')}-seed-{s}"
        ext.score(d, round_1p, build=lambda job: widen.build((*job, variant)), suffix=f"-seed-{s}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3], Path(sys.argv[4]), [int(a) for a in sys.argv[5:]])
