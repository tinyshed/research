"""Round 1p's widened candidates. Round 1b pairs a column only with tables
whose primary key is one column, and never pairs a table's own one-column
key: the paper's pairs that no candidate can answer are mostly a key that is
also its table's key (a row extending another, one to one) and a target that
declares no primary key. Two widenings, each a switch:

- `own key`: a table's one-column key may point at another table;
- `pseudo key`: a table declaring no primary key is a target through its one
  unique column, or else a column named `id` or after the table and `id`.

    .venv/Scripts/python -I widen.py dev <round 1b out> <variants out>
    .venv/Scripts/python -I widen.py final <round 1b out> <variants out> <variant>
    .venv/Scripts/python -I widen.py external <variants out> <variant> <round 1p out>

`dev` fits every variant on round 1b's training split as graph.py's final
fits round 1b's, and measures each on the validation split only: F1 one
target a column, and the paper's task built on validation schemas — two
tables with one declared key between them, the column asked. `final`
measures the chosen variant once on the test split and the hard cases.
`external` scores it on round 1p's databases with external.py's measures.
"""

import json
import sys
import time
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

ROUND_1B = Path(__file__).resolve().parent.parent / "round-1b"
sys.path.insert(0, str(ROUND_1B))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import lightgbm as lgb  # noqa: E402
import numpy as np  # noqa: E402
import scipy.sparse as sp  # noqa: E402
from sklearn.feature_extraction import FeatureHasher  # noqa: E402
from sklearn.metrics import average_precision_score  # noqa: E402

import graph  # noqa: E402
import keys  # noqa: E402
from train import HASHED, KIND, NEGATIVES, SEED, bootstrap, counts, norm, pair_features, prf, singular  # noqa: E402

VARIANTS = {
    "reported": dict(own_key=False, pseudo=False),
    "own key": dict(own_key=True, pseudo=False),
    "pseudo key": dict(own_key=False, pseudo=True),
    "both": dict(own_key=True, pseudo=True),
    # round 1q: features for keys named by the role a row plays
    "both, roles": dict(own_key=True, pseudo=True, roles=True),
}
KEY = 14  # where the target's key column sits in a pair's meta, after graph.py's fields


def key_of(t, pseudo):
    """A target's key column and whether it is a pseudo key, or None."""
    if len(t["pk"]) == 1:
        c = next((c for c in t["columns"] if c["name"] == t["pk"][0]), None)
        return (c, False) if c and KIND.get(c["family"]) else (None, False)
    if not pseudo or t["pk"]:
        return None, False
    unique = [c for c in t["columns"] if c["unique"] and KIND.get(c["family"])]
    if len(unique) == 1:
        return unique[0], True
    g = norm(t["name"])
    names = {"id", g + "id", singular(g) + "id"}
    named = [c for c in t["columns"] if norm(c["name"]) in names and KIND.get(c["family"])]
    return (named[0], True) if named else (None, False)


