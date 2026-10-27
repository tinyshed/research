"""Round 1b's models and measures: logistic regression and LightGBM against
round 1's rules, on SchemaPile-Perm with its declarations hidden.

    .venv/Scripts/python -I train.py <out>

reads hidden.jsonl and guesses.jsonl from <out>, as prepare.py and the
rules' `guess` wrote them, and writes results.json there.
"""

import json
import random
import sys
import time
from pathlib import Path

import lightgbm as lgb
import numpy as np
from sklearn.feature_extraction import FeatureHasher
from sklearn.linear_model import LogisticRegression
from sklearn.metrics import average_precision_score, precision_recall_curve

SEED = 20261027
HASHED = 2 ** 15
NEGATIVES = 0.1  # the share of negative pairs kept for training; every positive is kept
CHUNK = 200_000
KIND = {"int": "int", "uuid": "uuid", "text": "text"}


def tokens(name: str) -> list[str]:
    """As round 1's names.rs splits a name."""
    out, current = [], ""
    for i, c in enumerate(name):
        if not c.isalpha():
            if current:
                out.append(current)
                current = ""
            continue
        if c.isupper() and current:
            previous = name[i - 1]
            next_lower = i + 1 < len(name) and name[i + 1].islower()
            if previous.islower() or (previous.isupper() and next_lower):
                out.append(current)
                current = ""
        current += c.lower()
    if current:
        out.append(current)
    return out


def norm(name: str) -> str:
    return "".join(c.lower() for c in name if c.isalnum())


def singular(word: str) -> str:
    if word.endswith("ies"):
        return word[:-3] + "y"
    if word.endswith(("ses", "xes")):
        return word[:-2]
    if word.endswith("s"):
        return word[:-1]
    return word


def column_features(schema, table, column, with_index, neighbours):
    tk = tokens(column["name"])
    f = {f"c:{t}": 1.0 for t in tk}
    if tk:
        f[f"last:{tk[-1]}"] = 1.0
        f[f"first:{tk[0]}"] = 1.0
    for t in tokens(table["name"]):
        f[f"t:{t}"] = 1.0
    if neighbours:
        for other in table["columns"][:20]:
            if other is not column:
                for t in tokens(other["name"])[:3]:
                    f[f"n:{t}"] = 1.0
    f[f"family:{column['family']}"] = 1.0
    f[f"nullable:{column['nullable']}"] = 1.0
    if column["unique"]:
        f["unique"] = 1.0
    if column["primary"]:
        f["pk_single" if len(table["pk"]) == 1 else "pk_part"] = 1.0
    if with_index and column["index"]:
        f["index"] = 1.0
    f["tokens"] = min(len(tk), 6) / 6
    f["position"] = min(column["position"], 30) / 30
    f["columns"] = min(len(table["columns"]), 60) / 60
    f["tables"] = min(len(schema["tables"]), 100) / 100
    return f


def pair_features(schema, table, column, target, with_index):
    f = column_features(schema, table, column, with_index, neighbours=False)
    tk = tokens(column["name"])
    stem_tokens = tk[:-1] if tk and tk[-1] == "id" else tk
    stem = "".join(stem_tokens)
    gt = tokens(target["name"])
    g = norm(target["name"])
    for t in gt:
        f[f"g:{t}"] = 1.0
    for a in stem_tokens[-2:]:
        for b in gt[-2:]:
            f[f"x:{a}|{b}"] = 1.0
    f["stem=target"] = float(stem == g)
    f["stem=target singular"] = float(stem == singular(g))
    f["stem in target"] = float(bool(stem) and stem in g)
    f["target in stem"] = float(bool(singular(g)) and singular(g) in stem)
    f["named as target's key"] = float(norm(column["name"]) == norm(target["pk"][0]))
    f["shared tokens"] = len((set(stem_tokens) & set(gt)) - {"id"}) / 3
    f["ends in id"] = float(bool(tk) and tk[-1] == "id")
    f["self"] = float(target is table)
    f["target columns"] = min(len(target["columns"]), 60) / 60
    return f


def key_kind(table):
    if len(table["pk"]) != 1:
        return None
    column = next((c for c in table["columns"] if c["name"] == table["pk"][0]), None)
    return KIND.get(column["family"]) if column else None


