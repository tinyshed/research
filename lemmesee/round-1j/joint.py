"""Early fusion of the existing semantic resources and observed results.

prepare reads the public corpus and makes immutable input arrays; run fits
all preregistered comparisons. Only run measures model quality. Neither
command connects to a service or asks a language model.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.metadata
import importlib.util
import json
import math
import os
import platform
import re
import struct
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
OUT = ROOT / "corpus" / "round-1j"
SEED = 20261101
FOLDS = 5
DRAWS = 3
SLOTS = ("base", "measure", "time", "by")
PROPS = ("id", "money", "count", "sum_ok", "audit_time", "event_time", "good_group", "cryptic", "importance")
CONFIGS = ("baseline", "capacity", "late", "early", "joint")
PROFILE_NAMES = (
    "value_log_magnitude", "value_integer", "value_negative", "value_exponent",
    "value_digits", "value_equal", "label_blank", "label_numeric",
    "label_single_character", "label_digits", "label_letters", "label_length",
    "label_distinct", "label_word_known",
)


def module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def clean_json(x):
    if isinstance(x, dict):
        return {str(k): clean_json(v) for k, v in x.items()}
    if isinstance(x, (list, tuple)):
        return [clean_json(v) for v in x]
    if isinstance(x, (set, frozenset)):
        return [clean_json(v) for v in sorted(x, key=repr)]
    if isinstance(x, np.ndarray):
        return clean_json(x.tolist())
    if isinstance(x, np.generic):
        return clean_json(x.item())
    if isinstance(x, float) and not math.isfinite(x):
        return None
    return x


def write_json(path: Path, data) -> None:
    path.write_text(json.dumps(clean_json(data), ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def git(*args: str) -> str:
    return subprocess.check_output(["git", "-C", str(ROOT), *args], text=True).strip()


def environment() -> dict:
    info = {
        "utc": datetime.now(timezone.utc).isoformat(),
        "platform": platform.platform(),
        "python": platform.python_version(),
        "packages": {p: importlib.metadata.version(p) for p in ("numpy", "scipy", "scikit-learn", "lightgbm")},
        "revision": git("rev-parse", "HEAD"),
        "dirty": git("status", "--porcelain", "--untracked-files=normal").splitlines(),
        "threads": {k: os.environ.get(k) for k in ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS")},
    }
    if platform.system() == "Linux":
        cpu = Path("/proc/cpuinfo").read_text(encoding="utf-8")
        info["cpu"] = next((line.split(":", 1)[1].strip() for line in cpu.splitlines()
                            if line.startswith("model name")), "unknown")
        info["memory"] = Path("/proc/meminfo").read_text(encoding="utf-8").splitlines()[0]
        quota = Path("/sys/fs/cgroup/cpu.max")
        info["cpu_quota"] = quota.read_text().strip() if quota.exists() else "unknown"
    return info


def words(slot: list[str]) -> list[str]:
    return [w.split(":", 1)[-1] for w in slot if not w.startswith("#")]


def semantic(slots: dict, lex: dict, vectors: dict) -> np.ndarray:
    """Keep slot identity and lexical meaning; do not collapse to keep."""
    out = []
    dims = len(next(iter(vectors.values())))
    for slot in SLOTS:
        ws = words(slots.get(slot, []))
        known = [np.asarray(lex[w], dtype=float) for w in ws if w in lex]
        out.extend(np.max(known, axis=0) if known else np.full(len(PROPS), np.nan))
        vs = [vectors[w] for w in ws if w in vectors]
        out.extend(np.mean(vs, axis=0) if vs else np.full(dims, np.nan))
        out.append(len(vs) / len(ws) if ws else 0.0)
    return np.asarray(out, dtype=np.float32)


def number(value):
    try:
        n = float(value)
        return n if math.isfinite(n) else None
    except (TypeError, ValueError, OverflowError):
        return None


def mean_or_nan(xs) -> float:
    return float(np.mean(xs)) if xs else math.nan


def profile(panel: dict, vocabulary: set[str]) -> np.ndarray:
    """Descriptive features of representation; no guessed business verdict."""
    rows = panel.get("rows") or []
    values = [r[-1] for r in rows if r]
    valid = [(str(v), number(v)) for v in values if number(v) is not None]
    labels = [r[0] for r in rows if len(r) > 1]
    present = [str(x).strip() for x in labels if x is not None and str(x).strip()]
    numeric_labels = [s for s in present if number(s) is not None]
    tokens = [t for s in present for t in re.findall(r"[a-z]+", s.lower())]
    out = [
        mean_or_nan([math.log10(1 + abs(n)) for _, n in valid]),
        mean_or_nan([n.is_integer() for _, n in valid]),
        mean_or_nan([n < 0 for _, n in valid]),
        mean_or_nan(["e" in s.lower() for s, _ in valid]),
        mean_or_nan([sum(c.isdigit() for c in s) for s, _ in valid]),
        (len({n for _, n in valid}) / len(valid)) if valid else math.nan,
        1 - len(present) / len(labels) if labels else math.nan,
        len(numeric_labels) / len(present) if present else math.nan,
        mean_or_nan([len(s) == 1 for s in present]),
        mean_or_nan([sum(c.isdigit() for c in s) / max(1, len(s)) for s in present]),
        mean_or_nan([sum(c.isalpha() for c in s) / max(1, len(s)) for s in present]),
        mean_or_nan([len(s) for s in present]),
        len(set(present)) / len(present) if present else math.nan,
        mean_or_nan([t in vocabulary for t in tokens]),
    ]
    return np.asarray(out, dtype=np.float32)


def folds(families: list[str], draws: int = DRAWS):
    fams = sorted(set(families))
    for draw in range(draws):
        order = fams.copy()
        np.random.default_rng(SEED + draw).shuffle(order)
        where = {f: i % FOLDS for i, f in enumerate(order)}
        for k in range(FOLDS):
            test = np.asarray([where[f] == k for f in families])
            yield draw, k, ~test, test


def prepare(out: Path) -> None:
    dg = module("joint_diagnose", ROOT / "round-1g" / "diagnose.py")
    vec_mod = module("joint_vectors", ROOT / "round-1h" / "vectors.py")
    dirs = [ROOT / "corpus" / f"round-1{x}" / "look" for x in ("d", "e", "f")]
    D = dg.load_all(dirs, ROOT / "corpus" / "see")
    lex_path = ROOT / "corpus" / "round-1g" / "lexicon" / "lexicon.json"
    vec_path = ROOT / "corpus" / "round-1h" / "vectors" / "vectors.json"
    lex = json.loads(lex_path.read_text(encoding="utf-8"))["words"]
    vectors = vec_mod.load(ROOT / "corpus" / "round-1h")
    vocabulary = set(vectors)
    numeric_names = dg.look1d.RESULT + ["score", "rank"] + dg.look1d.QUESTION + dg.look1e.MORE
    flat, base, sem, prof, late, y, dbs, families, counters, meanings, identities = [], [], [], [], [], [], [], [], [], [], {}
    for fe in D["fes"]:
        identities[fe] = D["ids"][fe]
        for i, p in enumerate(D["mine"][fe]):
            t = dg.target(p, D["ans"])
            if t is None:
                continue
            flat.append({"id": p["id"], "db": fe, "title": p["title"], "identity": D["ids"][fe][i]})
            base.append(dg.vector(p, "numbers"))
            sem.append(semantic(p["slots"], lex, vectors))
            prof.append(profile(p, vocabulary))
            late.append([p.get("name", math.nan)])
            y.append(t)
            dbs.append(fe)
            families.append(D["family"][fe])
            counters.append([float(bool(D["ans"][j][p["id"]]["keep"])) if p["id"] in D["ans"][j] else math.nan
                             for j in dg.COUNTERS])
            meanings.append([float(D["ans"][j][p["id"]].get("meaning") == "no") if p["id"] in D["ans"][j] else math.nan
                             for j in dg.COUNTERS])
    out.mkdir(parents=True, exist_ok=True)
    arrays = {"base": np.asarray(base, dtype=np.float32), "semantic": np.asarray(sem, dtype=np.float32),
              "profile": np.asarray(prof, dtype=np.float32), "late": np.asarray(late, dtype=np.float32),
              "y": np.asarray(y), "db": np.asarray(dbs), "family": np.asarray(families),
              "judges": np.asarray(counters), "meaning_no": np.asarray(meanings)}
    np.savez_compressed(out / "data.npz", **arrays)
    inputs = [lex_path, vec_path, ROOT / "corpus" / "round-1h" / "names.json",
              ROOT / "corpus" / "round-1c" / "learn" / "words.json"]
    for directory in dirs:
        inputs.extend(directory.glob("*/pool.json"))
        inputs.extend(directory.glob("key.json"))
        for judge in dg.TEACHERS + dg.COUNTERS:
            inputs.extend(directory.glob(f"{judge}-*.json"))
    inputs.extend((ROOT / "corpus" / "see").glob("*/snapshot.json"))
    meta = {"environment": environment(), "panels": flat, "judges": dg.COUNTERS,
            "numeric_names": numeric_names, "profile_names": PROFILE_NAMES,
            "inputs": {str(p.relative_to(ROOT)).replace(os.sep, "/"): sha(p) for p in sorted(set(inputs))},
            "data_sha256": sha(out / "data.npz"), "script_sha256": sha(Path(__file__)),
            "source_sha256": {str(p.relative_to(ROOT)): sha(p)
                              for part in ("c", "d", "e", "f", "g", "h", "j")
                              for p in sorted((ROOT / f"round-1{part}").glob("*.py"))}}
    write_json(out / "data.json", meta)
    print("prepared frozen public inputs", flush=True)


def normalise(train, test):
    """Fit both imputation and scaling on the training rows only."""
    train, test = np.asarray(train, dtype=float), np.asarray(test, dtype=float)
    finite = np.isfinite(train)
    count = finite.sum(axis=0)
    mean = np.divide(np.where(finite, train, 0).sum(axis=0), count, out=np.zeros(train.shape[1]), where=count > 0)
    a = np.where(finite, train, mean)
    scale = np.maximum(a.std(axis=0), 1e-6)
    transform = lambda z: np.clip((np.where(np.isfinite(z), z, mean) - mean) / scale, -10, 10)
    return transform(train), transform(test), mean, scale


def joint_matrix(base, sem, prof):
    """Return semantic/result interactions, with their inputs retained."""
    # Result features and the question's aggregation/shape flags form the
    # context in which a word vector may mean something different.
    contexts = np.column_stack((base[:, :16], prof))
    products = np.einsum("ij,ik->ijk", sem, contexts).reshape(len(base), -1)
    main = np.column_stack((base, sem, prof))
    return np.column_stack((main, np.isfinite(main).astype(float), products))


def pack_linear(model: dict) -> bytes:
    n = len(model["coefficients"])
    return (b"LMJ1" + struct.pack("<If", n, float(model["intercept"]))
            + b"".join(np.asarray(model[k], dtype="<f4").tobytes()
                       for k in ("coefficients", "mean", "scale")))


def predict_linear(blob: bytes, X) -> np.ndarray:
    if blob[:4] != b"LMJ1":
        raise ValueError("Unknown linear model format")
    n, intercept = struct.unpack_from("<If", blob, 4)
    if len(blob) != 12 + 12 * n:
        raise ValueError("Truncated linear model")
    coefficients, mean, scale = np.frombuffer(blob, dtype="<f4", offset=12).reshape(3, n).astype(float)
    X = np.asarray(X, dtype=float)
    normal = np.clip((np.where(np.isfinite(X), X, mean) - mean) / scale, -10, 10)
    logits = normal @ coefficients + intercept
    return 1 / (1 + np.exp(-np.clip(logits, -700, 700)))


def pack_tree(model) -> bytes:
    """Retain double thresholds: rounding one across a float32 feature
    changes its branch. The old format's approximate check happened not
    to encounter that boundary on its mostly discrete features."""
    dump = model.dump_model()
    out = bytearray(b"LMJ2" + struct.pack("<HH", len(dump["feature_names"]), len(dump["tree_info"])))
    missing = {"None": 0, "Zero": 1, "NaN": 2}
    for tree in dump["tree_info"]:
        nodes, leaves = [], []

        def visit(node):
            if "leaf_value" in node:
                leaves.append(node["leaf_value"])
                return -len(leaves)
            i = len(nodes)
            nodes.append(None)
            left, right = visit(node["left_child"]), visit(node["right_child"])
            if node["decision_type"] != "<=":
                raise ValueError("Only numeric splits are supported")
            flags = missing[node["missing_type"]] | (4 if node["default_left"] else 0)
            nodes[i] = struct.pack("<HBdhh", node["split_feature"], flags, node["threshold"], left, right)
            return i

        visit(tree["tree_structure"])
        out += struct.pack("<HH", len(nodes), len(leaves))
        out += b"".join(nodes)
        out += struct.pack(f"<{len(leaves)}f", *leaves)
    return bytes(out)


class PackedTree:
    def __init__(self, blob: bytes):
        if blob[:4] != b"LMJ2":
            raise ValueError("Unknown tree format")
        self.features, count = struct.unpack_from("<HH", blob, 4)
        at, self.trees = 8, []
        for _ in range(count):
            nn, nl = struct.unpack_from("<HH", blob, at)
            at += 4
            nodes = [struct.unpack_from("<HBdhh", blob, at + 15 * i) for i in range(nn)]
            at += 15 * nn
            leaves = struct.unpack_from(f"<{nl}f", blob, at)
            at += 4 * nl
            self.trees.append((nodes, leaves))
        if at != len(blob):
            raise ValueError("Trailing bytes in tree format")

    def score(self, row):
        total = 0.0
        for nodes, leaves in self.trees:
            if not nodes:
                total += leaves[0]
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
            total += leaves[-1 - at]
        return total


def fit_predict(config, X, y, train, test, seed):
    if config == "joint":
        from sklearn.linear_model import LogisticRegression
        a, b, mean, scale = normalise(X[train], X[test])
        # Duplicating fractional labels as weighted positive and negative
        # rows gives their exact Bernoulli cross-entropy, not rounded labels.
        model = LogisticRegression(C=0.1, max_iter=1000, solver="lbfgs", random_state=seed)
        yy = y[train]
        model.fit(np.vstack((a, a)), np.r_[np.ones(len(a)), np.zeros(len(a))],
                  sample_weight=np.r_[yy, 1 - yy])
        return model.predict_proba(b)[:, 1], {
            "coefficients": model.coef_[0], "intercept": model.intercept_[0],
            "mean": mean, "scale": scale,
        }
    import lightgbm as lgb
    params = dict(objective="cross_entropy", learning_rate=0.05, num_leaves=4 if config == "baseline" else 15,
                  min_data_in_leaf=10, feature_fraction=0.5, bagging_fraction=0.9, bagging_freq=1,
                  lambda_l2=1.0, verbose=-1, seed=seed, deterministic=True, force_row_wise=True, num_threads=4)
    model = lgb.train(params, lgb.Dataset(X[train], y[train]), num_boost_round=150 if config == "baseline" else 400)
    return model.predict(X[test]), model


def auc(y, s):
    pos, neg = np.asarray(s)[np.asarray(y) == 1], np.asarray(s)[np.asarray(y) == 0]
    if not len(pos) or not len(neg):
        return math.nan
    return float(np.mean((pos[:, None] > neg[None, :]) + 0.5 * (pos[:, None] == neg[None, :])))


def paired_interval(differences: dict, family_by_db: dict, seed=SEED):
    """Bootstrap independent families, preserving every database in a draw."""
    grouped = {}
    for db, delta in differences.items():
        grouped.setdefault(family_by_db[db], []).append(delta)
    groups = list(grouped.values())
    rng = np.random.default_rng(seed)
    boots = [np.mean([v for i in rng.integers(0, len(groups), len(groups)) for v in groups[i]]) for _ in range(5000)]
    return {"mean": float(np.mean(list(differences.values()))),
            "95%": [float(np.percentile(boots, q)) for q in (2.5, 97.5)], "databases": len(differences),
            "families": len(groups)}


def run(out: Path, configs: list[str], draws: int) -> None:
    if platform.system() != "Linux":
        raise SystemExit("Learning comparisons run in the Linux container.")
    if git("status", "--porcelain", "--untracked-files=all", "--", "round-1j"):
        raise SystemExit("Commit the round-1j harness before measuring it.")
    meta = json.loads((out / "data.json").read_text(encoding="utf-8"))
    if sha(out / "data.npz") != meta["data_sha256"] or sha(Path(__file__)) != meta["script_sha256"]:
        raise SystemExit("Frozen inputs or harness changed; prepare again before running.")
    if any(sha(ROOT / p) != digest for p, digest in meta["source_sha256"].items()):
        raise SystemExit("A source dependency changed; prepare again before running.")
    arrays = dict(np.load(out / "data.npz", allow_pickle=False))
    base, sem, prof = arrays["base"], arrays["semantic"], arrays["profile"]
    matrices = {"baseline": base, "capacity": base, "late": np.column_stack((base, arrays["late"])),
                "early": np.column_stack((base, sem, prof)), "joint": joint_matrix(base, sem, prof)}
    dbs = arrays["db"].tolist()
    unique = sorted(set(dbs))
    db_indices = {db: np.flatnonzero(arrays["db"] == db) for db in unique}
    family_by_db = {db: str(arrays["family"][indices[0]]) for db, indices in db_indices.items()}
    scores = {c: np.full((draws, len(base)), np.nan) for c in configs}
    packed = {c: {"largest_bytes": 0, "largest_probability_difference": 0.0} for c in configs}
    for draw, fold, train, test in folds(arrays["family"].tolist(), draws):
        # Sequential fitting avoids measurements competing for processors.
        for config in configs:
            pred, model = fit_predict(config, matrices[config], arrays["y"], train, test, SEED + 100 * draw + fold)
            scores[config][draw, test] = pred
            if config == "joint":
                blob = pack_linear(model)
                check = predict_linear(blob, matrices[config][test][:20])
            else:
                blob = pack_tree(model)
                reader = PackedTree(blob)
                margins = np.asarray([reader.score(row) for row in matrices[config][test][:20]])
                check = 1 / (1 + np.exp(-margins))
            difference = float(np.max(np.abs(check - pred[:20])))
            if difference > 1e-5:
                raise RuntimeError(f"Packed model differs by {difference}")
            packed[config]["largest_bytes"] = max(packed[config]["largest_bytes"], len(blob))
            packed[config]["largest_probability_difference"] = max(packed[config]["largest_probability_difference"], difference)
            print(f"draw {draw + 1} fold {fold + 1}: {config}", flush=True)
    if any(not np.isfinite(s).all() for s in scores.values()):
        raise RuntimeError("An out-of-fold score is missing or not finite.")
    dg = module("joint_diagnose_count", ROOT / "round-1g" / "diagnose.py")
    set_mod = module("joint_sets_count", ROOT / "round-1g" / "sets.py")
    metrics = {}
    for config in configs:
        per = {}
        for j, judge in enumerate(meta["judges"]):
            for policy_name, penalties in (("plain", dg.NO_PENALTY), ("axes", set_mod.AXIS_ONCE)):
                for db, indices in db_indices.items():
                    y = arrays["judges"][indices, j]
                    if not np.isfinite(y).all():
                        continue
                    ids = [meta["panels"][int(i)]["identity"] for i in indices]
                    chooser = dg.look1e.Chooser(ids)
                    aa, kk, harmful = [], [], []
                    for prediction in scores[config][:, indices]:
                        selected = chooser.choose(prediction, penalties)
                        aa.append(auc(y, prediction))
                        kk.append(float(y[selected].sum()))
                        harmful.append(float((y[selected] * arrays["meaning_no"][indices, j][selected]).sum()))
                    for name, values in (("auc", aa), ("kept", kk), ("harmful_kept", harmful)):
                        valid = [v for v in values if math.isfinite(v)]
                        if valid:
                            per.setdefault(f"{judge}/{policy_name}/{name}", {})[db] = float(np.mean(valid))
        metrics[config] = per
    result = {"environment": environment(), "input_sha256": meta["data_sha256"],
              "configuration": {"folds": FOLDS, "draws": draws, "seed": SEED, "configs": configs},
              "packed_weights": packed,
              "scope": "development-family cross-validation; saved per-panel judges, not a new set judgment"}
    for config in configs:
        result[config] = {k: {"mean": float(np.mean(list(v.values()))), "databases": len(v)}
                          for k, v in metrics[config].items()}
        if config != "baseline" and "baseline" in metrics:
            for key, vals in metrics[config].items():
                common = {db: v - metrics["baseline"][key][db] for db, v in vals.items()
                          if db in metrics["baseline"][key]}
                if common:
                    result[config][key]["minus_baseline"] = paired_interval(common, family_by_db)
    write_json(out / "evaluate.json", result)
    write_json(out / "scores.json", {config: {db: scores[config][:, indices].mean(axis=0)
                                            for db, indices in db_indices.items()} for config in configs})
    write_json(out / "per-database.json", metrics)
    np.savez_compressed(out / "oof.npz", **scores)
    print(json.dumps(clean_json(result), ensure_ascii=False, indent=2), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "run"))
    parser.add_argument("--out", type=Path, default=OUT)
    parser.add_argument("--configs", nargs="+", choices=CONFIGS, default=list(CONFIGS))
    parser.add_argument("--draws", type=int, default=DRAWS)
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.out)
    else:
        run(args.out, args.configs, args.draws)
