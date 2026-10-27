"""Round 1b's second pass on foreign keys: features that read names letter
by letter, each column's targets measured against each other, one target
kept per column, and LightGBM tuned on the validation split.

    .venv/Scripts/python -I keys.py dev <out>     fit on training, measure on validation; the test is not read
    .venv/Scripts/python -I keys.py final <out>   the configuration dev chose, measured once on the test

reads hidden.jsonl and guesses.jsonl from <out> and writes keys-dev.json or
keys-final.json there.
"""

import json
import random
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import lightgbm as lgb  # noqa: E402
import numpy as np  # noqa: E402
from sklearn.feature_extraction import FeatureHasher  # noqa: E402
from sklearn.metrics import average_precision_score, precision_recall_curve  # noqa: E402

from train import (  # noqa: E402
    HASHED, KIND, NEGATIVES, SEED, bootstrap, counts, key_kind, norm, pair_features, prf, singular, tokens,
)

# Suffixes that name a key without naming its table: `customer_no`, `user_ref`
KEY_WORDS = {"id", "key", "code", "no", "num", "nr", "fk", "ref"}

# The second pass's configurations; `final` takes the one dev chose
CONFIGS = {
    "first pass": {"features": 1, "params": dict(n_estimators=200, num_leaves=15, learning_rate=0.1)},
    "letters": {"features": 2, "params": dict(n_estimators=200, num_leaves=15, learning_rate=0.1)},
    "letters, 31 leaves": {"features": 2, "params": dict(n_estimators=3000, num_leaves=31, learning_rate=0.05), "stop": 100},
    "letters, 63 leaves": {"features": 2, "params": dict(n_estimators=3000, num_leaves=63, learning_rate=0.05), "stop": 100},
}


def grams(s: str) -> set[str]:
    s = f"^{s}$"
    return {s[i:i + 3] for i in range(len(s) - 2)}


def jaccard(a: set, b: set) -> float:
    return len(a & b) / len(a | b) if a or b else 0.0


def similarity(a: str, b: str) -> float:
    """1 less the edit distance over the longer length."""
    if not a or not b:
        return 0.0
    previous = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        current = [i]
        for j, cb in enumerate(b, 1):
            current.append(min(previous[j] + 1, current[j - 1] + 1, previous[j - 1] + (ca != cb)))
        previous = current
    return 1 - previous[-1] / max(len(a), len(b))


def prefix(a: str, b: str) -> int:
    n = 0
    for x, y in zip(a, b):
        if x != y:
            break
        n += 1
    return n


def stem_of(name: str):
    tk = tokens(name)
    st = tk[:-1] if len(tk) > 1 and tk[-1] in KEY_WORDS else tk
    return "".join(st), tk


