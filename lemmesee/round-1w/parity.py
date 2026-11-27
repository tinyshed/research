"""Round 1w: a fit packed for this round's Rust reader, and the reader
against the libraries on the same schemas; round 1t's parity.py with round
1v's version 10 and 11 pairs (ten.py) and its wide second stage (wide.py)
on the Python side, and a part of a set where the whole is not wanted.

    python -I parity.py pack <models>
    python -I parity.py check <models> <round 1b out> <external out> <reader> <out>
        [--external <n | source>] [--test <n>] [--workers <n>]

`pack` writes <models>/keys.bin with round 1p's pack.py and <models>/pack.json:
the file's parts and its SHA-256. The fifth byte is the version: 2 where the
fit reads version 5's cues on round 1p's pairs, 3 where it reads round 1t's
version 6 pairs, and then the sixth byte is its flags:

      1  version 7's cues          16  version 10's cues
      2  version 8's cues          32  version 11's cues
      4  stack.py's six features   64  wide.py's 43 features, after stack.py's
      8  version 9's cues

`nine-mended` is 9; `eleven--wide` is 1 + 4 + 8 + 16 + 32 + 64 = 125.

`check` runs `reader parity` on round 1p's external databases and on
SchemaPile-Perm's test split, builds the same pairs in Python with the
trigrams sorted, and writes <out>/parity.json: pairs whose names or hashed
features differ, the largest margin differences of each stage against
XGBoost and LightGBM, the second stage's features that differ by name, and
the keys one draws and the other does not, each shown. `--external spider`
reads one source of the external databases, `--external 100` their first
hundred; `--test 200` the first two hundred schemas of the test split that
declare a key. `--workers` is the pool that builds Python's pairs, 2 unless
said.
"""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1v"))

import ten  # noqa: E402  (first: round 1t and the earlier rounds on the path, the trigrams sorted)

import cues  # noqa: E402  (round 1t's)
import letters  # noqa: E402

import numpy as np  # noqa: E402

import graph  # noqa: E402
import keys  # noqa: E402
import pack as packer  # noqa: E402  (round 1p's)
import widen  # noqa: E402

PRIME, OFFSET = np.uint64(0x100000001B3), np.uint64(0xCBF29CE484222325)
LATER = {"ten": False, "eleven": True}  # round 1v's variants: whether version 11's people are read

VARIANT = "both"


def variant_of(models: Path) -> str:
    fit = json.load(open(models / "graph-final.json", encoding="utf-8"))
    return fit["variant"] if fit.get("variant") in cues.VARIANTS or fit.get("variant") in LATER else "both"


def flags_of(variant: str, graph_kind) -> int:
    later = variant in LATER
    return ((1 if variant in ("seven", "eight", "nine") or later else 0) | (2 if variant == "eight" else 0)
            | (4 if graph_kind in ("relative", "wide") else 0) | (8 if variant == "nine" or later else 0)
            | (16 if later else 0) | (32 if LATER.get(variant) else 0) | (64 if graph_kind == "wide" else 0))