def pairs(schemas, split, guesses, own_key, pseudo, roles=False):
    """keys.pairs at version 2, its candidates widened by the two switches;
    with both off it yields what keys.pairs yields. `roles` adds round 1q's
    features for a key named by a role: its target's key name inside it."""
    for s in schemas:
        if s["split"] != split or not s["fk"]:
            continue
        declared = {(a, c): b for a, c, b in s["fk"]}
        targets = []
        for t in s["tables"]:
            key, is_pseudo = key_of(t, pseudo)
            if key is None:
                continue
            g = norm(t["name"])
            targets.append({
                "table": t, "view": {**t, "pk": [key["name"]]} if is_pseudo else t, "kind": KIND[key["family"]],
                "name": g, "singular": singular(g), "grams": keys.grams(g) | keys.grams(singular(g)),
                "pk": norm(key["name"]), "pk_type": key.get("type"), "key": key["name"], "pseudo": is_pseudo,
            })
        stems = [keys.stem_of(c["name"])[0] for t in s["tables"] for c in t["columns"]]
        named_by = {id(g): sum(1 for st in stems if st and st in (g["name"], g["singular"])) for g in targets}
        all_tokens = [keys.tokens(c["name"]) for t in s["tables"] for c in t["columns"]]
        id_convention = sum(1 for tk in all_tokens if tk and tk[-1] == "id") / max(len(all_tokens), 1)
        if roles:
            key_count = {}
            for g in targets:
                key_count[g["pk"]] = key_count.get(g["pk"], 0) + 1
            bare = [norm(c["name"]).rstrip("0123456789") for t in s["tables"] for c in t["columns"]]
            carrying = {k: sum(1 for b in bare if k in b) for k in key_count if k != "id" and len(k) >= 3}

        for table in s["tables"]:
            for column in table["columns"]:
                kind = KIND.get(column["family"])
                own = column["primary"] and len(table["pk"]) == 1
                if not kind or (own and not own_key):
                    continue
                stem, tk = keys.stem_of(column["name"])
                sg = keys.grams(stem) if stem else set()
                mine = [g for g in targets if g["kind"] == kind
                        and not (g["table"] is table and column["name"] == g["key"])]
                base = []
                for g in mine:
                    exact = stem in (g["name"], g["singular"]) or stem + "s" == g["name"]
                    p = keys.prefix(stem, g["singular"])
                    b = max(float(exact), keys.jaccard(sg, g["grams"]), float(norm(column["name"]) == g["pk"]),
                            0.8 * p / max(1, min(len(stem), len(g["singular"]))) if p >= 3 else 0.0)
                    base.append(b)
                best = max(base, default=0.0)
                guess = guesses.get((s["id"], table["name"], column["name"]), {}).get("fk")
                for g, b in zip(mine, base):
                    target = g["table"]
                    label = declared.get((table["name"], column["name"])) == target["name"]
                    f = pair_features(s, table, column, g["view"], with_index=True)
                    f["self"] = float(target is table)
                    others = [x for x, h in zip(base, mine) if h is not g]
                    f["v2 letters"] = keys.jaccard(sg, g["grams"])
                    f["v2 edit"] = keys.similarity(stem, g["singular"])
                    f["v2 prefix"] = min(keys.prefix(stem, g["singular"]), 8) / 8
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
                    if own_key:
                        f["v3 own key"] = float(own)
                    if roles:
                        name = norm(column["name"])
                        less = name.rstrip("0123456789")
                        k = g["pk"]
                        f["v4 less digits is the key"] = float(less == k and less != name)
                        f["v4 key inside"] = float(k != "id" and len(k) >= 3 and k in less)
                        f["v4 ends with the key"] = float(len(k) >= 3 and less.endswith(k) and less != k)
                        f["v4 targets keyed so"] = 1 / key_count[k]
                        f["v4 columns carrying the key"] = min(carrying.get(k, 0), 20) / 20
                    if pseudo:
                        f["v3 pseudo target"] = float(g["pseudo"])
                    yield s, table, column, target, label, guess == target["name"], f, g["key"]


def build(job):
    schemas, split, guesses, variant = job
    switches = VARIANTS[variant]
    hasher = FeatureHasher(n_features=HASHED, input_type="dict", alternate_sign=False)
    feats, meta = [], []
    for s, t, c, g, label, rule, f, key in pairs(schemas, split, guesses, **switches):
        feats.append(f)
        meta.append((s["id"], s["repo"], bool(s.get("novel")), t["name"], c["name"], g["name"], g is t, label, rule,
                     f.get("v2 base", 0.0), f.get("v2 same type", 0.0), f.get("v2 named as the key", 0.0),
                     f.get("ends in id", 0.0), f.get("v2 bare", 0.0), key))
    x = hasher.transform(feats).astype(np.float32) if feats else sp.csr_matrix((0, HASHED), dtype=np.float32)
    return x, meta


def build_all(pool, schemas, split, guesses, variant):
    chosen = [s for s in schemas if s["split"] == split and s["fk"]]
    by_schema = defaultdict(dict)
    for k, v in guesses.items():
        by_schema[k[0]][k] = v
    n = graph.WORKERS * 4
    jobs = [(c, split, {k: v for s in c for k, v in by_schema[s["id"]].items()}, variant)
            for c in (chosen[i::n] for i in range(n)) if c]
    xs, meta = [], []
    for x, m in pool.map(build, jobs):
        xs.append(x)
        meta.extend(m)
    return sp.vstack(xs).tocsr(), meta


