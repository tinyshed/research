"""Round 1r: targets through any unique column, and decimal keys as a kind
of their own, two switches on top of round 1p's `both`; labels that name
the referred column.

    .venv/Scripts/python -I targets.py referred <corpus> <round 1b out>
    .venv/Scripts/python -I targets.py dev <round 1b out> <round 1p variants> <out>
    .venv/Scripts/python -I targets.py final <round 1b out> <out> <variant> <round 1p variants> <round 1b models>
    .venv/Scripts/python -I targets.py external <variants> <variant> <round 1p out>

`referred` writes referred.jsonl beside round 1b's hidden.jsonl: the column
each one-column key of SchemaPile-Perm refers to, where SchemaPile records
one. `dev` fits every variant as widen.py's `fit` fits `both` and measures
each, and round 1p's saved `both`, on the validation split only. `final`
measures the chosen one once on the test split and the hard cases;
`external` scores a variant on round 1p's databases with external.py.
"""

import gzip
import json
import sys
import time
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1b"))
sys.path.insert(0, str(HERE.parent / "round-1p"))
sys.path.insert(0, str(HERE))

import lightgbm as lgb  # noqa: E402
import numpy as np  # noqa: E402
import scipy.sparse as sp  # noqa: E402
from sklearn.feature_extraction import FeatureHasher  # noqa: E402

import graph  # noqa: E402
import keys  # noqa: E402
import widen  # noqa: E402
from train import HASHED, KIND, NEGATIVES, SEED, bootstrap, counts, norm, pair_features, prf, singular  # noqa: E402

VARIANTS = {
    "both, labels": dict(unique=False, decimal=False),
    "both, unique": dict(unique=True, decimal=False),
    "both, decimal": dict(unique=False, decimal=True),
    "both, unique, decimal": dict(unique=True, decimal=True),
}
KEY = widen.KEY  # where the target's key column sits in a pair's meta
LABELLED = KEY + 1  # and whether the pair's label named its referred column


def kind_of(column: dict, decimal: bool):
    return KIND.get(column["family"]) or ("decimal" if decimal and column["family"] == "decimal" else None)


def keys_of(t: dict, unique: bool, decimal: bool) -> list[tuple[dict, bool, bool]]:
    """A target's key columns: (column, a pseudo key, not the table's key)."""
    out = []
    if len(t["pk"]) == 1:
        c = next((c for c in t["columns"] if c["name"] == t["pk"][0]), None)
        if c and kind_of(c, decimal):
            out.append((c, False, False))
    elif not t["pk"]:
        candidates = [c for c in t["columns"] if c["unique"] and kind_of(c, decimal)]
        if len(candidates) == 1:
            out.append((candidates[0], True, False))
        else:
            g = norm(t["name"])
            names = {"id", g + "id", singular(g) + "id"}
            named = [c for c in t["columns"] if norm(c["name"]) in names and kind_of(c, decimal)]
            if named:
                out.append((named[0], True, False))
    if unique:
        taken = {c["name"] for c, _, _ in out}
        for c in t["columns"]:
            if c["unique"] and not c["primary"] and c["name"] not in taken and kind_of(c, decimal):
                out.append((c, False, True))
    return out