def pack(models: Path) -> None:
    first, second = widen.load_models(models)
    data = packer.write([packer.xgb_trees(first), packer.lgb_trees(second.booster_ if hasattr(second, "booster_")
                                                                    else second)])
    stages = packer.read(data)  # round 1p's reader knows version 1 only
    variant = variant_of(models)
    fit = json.load(open(models / "graph-final.json", encoding="utf-8"))
    sizes = {}
    if variant in ("six", "seven", "eight", "nine") or variant in LATER:
        assert fit.get("graph") in (None, "relative", "wide"), "a second stage the reader does not read"
        flags = flags_of(variant, fit.get("graph"))
        data = data[:4] + bytes([3, flags]) + data[5:]
        sizes["flags"] = flags
    elif cues.VARIANTS[variant]:
        data = data[:4] + bytes([2]) + data[5:]
    (models / "keys.bin").write_bytes(data)
    sizes.update({"file bytes": len(data), "KiB": round(len(data) / 1024, 1),
                  "sha256": hashlib.sha256(data).hexdigest()})
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
    """The variant's build over contiguous chunks in file order, as the reader reads."""
    chosen = [s for s in schemas if s["split"] == split and s["fk"]]
    n = graph.WORKERS * 4
    size = max(1, -(-len(chosen) // n))
    later = VARIANT in LATER
    jobs = [(chosen[i:i + size], split, {}, LATER[VARIANT] if later else VARIANT) for i in range(0, len(chosen), size)]
    import scipy.sparse as sp
    xs, meta = [], []
    with letters.pool(graph.WORKERS) as pool:
        for x, m in pool.map(ten.build if later else cues.build, jobs):
            xs.append(x)
            meta.extend(m)
    return sp.vstack(xs).tocsr(), meta


def second_features(fit: dict, schemas: list):
    """The second stage a fit's graph-final.json names, and its features' names."""
    if fit.get("graph") == "wide":
        import wide  # noqa: E402  (round 1v's)
        sizes = {s["id"]: len(s["tables"]) for s in schemas}
        return (lambda meta, scores: wide.KINDS["wide"](meta, scores, sizes)), wide.NAMES["wide"]
    import stack  # noqa: E402  (round 1t's)
    names = list(graph.GRAPH_FEATURES) + (list(stack.EXTRA) if fit.get("graph") == "relative" else [])
    return stack.of(fit), names


def named_pairs(schema, split):
    """One schema's pairs with their named features, as the variant builds them."""
    if VARIANT in LATER:
        return (row[:8] for row in ten.pairs([schema], split, {}, LATER[VARIANT]))
    return cues.pairs([schema], split, {}, VARIANT)


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
    features, feature_names = second_features(json.load(open(models / "graph-final.json", encoding="utf-8")), schemas)
    g = features(meta, p1)
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
    # The second stage's features, bit for bit, where the reader wrote them
    if Path(f"{stem}.second.bin").exists():
        py = np.ascontiguousarray(g, dtype="<f4")
        rs = np.fromfile(f"{stem}.second.bin", dtype="<f4")
        result["second stage, features a pair"] = int(py.shape[1])
        if rs.size != py.size:
            result["second stage, features aligned"] = False
        else:
            rs = rs.reshape(py.shape)
            differ = py.view(np.uint32) != rs.view(np.uint32)
            rows = np.flatnonzero(differ.any(axis=1))
            result["second stage, pairs whose features differ"] = int(rows.size)
            result["second stage, features that differ"] = {
                feature_names[j]: {"pairs": int(differ[:, j].sum()),
                                   "largest difference": float(np.abs(py[:, j].astype(np.float64) - rs[:, j]).max())}
                for j in np.flatnonzero(differ.any(axis=0))}
            result["second stage, pairs whose features differ, shown"] = [
                {"pair": [meta[i][graph.SID], meta[i][graph.TABLE], meta[i][graph.COLUMN], meta[i][graph.TARGET]],
                 "differs": {feature_names[j]: [float(py[i, j]), float(rs[i, j])] for j in np.flatnonzero(differ[i])}}
                for i in rows[:5]]
    # The first few pairs whose features differ, named feature by feature
    shown = []
    for i in bad_feats[:5]:
        m = meta[i]
        py = None
        sch = next(s for s in schemas if s["id"] == m[graph.SID])
        for s, t, c, tg, _, _, f, _ in named_pairs(sch, split):
            if (t["name"], c["name"], tg["name"]) == (m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]):
                py = f
                break
        dumped = subprocess.run([str(reader), "dump", str(out / f"{name}.jsonl"), m[graph.SID], VARIANT], check=True,
                                capture_output=True, text=True, encoding="utf-8").stdout.splitlines()
        rs = next((json.loads(line)[3] for line in dumped if json.loads(line)[:3] ==
                   [m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]]), {})
        keys_ = sorted(set(py or {}) | set(rs))
        shown.append({"pair": list(m[:1]) + [m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]],
                      "differs": {k: [(py or {}).get(k), rs.get(k)] for k in keys_ if (py or {}).get(k) != rs.get(k)}})
    result["pairs that differ, shown"] = shown
    return result


def chosen_of(path: Path, split: str, part) -> list:
    """A set's schemas of the split that declare a key, in file order: all,
    the first `part` of them, or those of the source `part` names."""
    count = int(part) if part is not None and part.isdigit() else None
    out = []
    for line in open(path, encoding="utf-8"):
        s = json.loads(line)
        if s["split"] != split or not s["fk"]:
            continue
        if part is not None and count is None and s.get("source") != part:
            continue
        out.append(s)
        if count is not None and len(out) >= count:
            break
    return out


def check(models: Path, data: Path, ext: Path, reader: Path, out: Path, parts: dict) -> None:
    global VARIANT
    assert letters.is_sorted()
    VARIANT = variant_of(models)
    out.mkdir(parents=True, exist_ok=True)
    fit = json.load(open(models / "graph-final.json", encoding="utf-8"))
    threshold = fit["threshold"]
    result = {"keys.bin sha256": hashlib.sha256((models / "keys.bin").read_bytes()).hexdigest(),
              "threshold": threshold, "variant": VARIANT, "second stage": fit.get("graph", "graph"),
              "read of each set": {k: v or "all" for k, v in parts.items()}}
    for name, folder in (("external", ext), ("test", data)):
        chosen = chosen_of(folder / "hidden.jsonl", name, parts[name])
        with open(out / f"{name}.jsonl", "w", encoding="utf-8") as f:
            for s in chosen:
                f.write(json.dumps(s, ensure_ascii=False) + "\n")
        result[name] = one_set(name, chosen, name, models, reader, threshold, out)
        print(name, json.dumps(result[name], default=str)[:3000], flush=True)
    (out / "parity.json").write_text(json.dumps(result, indent=2, default=str) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "pack":
        pack(Path(sys.argv[2]))
    else:
        args, options = sys.argv[2:], {"--external": None, "--test": None, "--workers": "2"}
        while len(args) > 5:
            assert args[-2] in options, f"unknown option {args[-2]}"
            options[args[-2]] = args[-1]
            args = args[:-2]
        graph.WORKERS = int(options["--workers"])
        check(Path(args[0]), Path(args[1]), Path(args[2]), Path(args[3]).resolve(), Path(args[4]),
              {"external": options["--external"], "test": options["--test"]})