def pairs(schemas, split, guesses, version, sample=None, rng=None):
    """(schema, table, column, target, label, rules, features) for every
    candidate pair, each column's targets scored against each other."""
    for s in schemas:
        if s["split"] != split or not s["fk"]:
            continue
        declared = {(a, c): b for a, c, b in s["fk"]}
        targets = []
        for t in s["tables"]:
            kind = key_kind(t)
            if not kind:
                continue
            g = norm(t["name"])
            pk = next(c for c in t["columns"] if c["name"] == t["pk"][0])
            targets.append({
                "table": t, "kind": kind, "name": g, "singular": singular(g),
                "grams": grams(g) | grams(singular(g)), "pk": norm(pk["name"]), "pk_type": pk.get("type"),
            })
        stems = [stem_of(c["name"])[0] for t in s["tables"] for c in t["columns"]]
        named_by = {id(g): sum(1 for st in stems if st and st in (g["name"], g["singular"])) for g in targets}
        all_tokens = [tokens(c["name"]) for t in s["tables"] for c in t["columns"]]
        id_convention = sum(1 for tk in all_tokens if tk and tk[-1] == "id") / max(len(all_tokens), 1)

        for table in s["tables"]:
            for column in table["columns"]:
                kind = KIND.get(column["family"])
                if not kind or (column["primary"] and len(table["pk"]) == 1):
                    continue
                stem, tk = stem_of(column["name"])
                sg = grams(stem) if stem else set()
                mine = [g for g in targets if g["kind"] == kind
                        and not (g["table"] is table and column["name"] == table["pk"][0])]
                base = []
                for g in mine:
                    exact = stem in (g["name"], g["singular"]) or stem + "s" == g["name"]
                    p = prefix(stem, g["singular"])
                    b = max(float(exact), jaccard(sg, g["grams"]), float(norm(column["name"]) == g["pk"]),
                            0.8 * p / max(1, min(len(stem), len(g["singular"]))) if p >= 3 else 0.0)
                    base.append(b)
                best = max(base, default=0.0)
                guess = guesses.get((s["id"], table["name"], column["name"]), {}).get("fk")
                for g, b in zip(mine, base):
                    target = g["table"]
                    label = declared.get((table["name"], column["name"])) == target["name"]
                    if sample is not None and not label and rng.random() > sample:
                        continue
                    f = pair_features(s, table, column, target, with_index=True)
                    if version >= 2:
                        others = [x for x, h in zip(base, mine) if h is not g]
                        f["v2 letters"] = jaccard(sg, g["grams"])
                        f["v2 edit"] = similarity(stem, g["singular"])
                        f["v2 prefix"] = min(prefix(stem, g["singular"]), 8) / 8
                        f["v2 base"] = b
                        f["v2 best of the column"] = float(b >= best)
                        f["v2 margin"] = b - max(others, default=0.0)
                        f["v2 close rivals"] = min(sum(1 for x in others if x >= 0.5), 5) / 5
                        f["v2 named as the key"] = float(norm(column["name"]) == g["pk"])
                        f["v2 same type"] = float(column.get("type") == g["pk_type"])
                        f["v2 named by"] = min(named_by[id(g)], 10) / 10
                        f["v2 id convention"] = id_convention
                        f["v2 bare"] = float(not stem)
                        f["v2 key word:" + (tk[-1] if tk else "")] = 1.0
                        for gram in list(sg)[:12]:
                            f["cg:" + gram] = 1.0
                        f[f"p3:{stem[:3]}|{g['singular'][:3]}"] = 1.0
                    yield s, table, column, target, label, guess == target["name"], f


def matrix(hasher, rows):
    """Features, labels, the rules' guesses, column keys, repositories, novelty."""
    feats, y, rules, keys, repos, novel = [], [], [], [], [], []
    for s, t, c, _, label, guess, f in rows:
        feats.append(f)
        y.append(label)
        rules.append(guess)
        keys.append((s["id"], t["name"], c["name"]))
        repos.append(s["repo"])
        novel.append(bool(s.get("novel")))
    x = hasher.transform(feats).astype(np.float32)
    return x, np.array(y, dtype=bool), np.array(rules, dtype=bool), keys, np.array(repos), np.array(novel, dtype=bool)


def one_target(scores, keys):
    """Each column keeps its best-scored target; the others score nothing."""
    best = {}
    for i, k in enumerate(keys):
        if k not in best or scores[i] > scores[best[k]]:
            best[k] = i
    kept = np.zeros_like(scores)
    idx = np.fromiter(best.values(), dtype=np.int64)
    kept[idx] = scores[idx]
    return kept


def at_threshold(y, scores, threshold):
    return prf(*counts(y, scores >= threshold))


def best_threshold(y, scores):
    p, r, thresholds = precision_recall_curve(y, scores)
    f1 = 2 * p * r / np.maximum(p + r, 1e-12)
    i = int(np.argmax(f1[:-1]))
    return float(thresholds[i])


def recall_at(y, scores, precision):
    p, r, _ = precision_recall_curve(y, scores)
    return float(r[p >= precision].max()) if (p >= precision).any() else 0.0


def fit(config, x, y, xv, yv):
    params = dict(subsample=0.8, subsample_freq=1, colsample_bytree=0.5, min_child_samples=20,
                  random_state=SEED, verbose=-1, **config["params"])
    model = lgb.LGBMClassifier(**params)
    if "stop" in config:
        model.fit(x, y, eval_set=[(xv, yv)], eval_metric="average_precision",
                  callbacks=[lgb.early_stopping(config["stop"], verbose=False)])
    else:
        model.fit(x, y)
    return model


