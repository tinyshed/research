"""Round 1p: the key model packed as round 1b sized it — an inner node a
feature (u16), a threshold (f32), two children (u16 each) and a byte of
flags; a leaf its value (f32) — and read back, so its size is a file and
not an estimate, and its scores are checked against the libraries'.

    .venv/Scripts/python -I pack.py <models> <round 1p out> [<variant>]

writes <models>/keys.bin and <models>/pack.json: the file's bytes by part,
and on every pair of round 1p's databases the largest difference between the
packed model's scores and XGBoost's and LightGBM's, and how many keys drawn
at the threshold differ.

The file: `KEYS`, a version byte, then each stage — u16 trees, f32 base
margin — and each tree: u16 inner nodes, u16 leaves, the inner nodes, the
leaves. A child with its top bit set is a leaf's index. Flags: bit 0 a
missing value goes left; bits 1–2 what counts as missing — 0 an absent
feature (XGBoost on sparse rows), 1 none, 2 zero, 3 NaN (LightGBM).
"""

import json
import math
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import numpy as np  # noqa: E402

import external  # noqa: E402

LEAF = 0x8000
ABSENT, NONE, ZERO, NAN = 0, 1, 2, 3


def xgb_trees(model):
    raw = json.loads(model.get_booster().save_raw(raw_format="json"))
    p = float(raw["learner"]["learner_model_param"]["base_score"].strip("[]"))
    trees = []
    for t in raw["learner"]["gradient_booster"]["model"]["trees"]:
        left, right = t["left_children"], t["right_children"]
        inner = [i for i in range(len(left)) if left[i] != -1]
        leaves = [i for i in range(len(left)) if left[i] == -1]
        at = {n: k for k, n in enumerate(inner)} | {n: LEAF | k for k, n in enumerate(leaves)}
        nodes = [(t["split_indices"][n], t["split_conditions"][n], at[left[n]], at[right[n]],
                  int(t["default_left"][n]) | ABSENT << 1) for n in inner]
        trees.append((nodes, [t["split_conditions"][n] for n in leaves]))
    return math.log(p / (1 - p)), trees


def lgb_trees(booster):
    trees = []
    for info in booster.dump_model()["tree_info"]:
        nodes, leaves = [], []

        def walk(n):
            if "leaf_value" in n:
                leaves.append(n["leaf_value"])
                return LEAF | (len(leaves) - 1)
            k = len(nodes)
            nodes.append(None)
            assert n["decision_type"] == "<="
            left, right = walk(n["left_child"]), walk(n["right_child"])
            missing = {"None": NONE, "Zero": ZERO, "NaN": NAN}[n["missing_type"]]
            # The features are f32, so x <= t in double holds exactly when x
            # is at most the largest f32 not above t
            t = np.float32(n["threshold"])
            if float(t) > n["threshold"]:
                t = np.nextafter(t, np.float32(-np.inf))
            nodes[k] = (n["split_feature"], float(t), left, right, int(n["default_left"]) | missing << 1)
            return k

        walk(info["tree_structure"])
        trees.append((nodes, leaves))
    return 0.0, trees


def write(stages) -> bytes:
    out = bytearray(b"KEYS\x01")
    for base, trees in stages:
        out += struct.pack("<Hf", len(trees), base)
        for nodes, leaves in trees:
            out += struct.pack("<HH", len(nodes), len(leaves))
            for f, t, a, b, flags in nodes:
                out += struct.pack("<HfHHB", f, t, a, b, flags)
            out += struct.pack(f"<{len(leaves)}f", *leaves)
    return bytes(out)


def read(data: bytes):
    """The stages back as numpy arrays a tree, as a reader in Rust would hold them."""
    assert data[:5] == b"KEYS\x01"
    at, stages = 5, []
    for _ in range(2):
        n, base = struct.unpack_from("<Hf", data, at)
        at += 6
        trees = []
        for _ in range(n):
            ni, nl = struct.unpack_from("<HH", data, at)
            at += 4
            rec = np.frombuffer(data, dtype=np.dtype([("f", "<u2"), ("t", "<f4"), ("a", "<u2"), ("b", "<u2"),
                                                      ("flags", "u1")]), count=ni, offset=at)
            at += 11 * ni
            leaves = np.frombuffer(data, dtype="<f4", count=nl, offset=at)
            at += 4 * nl
            trees.append((rec, leaves))
        stages.append((base, trees))
    assert at == len(data)
    return stages