def forced_choice(schemas, split, meta, scores):
    """The paper's task on a split of SchemaPile-Perm: every pair of tables
    with exactly one declared key from the first to the second, the column
    the scores put first among the first table's columns paired with the
    second; no candidate is a miss."""
    rows = defaultdict(list)
    for i, m in enumerate(meta):
        rows[(m[graph.SID], m[graph.TABLE], m[graph.TARGET])].append(i)
    hits = total = covered = 0
    for s in schemas:
        if s["split"] != split or not s["fk"]:
            continue
        between = Counter((a, b) for a, _, b in s["fk"])
        for a, c, b in s["fk"]:
            if between[(a, b)] != 1:
                continue
            total += 1
            r = rows.get((s["id"], a, b), [])
            if not r:
                continue
            covered += 1
            best = max(r, key=lambda i: (scores[i], -i))
            hits += meta[best][graph.COLUMN] == c
    return {"pairs": total, "covered": covered / max(total, 1), "recall_at_1": hits / max(total, 1)}


def fit(pool, schemas, guesses, variant):
    """graph.py's final procedure: five folds by repository for the first
    stage's scores on training, the second stage on them, the threshold the
    validation split's best F1."""
    t0 = time.time()
    clock = lambda what: print(f"{variant}: {what}: {time.time() - t0:.0f} s", flush=True)  # noqa: E731
    x, meta = build_all(pool, schemas, "train", guesses, variant)
    xv, meta_v = build_all(pool, schemas, "validation", guesses, variant)
    clock(f"features, {len(meta)} training pairs")
    y = np.array([m[graph.LABEL] for m in meta], dtype=bool)
    folds = np.array([graph.fold_of(m[graph.REPO]) for m in meta])
    sample = y | (np.random.default_rng(SEED).random(len(y)) < NEGATIVES)
    oof = np.zeros(len(y))
    for k in range(graph.FOLDS):
        rows = np.flatnonzero(sample & (folds != k))
        model = graph.first_model(True)
        model.fit(x[rows], y[rows])
        held = np.flatnonzero(folds == k)
        oof[held] = graph.first_scores(model, x[held], True)
    clock("five folds")
    whole = graph.first_model(True)
    rows = np.flatnonzero(sample)
    whole.fit(x[rows], y[rows])
    del x
    second = lgb.LGBMClassifier(**graph.SECOND)
    second.fit(graph.graph_features(meta, oof)[sample], y[sample])
    clock("both stages")
    sv = second.predict_proba(graph.graph_features(meta_v, graph.first_scores(whole, xv, True)))[:, 1]
    yv = np.array([m[graph.LABEL] for m in meta_v], dtype=bool)
    kept = keys.one_target(sv, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta_v])
    threshold = keys.best_threshold(yv, kept)
    novel = np.array([m[graph.NOVEL] for m in meta_v], dtype=bool)
    p, r, f = keys.at_threshold(yv, kept, threshold)
    result = {
        "variant": variant, "training pairs": len(meta), "validation pairs": len(meta_v),
        "validation keys among candidates": int(yv.sum()), "threshold": threshold,
        "validation": {"precision": p, "recall": r, "f1": f,
                       "average_precision": float(average_precision_score(yv, kept)),
                       "novel_f1": keys.at_threshold(yv[novel], kept[novel], threshold)[2]},
        "validation, the paper's task": forced_choice(schemas, "validation", meta_v, sv),
        "seconds": round(time.time() - t0),
    }
    return result, whole, second