def pairs(schemas, split, guesses, referred, unique, decimal):
    """widen.pairs with both switches on and every target key a target of its
    own; a pair is a key when its column is declared to point at its table
    and at its key: the referred column where one is recorded, the table's
    primary or pseudo key where none is. With `referred` None, as on external
    databases and the hard cases, the label names the table only."""
    for s in schemas:
        if s["split"] != split or not s["fk"]:
            continue
        declared = {(a, c): b for a, c, b in s["fk"]}
        targets = []
        for t in s["tables"]:
            for key, is_pseudo, alternate in keys_of(t, unique, decimal):
                g = norm(t["name"])
                targets.append({
                    "table": t, "view": {**t, "pk": [key["name"]]} if is_pseudo or alternate else t,
                    "kind": kind_of(key, decimal), "name": g, "singular": singular(g),
                    "grams": keys.grams(g) | keys.grams(singular(g)), "pk": norm(key["name"]),
                    "pk_type": key.get("type"), "key": key["name"], "pseudo": is_pseudo, "alternate": alternate,
                })
        stems = [keys.stem_of(c["name"])[0] for t in s["tables"] for c in t["columns"]]
        named_by = {id(g): sum(1 for st in stems if st and st in (g["name"], g["singular"])) for g in targets}
        all_tokens = [keys.tokens(c["name"]) for t in s["tables"] for c in t["columns"]]
        id_convention = sum(1 for tk in all_tokens if tk and tk[-1] == "id") / max(len(all_tokens), 1)

        for table in s["tables"]:
            for column in table["columns"]:
                kind = kind_of(column, decimal)
                if not kind:
                    continue
                own = column["primary"] and len(table["pk"]) == 1
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
                points = referred.get((s["id"], table["name"], column["name"])) if referred is not None else None
                for g, b in zip(mine, base):
                    target = g["table"]
                    label = declared.get((table["name"], column["name"])) == target["name"]
                    named = points is not None
                    if label and named:
                        label = norm(points) == g["pk"]
                    elif label and referred is not None:
                        label = not g["alternate"]
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
                    f["v3 own key"] = float(own)
                    f["v3 pseudo target"] = float(g["pseudo"])
                    if unique:
                        name, k = norm(column["name"]), g["pk"]
                        f["v5 alternate key"] = float(g["alternate"])
                        f["v5 key inside"] = float(k != "id" and len(k) >= 3 and k in name)
                        f["v5 ends with the key"] = float(len(k) >= 3 and name.endswith(k) and name != k)
                    if decimal:
                        f["v5 decimal"] = float(kind == "decimal")
                    yield s, table, column, target, label, guess == target["name"], f, g["key"], named


def build(job):
    schemas, split, guesses, referred, variant = job
    hasher = FeatureHasher(n_features=HASHED, input_type="dict", alternate_sign=False)
    feats, meta = [], []
    for s, t, c, g, label, rule, f, key, named in pairs(schemas, split, guesses, referred, **VARIANTS[variant]):
        feats.append(f)
        meta.append((s["id"], s["repo"], bool(s.get("novel")), t["name"], c["name"], g["name"], g is t, label, rule,
                     f.get("v2 base", 0.0), f.get("v2 same type", 0.0), f.get("v2 named as the key", 0.0),
                     f.get("ends in id", 0.0), f.get("v2 bare", 0.0), key, named))
    x = hasher.transform(feats).astype(np.float32) if feats else sp.csr_matrix((0, HASHED), dtype=np.float32)
    return x, meta


def build_all(pool, schemas, split, guesses, referred, variant):
    chosen = [s for s in schemas if s["split"] == split and s["fk"]]
    by_schema, points = defaultdict(dict), defaultdict(dict)
    for k, v in guesses.items():
        by_schema[k[0]][k] = v
    for k, v in (referred or {}).items():
        points[k[0]][k] = v
    n = graph.WORKERS * 4
    jobs = [(c, split, {k: v for s in c for k, v in by_schema[s["id"]].items()},
             None if referred is None else {k: v for s in c for k, v in points[s["id"]].items()}, variant)
            for c in (chosen[i::n] for i in range(n)) if c]
    xs, meta = [], []
    for x, m in pool.map(build, jobs):
        xs.append(x)
        meta.extend(m)
    return sp.vstack(xs).tocsr(), meta


def load_referred(data: Path) -> dict:
    out = {}
    for line in open(data / "referred.jsonl", encoding="utf-8"):
        sid, t, c, r = json.loads(line)
        out[(sid, t, c)] = r
    return out