def score(stage, values: np.ndarray, present: np.ndarray) -> np.ndarray:
    """Margins of many rows at once: every row walks each tree a level at a time."""
    base, trees = stage
    rows = np.arange(len(values))
    out = np.full(len(values), base, dtype=np.float64)
    for rec, leaves in trees:
        if len(rec) == 0:
            out += leaves[0]
            continue
        cur = np.zeros(len(values), dtype=np.int64)
        done = np.zeros(len(values), dtype=bool)
        leaf = np.zeros(len(values), dtype=np.int64)
        while not done.all():
            r = rows[~done]
            node = rec[cur[r]]
            v = values[r, node["f"]].astype(np.float32)
            have = present[r, node["f"]]
            kind = node["flags"] >> 1
            missing = np.where(kind == ABSENT, ~have, np.where(kind == ZERO, np.abs(v) <= 1e-35,
                                                              np.where(kind == NAN, np.isnan(v), False)))
            # XGBoost goes left below its threshold, LightGBM at or below it
            go_left = np.where(kind == ABSENT, v < node["t"], v <= node["t"])
            go_left = np.where(missing, (node["flags"] & 1).astype(bool), go_left)
            nxt = np.where(go_left, node["a"], node["b"]).astype(np.int64)
            is_leaf = (nxt & LEAF) != 0
            leaf[r[is_leaf]] = nxt[is_leaf] & ~LEAF
            done[r[is_leaf]] = True
            cur[r[~is_leaf]] = nxt[~is_leaf]
        out += leaves[leaf]
    return out


def main(models: Path, out: Path, variant: str | None) -> None:
    run, graph = external.scorer(models)
    import lightgbm as lgb  # noqa: E402
    import xgboost as xgb  # noqa: E402
    first = xgb.XGBClassifier()
    first.load_model(str(models / "keys-first.ubj"))
    first.set_params(device="cpu")
    second = lgb.Booster(model_file=str(models / "keys-second.txt"))
    data = write([xgb_trees(first), lgb_trees(second)])
    (models / "keys.bin").write_bytes(data)
    stages = read(data)
    sizes = {"file": len(data)}
    for name, (_, trees) in zip(("first", "second"), stages):
        sizes[name] = {"trees": len(trees), "inner nodes": sum(len(r) for r, _ in trees),
                       "leaves": sum(len(lv) for _, lv in trees)}

    build = None
    if variant:
        import widen  # noqa: E402
        build = lambda job: widen.build((*job, variant))  # noqa: E731
    schemas = [json.loads(line) for line in open(out / "hidden.jsonl", encoding="utf-8")]
    x, meta = (build or graph.build)((schemas, "external", {}))
    lib1 = first.predict(x, output_margin=True)
    ours1 = np.zeros(x.shape[0])
    for start in range(0, x.shape[0], 2000):
        chunk = x[start:start + 2000]
        dense = chunk.toarray()
        present = np.zeros(dense.shape, dtype=bool)
        present[np.repeat(np.arange(chunk.shape[0]), np.diff(chunk.indptr)), chunk.indices] = True
        ours1[start:start + 2000] = score(stages[0], dense, present)
    g_lib = graph.graph_features(meta, 1 / (1 + np.exp(-lib1)))
    g_ours = graph.graph_features(meta, 1 / (1 + np.exp(-ours1)))
    lib2 = second.predict(g_lib, raw_score=True)
    ours2 = score(stages[1], g_ours, np.ones(g_ours.shape, dtype=bool))
    # The second stage alone, on the very features LightGBM read
    alone = score(stages[1], g_lib, np.ones(g_lib.shape, dtype=bool))
    threshold = json.load(open(models / "graph-final.json", encoding="utf-8"))["threshold"]
    keys_of = [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta]
    import keys  # noqa: E402
    drawn_lib = keys.one_target(1 / (1 + np.exp(-lib2)), keys_of) >= threshold
    drawn_ours = keys.one_target(1 / (1 + np.exp(-ours2)), keys_of) >= threshold
    result = {
        "bytes": sizes, "KiB": round(len(data) / 1024, 1), "pairs": int(x.shape[0]),
        "first stage, largest margin difference": float(np.abs(lib1 - ours1).max()),
        "second stage, largest margin difference": float(np.abs(lib2 - ours2).max()),
        "second stage on the same features, largest margin difference": float(np.abs(lib2 - alone).max()),
        "pairs whose second-stage margin differs by over 1e-4": int((np.abs(lib2 - ours2) > 1e-4).sum()),
        "keys drawn": int(drawn_lib.sum()), "keys drawn differently": int((drawn_lib != drawn_ours).sum()),
    }
    (models / "pack.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3] if len(sys.argv) > 3 else None)