def fk_rows(schemas, split, guesses, sample=None, rng=None):
    """(schema, table, column, target, label, rules) for every candidate pair."""
    for s in schemas:
        if s["split"] != split or not s["fk"]:
            continue
        declared = {(a, c): b for a, c, b in s["fk"]}
        targets = [(t, key_kind(t)) for t in s["tables"]]
        targets = [(t, k) for t, k in targets if k]
        for table in s["tables"]:
            for column in table["columns"]:
                kind = KIND.get(column["family"])
                if not kind or (column["primary"] and len(table["pk"]) == 1):
                    continue
                guess = guesses.get((s["id"], table["name"], column["name"]), {}).get("fk")
                for target, k in targets:
                    if k != kind or (target is table and column["name"] == target["pk"][0]):
                        continue
                    label = declared.get((table["name"], column["name"])) == target["name"]
                    if sample is not None and not label and rng.random() > sample:
                        continue
                    yield s, table, column, target, label, guess == target["name"]


def closed_rows(schemas, split, guesses):
    """(schema, table, column, None, label, rules) for every text or integer column."""
    for s in schemas:
        if s["split"] != split:
            continue
        closed = {(a, c) for a, c in s["closed"]}
        keys = {(a, c) for a, c, _ in s["fk"]}
        for table in s["tables"]:
            for column in table["columns"]:
                if column["family"] not in ("int", "text") or column["primary"] or (table["name"], column["name"]) in keys:
                    continue
                guess = guesses.get((s["id"], table["name"], column["name"]), {})
                yield s, table, column, None, (table["name"], column["name"]) in closed, bool(guess.get("category_named"))


def featurize(hasher, rows, task, with_index):
    if task == "fk":
        dicts = [pair_features(s, t, c, g, with_index) for s, t, c, g, _, _ in rows]
    else:
        dicts = [column_features(s, t, c, with_index, neighbours=True) for s, t, c, _, _, _ in rows]
    return hasher.transform(dicts).astype(np.float32)


def collect(models, hasher, generator, task, with_index):
    """Scores of every model, labels, the rules' guesses and repositories, chunk by chunk."""
    y, rules, repos, novel = [], [], [], []
    scores = {name: [] for name in models}
    chunk = []

    def flush():
        if not chunk:
            return
        x = featurize(hasher, chunk, task, with_index)
        for name, model in models.items():
            scores[name].append(model.predict_proba(x)[:, 1])
        y.extend(r[4] for r in chunk)
        rules.extend(r[5] for r in chunk)
        repos.extend(r[0]["repo"] for r in chunk)
        novel.extend(bool(r[0].get("novel")) for r in chunk)
        chunk.clear()

    for row in generator:
        chunk.append(row)
        if len(chunk) >= CHUNK:
            flush()
    flush()
    scores = {k: np.concatenate(v) if v else np.array([]) for k, v in scores.items()}
    return np.array(y), np.array(rules), np.array(repos), np.array(novel, dtype=bool), scores


def prf(tp, fp, fn):
    p = tp / (tp + fp) if tp + fp else 0.0
    r = tp / (tp + fn) if tp + fn else 0.0
    return p, r, (2 * p * r / (p + r) if p + r else 0.0)


def counts(y, predicted):
    return int((predicted & y).sum()), int((predicted & ~y).sum()), int((~predicted & y).sum())


def bootstrap(y, a, b, repos, rng, n=1000):
    """The 95 % interval of F1(a) − F1(b), resampling repositories."""
    names, index = np.unique(repos, return_inverse=True)
    per = np.zeros((len(names), 6))
    for col, (pred) in enumerate((a, b)):
        np.add.at(per[:, 3 * col + 0], index, (pred & y))
        np.add.at(per[:, 3 * col + 1], index, (pred & ~y))
        np.add.at(per[:, 3 * col + 2], index, (~pred & y))
    diffs = []
    for _ in range(n):
        s = per[rng.integers(0, len(names), len(names))].sum(axis=0)
        diffs.append(prf(*s[0:3])[2] - prf(*s[3:6])[2])
    return float(np.percentile(diffs, 2.5)), float(np.percentile(diffs, 97.5))


def model_size(name, model):
    if name == "logistic":
        return int(np.count_nonzero(model.coef_)) * 4 + 4
    return len(model.booster_.model_to_string().encode())


