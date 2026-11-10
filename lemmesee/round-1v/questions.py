"""Round 1v: see's questions through this round's keys, round 1u's measure unchanged.

    python -I questions.py <model> [<model> ...]

Round 1u asks see's questions of a database three ways — with its declared
keys, with round 1's rules, and with a model's keys in place of the
declarations — and counts the questions that join through declared keys
still asked. Here its code runs as it is (round-1u/questions.py), on Spider,
BIRD, CTU and LiveSQLBench, with one change: a model's keys are not drawn
again but read from the scores already kept, a frozen model's from dump.py
and one of this round's from second.py, each at its own threshold, one
target a column. `nine-mended` is read the same way, so its figures here
against round 1t's report say whether the kept scores draw the same keys.

Writes corpus/round-1v/questions-<models>.json.
"""

import json
import pickle
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "round-1u"))

import questions as round_1u  # noqa: E402  (round 1u's; puts the earlier rounds on the path)

import numpy as np  # noqa: E402

import whole  # noqa: E402

OUT = whole.OUT
FROZEN = ("1s", "seven-bounded", "nine-mended")
NAMES = {"spider": "spider", "bird": "bird", "ctu": "ctu", "livesqlbench (development)": "livesqlbench (development)"}
_CACHE = {}


def keys_of(model: str, name: str) -> dict:
    """{schema id: [(table, column, target)]} a model draws on a set, from its kept scores."""
    if model in FROZEN:
        sc = whole.Scored(model, name)
        preds = sc.baseline()
        return {s["id"]: [(t, c, g) for (t, c), g in p.items()] for s, p in zip(sc.schemas, preds)}
    schemas = pickle.load(open(OUT / "sets" / f"{whole.slug(name)}.pkl", "rb"))["schemas"]
    which = ""
    if model.endswith("@declared"):
        model, which = model[:-len("@declared")], "declared"
    rec = pickle.load(open(OUT / "first" / model.split("--")[0] / f"{whole.slug(name)}.pkl", "rb"))["pairs"]
    d = OUT / "second" / model
    raw = np.load(d / "sets" / f"{whole.slug(name)}.npy").astype(np.float64)
    result = json.load(open(d / "second.json", encoding="utf-8"))
    threshold = result["validation, declarations only"]["threshold"] if which == "declared" else result["threshold"]
    kept = whole.one_target(raw, rec)
    out = {}
    for i in np.flatnonzero(kept >= threshold):
        s = schemas[rec["schema"][i]]
        t = s["tables"][rec["table"][i]]
        out.setdefault(s["id"], []).append((t["name"], t["columns"][rec["column"][i]]["name"],
                                            s["tables"][rec["target"][i]]["name"]))
    return out


def drawn_keys(pool, schemas, split, guesses, model):
    # round 1u calls this a set at a time; the set is told by its first schema
    source = schemas[0]["source"] if schemas else ""
    name = {"spider": "spider", "bird": "bird", "ctu": "ctu"}.get(source, "livesqlbench (development)")
    if (model, name) not in _CACHE:
        _CACHE[(model, name)] = keys_of(model, name)
    return _CACHE[(model, name)]


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    models = tuple(sys.argv[1:])
    round_1u.drawn_keys = drawn_keys
    round_1u.OUT = OUT
    round_1u.main(models if len(models) > 1 else models + ("nine-mended",), round_1u.SETS,
                  "questions-" + "+".join(m.replace("@", "-at-") for m in models) + ".json")
