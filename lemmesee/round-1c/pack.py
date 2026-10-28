"""The ranker as a binary would carry it: its trees packed into bytes, and a
reader of those bytes that scores without LightGBM, checked against
LightGBM's own scores.

    python -I pack.py <learn dir>

writes <learn dir>/ranker.bin and prints its size and the largest
difference between the two readers' scores over the training questions.
"""

from __future__ import annotations

import math
import pickle
import struct
import sys
from pathlib import Path

MAGIC = b"LMR1"
MISSING = {"None": 0, "Zero": 1, "NaN": 2}


def pack(model) -> bytes:
    d = model.dump_model()
    names = d["feature_names"]
    out = bytearray(MAGIC)
    out += struct.pack("<HH", len(names), len(d["tree_info"]))
    blob = "\n".join(names).encode()
    out += struct.pack("<I", len(blob)) + blob
    for tree in d["tree_info"]:
        nodes, leaves = [], []

        def walk(n) -> int:
            """A node's index, or a leaf's as -1 - index."""
            if "leaf_value" in n:
                leaves.append(n["leaf_value"])
                return -len(leaves)
            i = len(nodes)
            nodes.append(None)
            left, right = walk(n["left_child"]), walk(n["right_child"])
            assert n["decision_type"] == "<=", n["decision_type"]
            flags = MISSING[n["missing_type"]] | (4 if n["default_left"] else 0)
            # LightGBM writes a threshold of 1e300 for a split every value
            # passes; float32's largest keeps it so for every finite feature
            threshold = max(-3.4e38, min(3.4e38, n["threshold"]))
            nodes[i] = struct.pack("<BBfhh", n["split_feature"], flags, threshold, left, right)
            return i

        walk(tree["tree_structure"])
        out += struct.pack("<HH", len(nodes), len(leaves))
        for n in nodes:
            out += n
        out += struct.pack(f"<{len(leaves)}f", *leaves)
    return bytes(out)


class Packed:
    def __init__(self, data: bytes):
        assert data[:4] == MAGIC
        nf, nt = struct.unpack_from("<HH", data, 4)
        (bl,) = struct.unpack_from("<I", data, 8)
        self.names = data[12:12 + bl].decode().split("\n")
        p = 12 + bl
        self.trees = []
        for _ in range(nt):
            nn, nl = struct.unpack_from("<HH", data, p)
            p += 4
            nodes = [struct.unpack_from("<BBfhh", data, p + 10 * k) for k in range(nn)]
            p += 10 * nn
            leaves = struct.unpack_from(f"<{nl}f", data, p)
            p += 4 * nl
            self.trees.append((nodes, leaves))

    def score(self, x: list[float]) -> float:
        total = 0.0
        for nodes, leaves in self.trees:
            i = 0
            if not nodes:
                total += leaves[0]
                continue
            while True:
                f, flags, thr, left, right = nodes[i]
                v = x[f]
                missing = flags & 3
                if math.isnan(v):
                    if missing == 2:
                        i = left if flags & 4 else right
                    else:
                        v = 0.0
                if not math.isnan(v):
                    if missing == 1 and abs(v) <= 1e-35:
                        i = left if flags & 4 else right
                    else:
                        i = left if v <= thr else right
                if i < 0:
                    total += leaves[-1 - i]
                    break
        return total


if __name__ == "__main__":
    import lightgbm as lgb
    import numpy as np
    sys.path.insert(0, str(Path(__file__).parent))
    import learn
    out = Path(sys.argv[1])
    model = lgb.Booster(model_file=str(out / "ranker.txt"))
    data = pack(model)
    (out / "ranker.bin").write_bytes(data)
    reader = Packed(data)
    rows = pickle.load((out / "data.pkl").open("rb"))[:20]
    names = model.feature_name()
    table = None
    if any(n.startswith("w_") for n in names):
        import json
        table = json.loads((out / "words.json").read_text(encoding="utf-8"))
    worst, n = 0.0, 0
    for r in rows:
        X, _, _ = learn.matrix_free(r, names, table)
        ours = np.array([reader.score([float(v) for v in x]) for x in X])
        theirs = model.predict(X)
        worst = max(worst, float(np.max(np.abs(ours - theirs))))
        n += len(X)
    print(f"ranker.bin: {len(data)} bytes, {len(reader.trees)} trees; largest difference over {n} questions: {worst:.2e}")
