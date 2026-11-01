"""Round 1p's size against quality: the chosen variant fitted again with
fewer trees in its first stage, each measured on SchemaPile-Perm's
validation split and on round 1p's databases.

    .venv/Scripts/python -I curve.py <round 1b out> <variants out> <variant> <round 1p out> [<trees>...]

Each size is fitted as widen.py fits a variant — five folds, the whole, the
second stage of 400 trees on the out-of-fold scores, the threshold from
validation — and saved under <variants out>/<variant>-<trees>/; its
external scores go to round 1p's out as score-curve-<trees>.json.
"""

import json
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import widen  # noqa: E402  (puts round 1b on the path)
import graph  # noqa: E402
import keys  # noqa: E402

TREES = [25, 50, 100, 200, 400]


def compact_kib(whole, second) -> dict:
    first_dump = whole.get_booster().get_dump()
    nodes = sum(len([line for line in t.splitlines() if line.strip()]) for t in first_dump)
    leaves = sum(t.count("leaf=") for t in first_dump)
    tree_info = second.booster_.dump_model()["tree_info"]
    second_leaves = sum(t["num_leaves"] for t in tree_info)
    return {"first": round(graph.compact_bytes(nodes - leaves, leaves) / 1024),
            "second": round(graph.compact_bytes(second_leaves - len(tree_info), second_leaves) / 1024)}


def main(data: Path, out: Path, variant: str, round_1p: Path, trees: list[int]) -> None:
    import external as ext  # noqa: E402
    schemas, guesses = keys.load(data)
    base = dict(graph.FIRST_GPU)
    results = {}
    with ProcessPoolExecutor(graph.WORKERS) as pool:
        for n in trees:
            graph.FIRST_GPU = {**base, "n_estimators": n}
            result, whole, second = widen.fit(pool, schemas, guesses, variant)
            result["first trees"] = n
            result["compact KiB"] = compact_kib(whole, second)
            d = out / f"{variant.replace(' ', '-')}-{n}"
            d.mkdir(parents=True, exist_ok=True)
            whole.save_model(str(d / "keys-first.ubj"))
            second.booster_.save_model(str(d / "keys-second.txt"))
            (d / "graph-final.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
            results[n] = result
            print(json.dumps(result), flush=True)
    graph.FIRST_GPU = base
    for n in trees:
        d = out / f"{variant.replace(' ', '-')}-{n}"
        ext.score(d, round_1p, build=lambda job: widen.build((*job, variant)), suffix=f"-curve-{n}")
    (out / f"curve-{variant.replace(' ', '-')}.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3], Path(sys.argv[4]),
         [int(a) for a in sys.argv[5:]] or TREES)