def referred_columns(corpus: Path, data: Path) -> None:
    """The referred column of every one-column key round 1b kept."""
    raw = json.load(gzip.open(corpus / "schemapile-perm/12682521/schemapile-perm.json.gz", "rt", encoding="utf-8"))
    schemas = [json.loads(line) for line in open(data / "hidden.jsonl", encoding="utf-8")]
    seen = Counter()
    fields = Counter()
    with open(data / "referred.jsonl", "w", encoding="utf-8") as out:
        for s in schemas:
            tables = raw[s["id"]]["TABLES"]
            pks = {t["name"]: t["pk"] for t in s["tables"]}
            for tname, t in tables.items():
                for f in t["FOREIGN_KEYS"] or []:
                    fields.update(f.keys())
                    if len(f["COLUMNS"]) != 1:
                        continue
                    target = next((n for n in pks if n.lower() == (f["FOREIGN_TABLE"] or "").lower()), None)
                    if target is None:
                        continue
                    r = f.get("REFERRED_COLUMNS") or []
                    if len(r) != 1:
                        seen["no referred column recorded"] += 1
                        continue
                    pk = pks[target]
                    pk, r = [c.lower() for c in pk], [c.lower() for c in r]
                    seen["refers to the target's one-column primary key" if pk == r else
                         "refers to another column" if len(pk) == 1 else
                         "the target's key spans several columns" if pk else "the target declares no primary key"] += 1
                    seen[f"{s['split']}: refers to another column than a one-column key"] += len(pk) == 1 and pk != r
                    out.write(json.dumps([s["id"], tname, f["COLUMNS"][0], f["REFERRED_COLUMNS"][0]],
                                         ensure_ascii=False) + "\n")
    summary = {"foreign key fields": dict(fields), "keys": dict(seen)}
    (data / "referred-summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, indent=1))


def every_key(schemas, split, meta, kept, threshold) -> dict:
    """F1 over every one-column key the split's schemas declare, a key right
    when its column and its table are."""
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["split"] == split and s["fk"]}
    preds = defaultdict(set)
    for i in np.flatnonzero(kept >= threshold):
        m = meta[i]
        preds[m[graph.SID]].add((m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]))
    tp = sum(len(truth[i] & preds[i]) for i in truth)
    fp = sum(len(preds[i] - truth.get(i, set())) for i in preds)
    fn = sum(len(truth[i] - preds[i]) for i in truth)
    return dict(zip(("precision", "recall", "f1"), prf(tp, fp, fn)), keys=sum(len(v) for v in truth.values()))


def forced_choice(schemas, split, meta, scores, referred) -> dict:
    """The paper's task on a split of SchemaPile-Perm: every pair of tables
    with exactly one declared key from the first to the second, right when
    the best-scored candidate between them has the declared column and its
    referred column (the target's primary key where none is recorded)."""
    rows = defaultdict(list)
    for i, m in enumerate(meta):
        rows[(m[graph.SID], m[graph.TABLE], m[graph.TARGET])].append(i)
    hits = total = covered = 0
    for s in schemas:
        if s["split"] != split or not s["fk"]:
            continue
        pks = {t["name"]: t["pk"] for t in s["tables"]}
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
            primary = pks[b][0] if len(pks[b]) == 1 else None
            want = referred.get((s["id"], a, c)) or primary
            key = meta[best][KEY] if len(meta[best]) > KEY else primary
            hits += meta[best][graph.COLUMN] == c and want is not None and key is not None \
                and norm(key) == norm(want)
    return {"pairs": total, "covered": covered / max(total, 1), "recall_at_1": hits / max(total, 1)}


def measure(name, schemas, meta_v, sv, threshold, referred) -> dict:
    yv = np.array([m[graph.LABEL] for m in meta_v], dtype=bool)
    kept = keys.one_target(sv, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta_v])
    p, r, f = keys.at_threshold(yv, kept, threshold)
    return {"variant": name, "threshold": threshold, "validation pairs": len(meta_v),
            "validation keys among candidates": int(yv.sum()),
            "validation, over candidates": {"precision": p, "recall": r, "f1": f},
            "validation, over every declared key": every_key(schemas, "validation", meta_v, kept, threshold),
            "validation, the paper's task": forced_choice(schemas, "validation", meta_v, sv, referred)}


def fit(pool, schemas, guesses, referred, variant):
    """widen.py's fit: five folds by repository for the first stage's scores
    on training, the second stage on them, the threshold the validation
    split's best F1 over candidates."""
    t0 = time.time()
    clock = lambda what: print(f"{variant}: {what}: {time.time() - t0:.0f} s", flush=True)  # noqa: E731
    x, meta = build_all(pool, schemas, "train", guesses, referred, variant)
    xv, meta_v = build_all(pool, schemas, "validation", guesses, referred, variant)
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
    result = measure(variant, schemas, meta_v, sv, threshold, referred)
    result.update({"training pairs": len(meta), "training keys": int(y.sum()), "seconds": round(time.time() - t0)})
    return result, whole, second