def load(out: Path):
    schemas = [json.loads(line) for line in open(out / "hidden.jsonl", encoding="utf-8")]
    guesses = {}
    for line in open(out / "guesses.jsonl", encoding="utf-8"):
        g = json.loads(line)
        guesses[(g["schema"], g["table"], g["column"])] = g
    return schemas, guesses


def data(schemas, guesses, version, split):
    hasher = FeatureHasher(n_features=HASHED, input_type="dict", alternate_sign=False)
    rng = random.Random(SEED)
    if split == "train":
        return matrix(hasher, pairs(schemas, "train", guesses, version, sample=NEGATIVES, rng=rng))
    return matrix(hasher, pairs(schemas, split, guesses, version))


def dev(out: Path) -> None:
    schemas, guesses = load(out)
    results = {}
    cache = {}
    for name, config in CONFIGS.items():
        v = config["features"]
        if v not in cache:
            t0 = time.time()
            cache[v] = (data(schemas, guesses, v, "train"), data(schemas, guesses, v, "validation"))
            print(f"features {v}: {time.time() - t0:.0f} s", flush=True)
        (x, y, *_), (xv, yv, vrules, vkeys, _, vnovel) = cache[v]
        t0 = time.time()
        model = fit(config, x, y, xv, yv)
        scores = model.predict_proba(xv)[:, 1]
        rules = prf(*counts(yv, vrules))
        entry = {"fit_seconds": round(time.time() - t0), "trees": model.booster_.num_trees(),
                 "kib": round(len(model.booster_.model_to_string().encode()) / 1024)}
        for how, s in (("pairs", scores), ("one target", one_target(scores, vkeys))):
            t = best_threshold(yv, s)
            p, r, f = at_threshold(yv, s, t)
            np_, nr_, nf_ = at_threshold(yv[vnovel], s[vnovel], t)
            entry[how] = {
                "precision": p, "recall": r, "f1": f, "average_precision": float(average_precision_score(yv, s)),
                "recall_at_rules_precision": recall_at(yv, s, rules[0]), "novel_f1": nf_,
            }
        results[name] = entry
        results["rules"] = dict(zip(("precision", "recall", "f1"), rules))
        print(name, json.dumps(entry), flush=True)
    print("rules", json.dumps(results["rules"]))
    (out / "keys-dev.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")


def final(out: Path, name: str, how: str) -> None:
    schemas, guesses = load(out)
    config = CONFIGS[name]
    v = config["features"]
    x, y, *_ = data(schemas, guesses, v, "train")
    xv, yv, _, vkeys, _, _ = data(schemas, guesses, v, "validation")
    model = fit(config, x, y, xv, yv)
    pick = (lambda s, k: one_target(s, k)) if how == "one target" else (lambda s, k: s)
    threshold = best_threshold(yv, pick(model.predict_proba(xv)[:, 1], vkeys))
    del x, xv
    xt, ty, trules, tkeys, trepos, tnovel = data(schemas, guesses, v, "test")
    scores = pick(model.predict_proba(xt)[:, 1], tkeys)
    predicted = scores >= threshold
    rng = np.random.default_rng(SEED)
    rules = prf(*counts(ty, trules))
    result = {"configuration": name, "how": how, "threshold": threshold, "trees": model.booster_.num_trees(),
              "kib": round(len(model.booster_.model_to_string().encode()) / 1024, 1)}
    for subset, mask in (("all", np.ones_like(ty)), ("novel", tnovel)):
        yy, pp, rr, ss = ty[mask], predicted[mask], trules[mask], scores[mask]
        low, high = bootstrap(yy, pp, rr, trepos[mask], rng)
        result[subset] = {
            "rows": int(mask.sum()), "positives": int(yy.sum()),
            "rules": dict(zip(("precision", "recall", "f1"), prf(*counts(yy, rr)))),
            "model": dict(zip(("precision", "recall", "f1"), prf(*counts(yy, pp)))),
            "average_precision": float(average_precision_score(yy, ss)),
            "recall_at_rules_precision": recall_at(yy, ss, prf(*counts(yy, rr))[0]),
            "f1_minus_rules_95": [low, high],
        }
    print(json.dumps(result, indent=1))
    (out / "keys-final.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    model.booster_.save_model(str(out / "keys-model.txt"))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "dev":
        dev(Path(sys.argv[2]))
    else:
        final(Path(sys.argv[2]), sys.argv[3], sys.argv[4])
