"""Round 1s: the refit packed as round 1p packed `both`, and the Rust reader
against the libraries on the same schemas.

    python -I parity.py pack <models>
    python -I parity.py check <models> <round 1b out> <external out> <reader> <out>

`pack` writes <models>/keys.bin with round 1p's pack.py and <models>/pack.json:
the file's parts and its SHA-256. `check` runs `reader parity` on round 1p's
external databases and on SchemaPile-Perm's test split, builds the same pairs
in Python with the trigrams sorted, and writes <out>/parity.json: pairs whose
names or hashed features differ, the largest margin differences of each stage
against XGBoost and LightGBM, and the keys one draws and the other does not,
each shown.
"""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import letters  # noqa: E402  (first: it sorts the trigrams in this process)

import numpy as np  # noqa: E402

import graph  # noqa: E402
import keys  # noqa: E402
import pack as packer  # noqa: E402  (round 1p's)
import widen  # noqa: E402

PRIME, OFFSET = np.uint64(0x100000001B3), np.uint64(0xCBF29CE484222325)


def pack(models: Path) -> None:
    first, second = widen.load_models(models)
    data = packer.write([packer.xgb_trees(first), packer.lgb_trees(second.booster_ if hasattr(second, "booster_")
                                                                    else second)])
    (models / "keys.bin").write_bytes(data)
    stages = packer.read(data)
    sizes = {"file bytes": len(data), "KiB": round(len(data) / 1024, 1),
             "sha256": hashlib.sha256(data).hexdigest()}
    for name, (_, trees) in zip(("first", "second"), stages):
        sizes[name] = {"trees": len(trees), "inner nodes": sum(len(r) for r, _ in trees),
                       "leaves": sum(len(lv) for _, lv in trees)}
    (models / "pack.json").write_text(json.dumps(sizes, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(sizes, indent=1))


def fnv_rows(x) -> np.ndarray:
    """FNV-1a 64 of each CSR row's (u16 index, f32 value) little-endian, as
    the reader's `digest`, all rows at once."""
    x = x.tocsr()
    x.sort_indices()
    rec = np.empty(x.nnz, dtype=[("i", "<u2"), ("v", "<f4")])
    rec["i"], rec["v"] = x.indices, x.data.astype(np.float32)
    raw = rec.view(np.uint8)
    start, length = x.indptr[:-1].astype(np.int64) * 6, np.diff(x.indptr).astype(np.int64) * 6
    h = np.full(x.shape[0], OFFSET, dtype=np.uint64)
    for k in range(int(length.max(initial=0))):
        on = np.flatnonzero(length > k)
        h[on] = (h[on] ^ raw[start[on] + k].astype(np.uint64)) * PRIME
    return h


def fnv_names(parts: list[str]) -> int:
    h = 0xCBF29CE484222325
    for p in parts:
        for b in p.encode("utf-8") + b"\0":
            h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def ordered_build(schemas, split):
    """widen.build over contiguous chunks in file order, as the reader reads."""
    chosen = [s for s in schemas if s["split"] == split and s["fk"]]
    n = graph.WORKERS * 4
    size = -(-len(chosen) // n)
    jobs = [(chosen[i:i + size], split, {}, "both") for i in range(0, len(chosen), size)]
    import scipy.sparse as sp
    xs, meta = [], []
    with letters.pool(graph.WORKERS) as pool:
        for x, m in pool.map(widen.build, jobs):
            xs.append(x)
            meta.extend(m)
    return sp.vstack(xs).tocsr(), meta


def one_set(name, schemas, split, models, reader, threshold, out):
    rec = np.dtype([("names", "<u8"), ("features", "<u8"), ("first", "<f4"), ("second", "<f8")])
    stem = out / f"rust-{name}"
    subprocess.run([str(reader), "parity", str(models / "keys.bin"), repr(threshold), str(out / f"{name}.jsonl"),
                    split, str(stem)], check=True)
    rust = np.fromfile(f"{stem}.bin", dtype=rec)
    first, second = widen.load_models(models)
    x, meta = ordered_build(schemas, split)
    m1 = first.predict(x, output_margin=True).astype(np.float32)
    p1 = first.predict_proba(x)[:, 1]
    g = graph.graph_features(meta, p1)
    m2 = second.predict(g, raw_score=True)
    p2 = second.predict(g)
    kept = keys.one_target(p2, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
    drawn_py = {(meta[i][graph.SID], meta[i][graph.TABLE], meta[i][graph.COLUMN], meta[i][graph.TARGET])
                for i in np.flatnonzero(kept >= threshold)}
    drawn_rust = set()
    for line in open(f"{stem}.keys.jsonl", encoding="utf-8"):
        sid, t, c, r, _ = json.loads(line)
        drawn_rust.add((sid, t, c, r))
    result = {"schemas": len({m[graph.SID] for m in meta}), "pairs, Python": len(meta), "pairs, Rust": len(rust)}
    if len(rust) != len(meta):
        result["aligned"] = False
        return result
    names = np.array([fnv_names([m[graph.SID], m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]]) for m in meta],
                     dtype=np.uint64)
    feats = fnv_rows(x)
    bad_names = np.flatnonzero(names != rust["names"])
    bad_feats = np.flatnonzero(feats != rust["features"])
    d1, d2 = np.abs(m1 - rust["first"]), np.abs(m2 - rust["second"])
    result.update({
        "pairs whose names differ": int(bad_names.size),
        "pairs whose hashed features differ": int(bad_feats.size),
        "first stage, largest margin difference": float(d1.max()),
        "first stage, pairs differing by over 1e-4": int((d1 > 1e-4).sum()),
        "second stage, largest margin difference": float(d2.max()),
        "second stage, pairs differing by over 1e-4": int((d2 > 1e-4).sum()),
        "keys drawn, Python": len(drawn_py), "keys drawn, Rust": len(drawn_rust),
        "drawn by Python only": sorted(drawn_py - drawn_rust)[:20],
        "drawn by Rust only": sorted(drawn_rust - drawn_py)[:20],
    })
    # The first few pairs whose features differ, named feature by feature
    shown = []
    for i in bad_feats[:5]:
        m = meta[i]
        py = None
        sch = next(s for s in schemas if s["id"] == m[graph.SID])
        for s, t, c, tg, _, _, f, _ in widen.pairs([sch], split, {}, True, True):
            if (t["name"], c["name"], tg["name"]) == (m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]):
                py = f
                break
        dumped = subprocess.run([str(reader), "dump", str(out / f"{name}.jsonl"), m[graph.SID]], check=True,
                                capture_output=True, text=True, encoding="utf-8").stdout.splitlines()
        rs = next((json.loads(line)[3] for line in dumped if json.loads(line)[:3] ==
                   [m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]]), {})
        keys_ = sorted(set(py or {}) | set(rs))
        shown.append({"pair": list(m[:1]) + [m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]],
                      "differs": {k: [(py or {}).get(k), rs.get(k)] for k in keys_ if (py or {}).get(k) != rs.get(k)}})
    result["pairs that differ, shown"] = shown
    return result