def chosen(results: dict) -> str:
    """The rule written before any variant was fitted."""
    base = results["both"]
    f1 = lambda r: r["validation, over every declared key"]["f1"]  # noqa: E731
    r1 = lambda r: r["validation, the paper's task"]["recall_at_1"]  # noqa: E731
    ok = [v for v in VARIANTS if f1(results[v]) >= f1(base) - 0.005 and r1(results[v]) > r1(base)]
    return max(ok, key=lambda v: r1(results[v])) if ok else "both"


def dev(data: Path, round_1p: Path, out: Path) -> None:
    schemas, guesses = keys.load(data)
    referred = load_referred(data)
    out.mkdir(parents=True, exist_ok=True)
    results = {}
    with ProcessPoolExecutor(graph.WORKERS) as pool:
        # Round 1p's `both` as saved, its pairs as widen.py builds them
        first, second = widen.load_models(round_1p / "both")
        xv, meta_v = widen.build_all(pool, schemas, "validation", guesses, "both")
        sv = second.predict(graph.graph_features(meta_v, first.predict_proba(xv)[:, 1]))
        threshold = json.load(open(round_1p / "both" / "graph-final.json", encoding="utf-8"))["threshold"]
        results["both"] = measure("both", schemas, meta_v, sv, threshold, referred)
        print(json.dumps(results["both"]), flush=True)
        del xv
        for variant in VARIANTS:
            result, whole, second = fit(pool, schemas, guesses, referred, variant)
            d = out / variant.replace(", ", "-").replace(" ", "-")
            d.mkdir(exist_ok=True)
            whole.save_model(str(d / "keys-first.ubj"))
            second.booster_.save_model(str(d / "keys-second.txt"))
            (d / "graph-final.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
            results[variant] = result
            print(json.dumps(result), flush=True)
    results["chosen by the rule"] = chosen(results)
    (out / "dev.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    print("chosen:", results["chosen by the rule"])


def baselines(pool, schemas, guesses, referred, round_1p: Path, base: Path) -> dict:
    """Round 1b's model and round 1p's `both` as saved, on the same test
    split: F1 over every declared key and the paper's task."""
    out = {}
    for name, models, make in (("base", base, lambda: graph.build_all(pool, schemas, "test", guesses)),
                               ("both", round_1p / "both",
                                lambda: widen.build_all(pool, schemas, "test", guesses, "both"))):
        first, second = widen.load_models(models)
        threshold = json.load(open(models / "graph-final.json", encoding="utf-8"))["threshold"]
        x, meta = make()
        raw = second.predict(graph.graph_features(meta, first.predict_proba(x)[:, 1]))
        kept = keys.one_target(raw, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
        y = np.array([m[graph.LABEL] for m in meta], dtype=bool)
        out[name] = {"threshold": threshold,
                     "over candidates": dict(zip(("precision", "recall", "f1"), keys.at_threshold(y, kept, threshold))),
                     "over every declared key": every_key(schemas, "test", meta, kept, threshold),
                     "the paper's task": forced_choice(schemas, "test", meta, raw, referred)}
        print(name, json.dumps(out[name]), flush=True)
    return out


def final(data: Path, out: Path, variant: str, round_1p: Path, base: Path) -> None:
    """The chosen variant measured once on the test split and the hard cases,
    and beside it round 1b's model and round 1p's `both` on the same test;
    with `both` chosen, the two beside it alone."""
    if variant == "both":
        schemas, guesses = keys.load(data)
        with ProcessPoolExecutor(graph.WORKERS) as pool:
            result = {"test": baselines(pool, schemas, guesses, load_referred(data), round_1p, base)}
        out.mkdir(parents=True, exist_ok=True)
        (out / "both-final.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        return
    d = out / variant.replace(", ", "-").replace(" ", "-")
    first, second = widen.load_models(d)
    result = json.load(open(d / "graph-final.json", encoding="utf-8"))
    threshold = result["threshold"]
    schemas, guesses = keys.load(data)
    referred = load_referred(data)
    hard = [json.loads(line) for line in open(data / "hard-hidden.jsonl", encoding="utf-8")]
    hard_guesses = {}
    for line in open(data / "hard-guesses.jsonl", encoding="utf-8"):
        g = json.loads(line)
        hard_guesses[(g["schema"], g["table"], g["column"])] = g
    rng = np.random.default_rng(SEED)
    with ProcessPoolExecutor(graph.WORKERS) as pool:
        result["test, beside it"] = baselines(pool, schemas, guesses, referred, round_1p, base)
        xt, meta_t = build_all(pool, schemas, "test", guesses, referred, variant)
        xh, meta_h = build_all(pool, hard, "hard", hard_guesses, None, variant)

    def predict(x, m):
        raw = second.predict(graph.graph_features(m, first.predict_proba(x)[:, 1]))
        return raw, keys.one_target(raw, [(r[graph.SID], r[graph.TABLE], r[graph.COLUMN]) for r in m])

    raw_t, st = predict(xt, meta_t)
    yt = np.array([m[graph.LABEL] for m in meta_t], dtype=bool)
    rules_t = np.array([m[graph.RULES] for m in meta_t], dtype=bool)
    repos_t = np.array([m[graph.REPO] for m in meta_t])
    predicted = st >= threshold
    test = {}
    subsets = {"all": np.ones_like(yt), "novel": np.array([m[graph.NOVEL] for m in meta_t], dtype=bool)}
    for name, mask in subsets.items():
        low, high = bootstrap(yt[mask], predicted[mask], rules_t[mask], repos_t[mask], rng)
        test[name] = {"pairs": int(mask.sum()), "keys": int(yt[mask].sum()),
                      "rules": dict(zip(("precision", "recall", "f1"), prf(*counts(yt[mask], rules_t[mask])))),
                      "model": dict(zip(("precision", "recall", "f1"), prf(*counts(yt[mask], predicted[mask])))),
                      "f1_minus_rules_95": [low, high]}
    _, sh = predict(xh, meta_h)
    yh = np.array([m[graph.LABEL] for m in meta_h], dtype=bool)
    rules_h = np.array([m[graph.RULES] for m in meta_h], dtype=bool)
    result["test"] = test
    result["test, over every declared key"] = every_key(schemas, "test", meta_t, st, threshold)
    result["test, the paper's task"] = forced_choice(schemas, "test", meta_t, raw_t, referred)
    result["hard"] = {"keys": int(yh.sum()),
                      "rules": dict(zip(("precision", "recall", "f1"), prf(*counts(yh, rules_h)))),
                      "model": dict(zip(("precision", "recall", "f1"), prf(*counts(yh, sh >= threshold))))}
    (d / "final.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


def external(variants: Path, variant: str, round_1p: Path) -> None:
    """external.py's measures, and F1 over every declared key from the keys
    it draws, as round 1p's gemini.py counts them."""
    import external as ext  # noqa: E402
    import gemini  # noqa: E402
    stem = variant.replace(", ", "-").replace(" ", "-")
    d = variants / stem
    suffix = "-1r-" + stem
    if variant in VARIANTS:
        job = lambda j: build((*j, None, variant))  # noqa: E731
    else:
        job = lambda j: widen.build((*j, variant))  # noqa: E731
    ext.score(d, round_1p, build=job, suffix=suffix)
    schemas = [json.loads(line) for line in open(round_1p / "hidden.jsonl", encoding="utf-8")]
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas}
    preds = defaultdict(set)
    for line in open(round_1p / f"open-predictions{suffix}.jsonl", encoding="utf-8"):
        sid, t, c, r, _ = json.loads(line)
        preds[sid].add((t, c, r))
    result = {}
    for name in ("spider", "bird", "ctu", "all"):
        ids = [s["id"] for s in schemas if name in ("all", s["source"]) and s["fk"]]
        result[name] = dict(zip(("precision", "recall", "f1"), gemini.prf(*gemini.open_counts(truth, preds, ids))),
                            databases=len(ids))
    (round_1p / f"every-key{suffix}.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    a = sys.argv
    if a[1] == "referred":
        referred_columns(Path(a[2]), Path(a[3]) if len(a) > 3 else Path(a[2]) / "round-1b")
    elif a[1] == "dev":
        dev(Path(a[2]), Path(a[3]), Path(a[4]))
    elif a[1] == "final":
        final(Path(a[2]), Path(a[3]), a[4], Path(a[5]), Path(a[6]))
    else:
        external(Path(a[2]), a[3], Path(a[4]))
