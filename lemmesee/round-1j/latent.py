"""Read the frozen title student's tree contributions before their sum.

This command prepares features only; fitting and judging stay in joint.py.
"""
from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import numpy as np

import joint


def contributions(reader, row):
    values = []
    for nodes, leaves in reader.trees:
        if not nodes:
            values.append(leaves[0])
            continue
        at = 0
        while at >= 0:
            feature, flags, threshold, left, right = nodes[at]
            value = float(row[feature])
            if math.isnan(value) and flags & 3 == 2:
                at = left if flags & 4 else right
            else:
                if math.isnan(value):
                    value = 0.0
                if flags & 3 == 1 and abs(value) <= 1e-35:
                    at = left if flags & 4 else right
                else:
                    at = left if value <= threshold else right
        values.append(leaves[-1 - at])
    return np.asarray(values, dtype=np.float32)


def prepare(out: Path):
    root = joint.ROOT
    meta = json.loads((out / "data.json").read_text(encoding="utf-8"))
    h = root / "corpus" / "round-1h"
    student = joint.module("joint_name_student", root / "round-1h" / "student.py")
    pack = joint.module("joint_name_pack", root / "round-1c" / "pack.py")
    import rank
    from aq import generate, load
    y = student.targets(h)
    panels = [p for p in student.load_panels(h) if p["id"] in y]
    names = sorted({k for p in panels[:2000] for k in p["features"]})
    reader = student.Reader(root / "corpus" / "round-1g", h)
    reader.rates, _ = student.rate_table(panels, y)
    del panels
    forest = pack.Packed((h / "student.bin").read_bytes())
    wanted = {}
    for i, p in enumerate(meta["panels"]):
        wanted.setdefault(p["db"], []).append((i, p))
    leaves = np.full((len(meta["panels"]), len(forest.trees)), np.nan, dtype=np.float32)
    parity = 0.0
    for fe, items in wanted.items():
        snap = json.loads((root / "corpus" / "see" / fe / "snapshot.json").read_text(encoding="utf-8"))
        schema = load(snap, False)
        qs = {}
        for q in generate(schema):
            qs.setdefault(q.title(schema), q)
        full = load(snap, True)
        for q in generate(full):
            qs.setdefault(q.title(full), q)
        inputs = []
        for _, p in items:
            q = qs.get(p["title"])
            if q is None:
                raise RuntimeError(f"No title student's question for {fe}: {p['title']}")
            inputs.append({"id": p["id"], "repo": fe, "score": 0, "rank": 99,
                           "slots": rank.slots(schema, q), "features": rank.features(schema, q)})
        # The actual prior score and rank are part of the student's input.
        arrays = np.load(out / "data.npz", allow_pickle=False)
        base_names = meta["numeric_names"]
        for data, (i, _) in zip(inputs, items):
            data["score"] = float(arrays["base"][i, base_names.index("score")])
            data["rank"] = float(arrays["base"][i, base_names.index("rank")])
        X = student.matrix(inputs, reader, names, student.SETS["all"])
        for (i, _), x in zip(items, X):
            leaves[i] = contributions(forest, x)
            parity = max(parity, abs(float(leaves[i].astype(float).sum()) - forest.score(x)))
    if not np.isfinite(leaves).all() or parity > 1e-6:
        raise RuntimeError(f"Frozen encoder did not reconstruct: {parity}")
    np.savez_compressed(out / "latent.npz", leaves=leaves)
    resources = [h / "student.bin", h / "panels.jsonl", h / "vectors" / "vectors.json",
                 root / "corpus" / "round-1g" / "lexicon" / "lexicon.json"]
    resources += sorted((h / "labels").glob("*.json"))
    joint.write_json(out / "latent.json", {
        "data_sha256": meta["data_sha256"], "latent_sha256": joint.sha(out / "latent.npz"),
        "encoder_sum_parity": parity, "trees": len(forest.trees),
        "resources": {str(p.relative_to(root)): joint.sha(p) for p in resources},
        "environment": joint.environment(),
        "scope": "the same compiled frozen encoder, scalar versus retained tree contributions",
    })
    print("prepared frozen title encoder features", flush=True)


if __name__ == "__main__":
    prepare(Path(sys.argv[1]) if len(sys.argv) > 1 else joint.OUT)