def check(models: Path, data: Path, ext: Path, reader: Path, out: Path) -> None:
    assert letters.is_sorted()
    out.mkdir(parents=True, exist_ok=True)
    threshold = json.load(open(models / "graph-final.json", encoding="utf-8"))["threshold"]
    result = {"keys.bin sha256": hashlib.sha256((models / "keys.bin").read_bytes()).hexdigest(),
              "threshold": threshold}
    sets = {"external": [json.loads(line) for line in open(ext / "hidden.jsonl", encoding="utf-8")],
            "test": [json.loads(line) for line in open(data / "hidden.jsonl", encoding="utf-8")]}
    for name, schemas in sets.items():
        split = "external" if name == "external" else "test"
        with open(out / f"{name}.jsonl", "w", encoding="utf-8") as f:
            for s in schemas:
                if s["split"] == split and s["fk"]:
                    f.write(json.dumps(s, ensure_ascii=False) + "\n")
        chosen = [s for s in schemas if s["split"] == split and s["fk"]]
        result[name] = one_set(name, chosen, split, models, reader, threshold, out)
        print(name, json.dumps(result[name], default=str)[:2000], flush=True)
    (out / "parity.json").write_text(json.dumps(result, indent=2, default=str) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "pack":
        pack(Path(sys.argv[2]))
    else:
        check(Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]), Path(sys.argv[5]).resolve(), Path(sys.argv[6]))