def dev(data: Path, out: Path) -> None:
    schemas, guesses = keys.load(data)
    out.mkdir(parents=True, exist_ok=True)
    results = {}
    with ProcessPoolExecutor(graph.WORKERS) as pool:
        for variant in VARIANTS:
            result, whole, second = fit(pool, schemas, guesses, variant)
            d = out / variant.replace(" ", "-")
            d.mkdir(exist_ok=True)
            whole.save_model(str(d / "keys-first.ubj"))
            second.booster_.save_model(str(d / "keys-second.txt"))
            (d / "graph-final.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
            results[variant] = result
            print(json.dumps(result), flush=True)
    (out / "dev.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")


def load_models(d: Path):
    import xgboost as xgb  # noqa: E402
    first = xgb.XGBClassifier()
    first.load_model(str(d / "keys-first.ubj"))
    first.set_params(device="cpu")
    second = lgb.Booster(model_file=str(d / "keys-second.txt"))
    return first, second


def final(data: Path, out: Path, variant: str) -> None:
    """The chosen variant measured once on the test split and the hard cases."""
    d = out / variant.replace(" ", "-")
    first, second = load_models(d)
    result = json.load(open(d / "graph-final.json", encoding="utf-8"))
    threshold = result["threshold"]
    schemas, guesses = keys.load(data)
    hard = [json.loads(line) for line in open(data / "hard-hidden.jsonl", encoding="utf-8")]
    hard_guesses = {}
    for line in open(data / "hard-guesses.jsonl", encoding="utf-8"):
        g = json.loads(line)
        hard_guesses[(g["schema"], g["table"], g["column"])] = g
    rng = np.random.default_rng(SEED)
    with ProcessPoolExecutor(graph.WORKERS) as pool:
        xt, meta_t = build_all(pool, schemas, "test", guesses, variant)
        xh, meta_h = build_all(pool, hard, "hard", hard_guesses, variant)

    def predict(x, m):
        raw = second.predict(graph.graph_features(m, first.predict_proba(x)[:, 1]))
        return raw, keys.one_target(raw, [(r[graph.SID], r[graph.TABLE], r[graph.COLUMN]) for r in m])

    raw_t, st = predict(xt, meta_t)
    yt = np.array([m[graph.LABEL] for m in meta_t], dtype=bool)
    rules_t = np.array([m[graph.RULES] for m in meta_t], dtype=bool)
    repos_t = np.array([m[graph.REPO] for m in meta_t])
    predicted = st >= threshold
    test = {}
    subsets = {"all": np.ones_like(yt), "novel": np.array([m[graph.NOVEL] for m in meta_t], dtype=bool),
               "complete": graph.complete_mask(meta_t)}
    for name, mask in subsets.items():
        rules = prf(*counts(yt[mask], rules_t[mask]))
        low, high = bootstrap(yt[mask], predicted[mask], rules_t[mask], repos_t[mask], rng)
        test[name] = {"pairs": int(mask.sum()), "keys": int(yt[mask].sum()),
                      "rules": dict(zip(("precision", "recall", "f1"), rules)),
                      "model": dict(zip(("precision", "recall", "f1"), prf(*counts(yt[mask], predicted[mask])))),
                      "f1_minus_rules_95": [low, high]}
    _, sh = predict(xh, meta_h)
    yh = np.array([m[graph.LABEL] for m in meta_h], dtype=bool)
    rules_h = np.array([m[graph.RULES] for m in meta_h], dtype=bool)
    result["test"] = test
    result["test, the paper's task"] = forced_choice(schemas, "test", meta_t, raw_t)
    result["hard"] = {"keys": int(yh.sum()),
                      "rules": dict(zip(("precision", "recall", "f1"), prf(*counts(yh, rules_h)))),
                      "model": dict(zip(("precision", "recall", "f1"), prf(*counts(yh, sh >= threshold))))}
    (d / "final.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


def external(out: Path, variant: str, round_1p: Path) -> None:
    import external as ext  # noqa: E402
    d = out / variant.replace(" ", "-")
    ext.score(d, round_1p, build=lambda job: build((*job, variant)), suffix="-" + variant.replace(" ", "-"))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "dev":
        dev(Path(sys.argv[2]), Path(sys.argv[3]))
    elif sys.argv[1] == "final":
        final(Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4])
    else:
        external(Path(sys.argv[2]), sys.argv[3], Path(sys.argv[4]))
