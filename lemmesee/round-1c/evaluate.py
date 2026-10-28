"""Round 1c's measure, run once at the end: the ranker fitted on the first
wave of teacher schemas and measured on the second, never looked at, beside
round 1c's hand prior and round 1's rules on the same schemas, each schema's
difference bootstrapped.

    python -I evaluate.py <learn dir> <teacher dir> <rules binary> <first key of the second wave>
"""

from __future__ import annotations

import json
import pickle
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import baseline  # noqa: E402
import gold  # noqa: E402
import learn  # noqa: E402
import rank  # noqa: E402
from aq import load  # noqa: E402

SEED = 20261028


def per_schema(r, chosen_ids) -> dict:
    out = {}
    for t, d in r["teachers"].items():
        ten = {g for g in d["asked"] if g is not None}
        out[f"{t} precision"] = len(chosen_ids & ten) / 5
        out[f"{t} recall"] = sum(1 for g in d["asked"][:5] if g in chosen_ids) / 5
    return out


def round1_fives(binary: Path, tdir: Path, keys: set[str], work: Path) -> dict[str, set]:
    work.mkdir(parents=True, exist_ok=True)
    fives = {}
    for line in (tdir / "schemas.jsonl").open(encoding="utf-8"):
        snap = json.loads(line)
        if snap["key"] not in keys:
            continue
        path = work / f"{snap['key']}.json"
        path.write_text(json.dumps(snap), encoding="utf-8")
        d = work / snap["key"]
        subprocess.run([str(binary), "--snapshot", str(path), "--out", str(d)], check=True, capture_output=True)
        cands = {c["id"]: c for c in json.loads((d / "candidates.json").read_text(encoding="utf-8"))["candidates"]}
        panels = [p["id"] for p in json.loads((d / "panels-A.json").read_text(encoding="utf-8"))["panels"]]
        schema = load(snap, reads_n=False)
        fives[snap["key"]] = {baseline.ident(schema, cands[i]) for i in panels} - {None}
    return fives


def bootstrap(diff: np.ndarray, n: int = 10000) -> tuple[float, float]:
    rng = np.random.default_rng(SEED)
    means = [rng.choice(diff, size=len(diff), replace=True).mean() for _ in range(n)]
    return float(np.percentile(means, 2.5)), float(np.percentile(means, 97.5))


def main(out: Path, tdir: Path, binary: Path, first: str) -> None:
    rows = pickle.load((out / "data.pkl").open("rb"))
    names = learn.all_names(rows)
    train = [r for r in rows if r["key"] < first]
    test = [r for r in rows if r["key"] >= first]
    model = learn.fit_one(train, names, 600)
    r1 = round1_fives(binary, tdir, {r["key"] for r in test}, out / "round1")
    scores = defaultdict(lambda: defaultdict(list))
    for r in test:
        X, _, _ = learn.matrix_free(r, names, None)
        learned = model.predict(X)
        prior = [rank.prior(f) for f in r["features"]]
        systems = {
            "round 1c, learned": {r["ids"][i] for i in learn.five(r, learned)},
            "round 1c, hand prior": {r["ids"][i] for i in learn.five(r, prior)},
            "round 1, rules A": r1.get(r["key"], set()),
        }
        for name, chosen in systems.items():
            for k, v in per_schema(r, chosen).items():
                scores[name][k].append(v)
    result = {"train schemas": len(train), "test schemas": len(test), "systems": {}, "differences": {}}
    for name, d in scores.items():
        result["systems"][name] = {k: round(float(np.mean(v)), 3) for k, v in d.items()}
    for metric in scores["round 1c, learned"]:
        a = np.array(scores["round 1c, learned"][metric])
        b = np.array(scores["round 1, rules A"][metric])
        lo, hi = bootstrap(a - b)
        result["differences"][f"learned − round 1, {metric}"] = {"mean": round(float((a - b).mean()), 3), "95%": [round(lo, 3), round(hi, 3)]}
    (out / "evaluate.json").write_text(json.dumps(result, indent=1, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(result, indent=1, ensure_ascii=False))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4])