def run(schemas, guesses, task, with_index):
    rng = random.Random(SEED)
    hasher = FeatureHasher(n_features=HASHED, input_type="dict", alternate_sign=False)
    if task == "fk":
        train = list(fk_rows(schemas, "train", guesses, sample=NEGATIVES, rng=rng))
        rows = lambda split: fk_rows(schemas, split, guesses)
    else:
        train = list(closed_rows(schemas, "train", guesses))
        rows = lambda split: closed_rows(schemas, split, guesses)
    x = featurize(hasher, train, task, with_index)
    y = np.array([r[4] for r in train])
    started = time.time()
    models = {
        "logistic": LogisticRegression(solver="liblinear", C=1.0, max_iter=300),
        "trees": lgb.LGBMClassifier(
            n_estimators=200, num_leaves=15, learning_rate=0.1, subsample=0.8, subsample_freq=1,
            colsample_bytree=0.5, min_child_samples=20, random_state=SEED, verbose=-1,
        ),
    }
    fit_seconds = {}
    for name, model in models.items():
        t0 = time.time()
        model.fit(x, y)
        fit_seconds[name] = round(time.time() - t0, 1)
    del x

    vy, _, _, _, vscores = collect(models, hasher, rows("validation"), task, with_index)
    ty, trules, trepos, tnovel, tscores = collect(models, hasher, rows("test"), task, with_index)
    ty = ty.astype(bool)
    trules = trules.astype(bool)
    nprng = np.random.default_rng(SEED)

    rtp, rfp, rfn = counts(ty, trules)
    result = {
        "task": task, "index_seen": with_index,
        "train_rows": len(train), "train_positives": int(y.sum()),
        "validation_rows": int(len(vy)), "test_rows": int(len(ty)), "test_positives": int(ty.sum()),
        "rules": dict(zip(("precision", "recall", "f1"), prf(rtp, rfp, rfn))),
        "fit_seconds": fit_seconds, "models": {},
    }
    rp, rr, _ = prf(rtp, rfp, rfn)
    # The same measures on the test schemas no training schema resembles
    ny, nrules, nrepos = ty[tnovel], trules[tnovel], trepos[tnovel]
    result["novel"] = {
        "test_rows": int(len(ny)), "test_positives": int(ny.sum()),
        "rules": dict(zip(("precision", "recall", "f1"), prf(*counts(ny, nrules)))), "models": {},
    }
    for name, model in models.items():
        p_curve, r_curve, thresholds = precision_recall_curve(vy, vscores[name])
        f1 = 2 * p_curve * r_curve / np.maximum(p_curve + r_curve, 1e-12)
        best = int(np.argmax(f1[:-1]))
        threshold = float(thresholds[best])
        predicted = tscores[name] >= threshold
        tp, fp, fn = counts(ty, predicted)
        p, r, f = prf(tp, fp, fn)
        tp_curve, tr_curve, _ = precision_recall_curve(ty, tscores[name])
        # the model's best precision where it recalls as much as the rules, and
        # its best recall where it is as precise
        precision_at_rules_recall = float(tp_curve[tr_curve >= rr].max()) if (tr_curve >= rr).any() else 0.0
        recall_at_rules_precision = float(tr_curve[tp_curve >= rp].max()) if (tp_curve >= rp).any() else 0.0
        low, high = bootstrap(ty, predicted, trules, trepos, nprng)
        result["models"][name] = {
            "threshold": threshold, "precision": p, "recall": r, "f1": f,
            "average_precision": float(average_precision_score(ty, tscores[name])),
            "precision_at_rules_recall": precision_at_rules_recall,
            "recall_at_rules_precision": recall_at_rules_precision,
            "f1_minus_rules_95": [low, high],
            "weights_kib": round(model_size(name, model) / 1024, 1),
        }
        npred = predicted[tnovel]
        nlow, nhigh = bootstrap(ny, npred, nrules, nrepos, nprng)
        result["novel"]["models"][name] = {
            **dict(zip(("precision", "recall", "f1"), prf(*counts(ny, npred)))),
            "f1_minus_rules_95": [nlow, nhigh],
        }
    return result


def main(out: Path) -> None:
    schemas = [json.loads(line) for line in open(out / "hidden.jsonl", encoding="utf-8")]
    guesses = {}
    for line in open(out / "guesses.jsonl", encoding="utf-8"):
        g = json.loads(line)
        guesses[(g["schema"], g["table"], g["column"])] = g
    results = []
    for task in ("fk", "closed"):
        for with_index in (True, False):
            r = run(schemas, guesses, task, with_index)
            results.append(r)
            print(json.dumps(r, indent=1), flush=True)
    (out / "results.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
