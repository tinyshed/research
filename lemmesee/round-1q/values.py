"""Round 1q, section 6: value keys — a column whose sampled values are found
exactly among a unique text column's, drawn as a key whatever its name.

    .venv/Scripts/python -I values.py tune <round 1p out> <cascade out> <stats> <sources>
    .venv/Scripts/python -I values.py measure <round 1p out> <cascade out> <stats> <sources> <q> <h> <out>

`tune` prints F1 over every declared key for a grid of q and h on the named
sources (comma-separated, e.g. spider,bird); `measure` writes values.json for
one q and h. Keys are the cascade cut's sure keys, the model's at its
threshold, with a value key added for a column that has none.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

ROUND_1P = Path(__file__).resolve().parent.parent / "round-1p"
sys.path.insert(0, str(ROUND_1P))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import gemini  # noqa: E402

from whose import sample  # noqa: E402


def distinct(col: dict, rows: int) -> float:
    nd = col["n_distinct"]
    return -nd * rows if nd < 0 else nd


def value_keys(stats: dict, schema: dict, q_min: float, h_min: float) -> set:
    """(table, column, target) for every value key of one database."""
    pk = {t["name"]: t["pk"] for t in schema["tables"]}
    anchors = []
    for t, tab in stats.items():
        for c, col in tab["columns"].items():
            vs = sample(col)
            if not vs or not all(isinstance(v, str) for v in vs):
                continue
            unique = col["n_distinct"] == -1 or pk.get(t) == [c]
            if not unique or tab["rows"] < 2:
                continue
            q = min(len(set(vs)) / max(distinct(col, tab["rows"]), 1), 1.0)
            if q >= q_min:
                anchors.append((t, c, set(vs), tab["rows"]))
    best = {}
    for t, tab in stats.items():
        for c, col in tab["columns"].items():
            if col["n_distinct"] == -1 or pk.get(t) == [c]:
                continue
            vs = set(sample(col))
            if not vs or not all(isinstance(v, str) for v in vs):
                continue
            for at, ac, avs, arows in anchors:
                if at == t:
                    continue
                h = len(vs & avs) / len(vs)
                if h >= h_min and distinct(col, tab["rows"]) <= arows:
                    if (t, c) not in best or h > best[(t, c)][1]:
                        best[(t, c)] = (at, h)
    return {(t, c, at) for (t, c), (at, _) in best.items()}


def load(data: Path, cascade: Path, stats_dir: Path, sources: set):
    schemas = {s["id"]: s for s in map(json.loads, open(data / "hidden.jsonl", encoding="utf-8"))}
    stats = {}
    for f in stats_dir.glob("*/*.json"):
        sid = f"{f.parent.name}/{f.stem}"
        if f.parent.name in sources and sid in schemas and schemas[sid]["fk"]:
            stats[sid] = json.load(open(f, encoding="utf-8"))
    sure = defaultdict(set)
    for line in open(cascade / "sure.jsonl", encoding="utf-8"):
        sid, t, c, r, _ = json.loads(line)
        if sid in stats:
            sure[sid].add((t, c, r))
    truth = {i: {tuple(k) for k in schemas[i]["fk"]} for i in stats}
    return schemas, stats, sure, truth


def combine(stats, schemas, sure, q, h):
    out, added = {}, 0
    for sid in stats:
        keys = set(sure[sid])
        taken = {(t, c) for t, c, _ in keys}
        for t, c, r in value_keys(stats[sid], schemas[sid], q, h):
            if (t, c) not in taken:
                keys.add((t, c, r))
                added += 1
        out[sid] = keys
    return out, added


def tune(data, cascade, stats_dir, sources):
    schemas, stats, sure, truth = load(data, cascade, stats_dir, sources)
    ids = list(stats)
    p, r, f = gemini.prf(*gemini.open_counts(truth, sure, ids))
    print(f"{len(ids)} databases; model alone P {p:.3f} R {r:.3f} F1 {f:.3f}")
    grid = {"databases": len(ids), "model alone": {"precision": p, "recall": r, "f1": f}, "grid": []}
    for q in (0.3, 0.5, 0.8):
        for h in (0.7, 0.8, 0.9, 1.0):
            keys, added = combine(stats, schemas, sure, q, h)
            p, r, f = gemini.prf(*gemini.open_counts(truth, keys, ids))
            grid["grid"].append({"q": q, "h": h, "added": added, "precision": p, "recall": r, "f1": f})
            print(f"q {q} h {h}: added {added:4} P {p:.3f} R {r:.3f} F1 {f:.3f}")
    (stats_dir.parent / f"values-tune-{'-'.join(sorted(sources))}.json").write_text(
        json.dumps(grid, indent=1) + "\n", encoding="utf-8")


def measure(data, cascade, stats_dir, sources, q, h, out):
    schemas, stats, sure, truth = load(data, cascade, stats_dir, sources)
    keys, added = combine(stats, schemas, sure, q, h)
    result = {"q": q, "h": h, "databases": len(stats), "value keys added": added}
    for name in sorted(sources) + ["all"]:
        ids = [i for i in stats if name in ("all", schemas[i]["source"])]
        for how, preds in (("model alone", sure), ("model and value keys", keys)):
            result[f"{name}, {how}"] = dict(zip(("precision", "recall", "f1"),
                                               gemini.prf(*gemini.open_counts(truth, preds, ids))))
    (out / f"values-{'-'.join(sorted(sources))}.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    a = sys.argv
    if a[1] == "tune":
        tune(Path(a[2]), Path(a[3]), Path(a[4]), set(a[5].split(",")))
    else:
        measure(Path(a[2]), Path(a[3]), Path(a[4]), set(a[5].split(",")), float(a[6]), float(a[7]), Path(a[8]))
