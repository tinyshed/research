"""Round 1b's graph stage for foreign keys: a first model scores every pair
of a schema, features are read off those scores over the whole schema — how
many confident arrows reach a table, whether a column's siblings point the
same way, how far a column's best target leads — and a second model learns
from them. The first model's scores on training schemas come from models
that did not see those schemas, five folds by repository.

    .venv/Scripts/python -I graph.py dev <out> [--small] [--gpu]
    .venv/Scripts/python -I graph.py final <out>

`dev` fits on training and measures on validation only; the test is not
read. `final` takes the configuration dev chose — the first stage on the
graphics card, one target a column — and measures it once on the test and
on the hard cases, saving its two models. Every pair's features are built
once, by one process a core, and the five folds and the whole take their
rows from that one matrix.
"""

import hashlib
import json
import os
import random
import sys
import time
from collections import defaultdict
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import lightgbm as lgb  # noqa: E402
import numpy as np  # noqa: E402
import xgboost as xgb  # noqa: E402
import scipy.sparse as sp  # noqa: E402
from sklearn.feature_extraction import FeatureHasher  # noqa: E402
from sklearn.metrics import average_precision_score  # noqa: E402

import keys  # noqa: E402
from train import HASHED, NEGATIVES, SEED, bootstrap, counts, prf, tokens  # noqa: E402

FOLDS = 5
WORKERS = max(1, (os.cpu_count() or 2) - 1)
FIRST = dict(n_estimators=550, num_leaves=63, learning_rate=0.1, subsample=0.8, subsample_freq=1,
             colsample_bytree=0.5, min_child_samples=20, random_state=SEED, verbose=-1, n_jobs=-1)
SECOND = dict(n_estimators=400, num_leaves=31, learning_rate=0.05, subsample=0.8, subsample_freq=1,
              colsample_bytree=0.8, min_child_samples=50, random_state=SEED, verbose=-1, n_jobs=-1)
# The first stage on the graphics card, as strong as dev found it: 829
# trees was where its validation curve stopped rising (gpu.py)
FIRST_GPU = dict(n_estimators=829, tree_method="hist", device="cuda", grow_policy="lossguide", max_leaves=63,
                 max_depth=0, learning_rate=0.05, subsample=0.8, colsample_bytree=0.5, random_state=SEED)
CONFIDENT = 0.5
GRAPH_FEATURES = [
    "score", "rank in column", "lead over the column's next", "column's best", "confident targets of the column",
    "others into the target, sum", "others into the target, best", "others into the target, confident",
    "siblings into the target, best", "confident siblings", "confident columns of the schema",
    "self", "back from the target", "same last word, mean best", "base", "same type", "named as the key",
    "ends in id", "bare",
]
# What the graph stage keeps of a pair, besides its hashed features
SID, REPO, NOVEL, TABLE, COLUMN, TARGET, SELF, LABEL, RULES, BASE, SAME_TYPE, NAMED_KEY, ENDS_ID, BARE = range(14)


def fold_of(repo: str) -> int:
    return int(hashlib.sha256(repo.encode()).hexdigest(), 16) % FOLDS


def build(job):
    """One chunk of schemas: every pair's hashed features, and its meta."""
    schemas, split, guesses = job
    hasher = FeatureHasher(n_features=HASHED, input_type="dict", alternate_sign=False)
    feats, meta = [], []
    for s, t, c, g, label, rule, f in keys.pairs(schemas, split, guesses, 2):
        feats.append(f)
        meta.append((s["id"], s["repo"], bool(s.get("novel")), t["name"], c["name"], g["name"], g is t, label, rule,
                     f.get("v2 base", 0.0), f.get("v2 same type", 0.0), f.get("v2 named as the key", 0.0),
                     f.get("ends in id", 0.0), f.get("v2 bare", 0.0)))
    x = hasher.transform(feats).astype(np.float32) if feats else sp.csr_matrix((0, HASHED), dtype=np.float32)
    return x, meta


def build_all(pool, schemas, split, guesses):
    chosen = [s for s in schemas if s["split"] == split and s["fk"]]
    by_schema = defaultdict(dict)
    for k, v in guesses.items():
        by_schema[k[0]][k] = v
    chunks = [chosen[i::WORKERS * 4] for i in range(WORKERS * 4)]
    jobs = [(c, split, {k: v for s in c for k, v in by_schema[s["id"]].items()}) for c in chunks if c]
    xs, meta = [], []
    for x, m in pool.map(build, jobs):
        xs.append(x)
        meta.extend(m)
    return sp.vstack(xs).tocsr(), meta


def graph_features(meta, scores):
    """The second model's features for every pair, schema by schema."""
    out = np.zeros((len(meta), len(GRAPH_FEATURES)), dtype=np.float32)
    start = 0
    while start < len(meta):
        end = start
        while end < len(meta) and meta[end][SID] == meta[start][SID]:
            end += 1
        rows = range(start, end)
        by_column = defaultdict(list)
        back_to = defaultdict(float)
        # Arrows into each target, and into it from each table: their sum,
        # how many are confident, and the best two, so that a pair's own
        # arrow can be left out without walking the others again
        into_sum, into_confident = defaultdict(float), defaultdict(int)
        into_top, sibling_top = defaultdict(list), defaultdict(list)
        for i in rows:
            m = meta[i]
            sc = scores[i]
            by_column[(m[TABLE], m[COLUMN])].append(sc)
            back_to[(m[TABLE], m[TARGET])] = max(back_to[(m[TABLE], m[TARGET])], sc)
            into_sum[m[TARGET]] += sc
            into_confident[m[TARGET]] += sc >= CONFIDENT
            for top, k in ((into_top, m[TARGET]), (sibling_top, (m[TABLE], m[TARGET]))):
                top[k].append(sc)
                if len(top[k]) > 2:
                    top[k].sort(reverse=True)
                    del top[k][2:]
        mine_of = {k: sorted(v, reverse=True) for k, v in by_column.items()}
        best_of = {k: v[0] for k, v in mine_of.items()}
        confident = {k for k, b in best_of.items() if b >= CONFIDENT}
        confident_in_table = defaultdict(int)
        for k in confident:
            confident_in_table[k[0]] += 1
        last_word = {k: (tokens(k[1]) or [""])[-1] for k in by_column}
        word_sum, word_count = defaultdict(float), defaultdict(int)
        for k, w in last_word.items():
            word_sum[w] += best_of[k]
            word_count[w] += 1

        def without(top, sc):
            top = sorted(top, reverse=True)
            if top and top[0] == sc:
                return top[1] if len(top) > 1 else 0.0
            return top[0] if top else 0.0

        for i in rows:
            m = meta[i]
            key = (m[TABLE], m[COLUMN])
            mine = mine_of[key]
            sc = scores[i]
            rank = 1 + sum(1 for x in mine if x > sc)
            next_best = mine[1] if rank == 1 and len(mine) > 1 else (mine[0] if rank > 1 else 0.0)
            w = last_word[key]
            mates = word_count[w] - 1
            out[i] = [
                sc, rank, sc - next_best, mine[0], sum(1 for x in mine if x >= CONFIDENT),
                into_sum[m[TARGET]] - sc, without(into_top[m[TARGET]], sc),
                into_confident[m[TARGET]] - (sc >= CONFIDENT),
                without(sibling_top[(m[TABLE], m[TARGET])], sc),
                confident_in_table[m[TABLE]] - (key in confident),
                (len(confident) - (key in confident)) / max(len(by_column), 1),
                float(m[SELF]),
                back_to.get((m[TARGET], m[TABLE]), 0.0) if not m[SELF] else 0.0,
                (word_sum[w] - best_of[key]) / mates if mates else -1.0,
                m[BASE], m[SAME_TYPE], m[NAMED_KEY], m[ENDS_ID], m[BARE],
            ]
        start = end
    return out


def complete_mask(meta):
    """Pairs of schemas whose every key the rules guess by name is declared."""
    broken = {m[SID] for m in meta if m[RULES] and not m[LABEL]}
    return np.array([m[SID] not in broken for m in meta], dtype=bool)


def measure(y, scores, ckeys, rules, novel, complete):
    rp = prf(*counts(y, rules))[0]
    out = {}
    for how, s in (("pairs", scores), ("one target", keys.one_target(scores, ckeys))):
        t = keys.best_threshold(y, s)
        p, r, f = keys.at_threshold(y, s, t)
        predicted = s >= t
        false = predicted & ~y
        out[how] = {
            "threshold": t, "precision": p, "recall": r, "f1": f,
            "average_precision": float(average_precision_score(y, s)),
            "recall_at_rules_precision": keys.recall_at(y, s, rp),
            "novel_f1": keys.at_threshold(y[novel], s[novel], t)[2],
            "complete_f1": keys.at_threshold(y[complete], s[complete], t)[2],
            # A false arrow the rules would draw by name too is most likely a
            # key the schema did not declare
            "false_named_like_their_target": float((false & rules).sum() / max(false.sum(), 1)),
        }
    return out


def first_model(gpu: bool):
    return xgb.XGBClassifier(**FIRST_GPU) if gpu else lgb.LGBMClassifier(**FIRST)


def first_scores(model, x, gpu: bool):
    if gpu:
        model.set_params(device="cpu")  # the rows are in memory, not on the card
    scores = model.predict_proba(x)[:, 1]
    if gpu:
        model.set_params(device="cuda")
    return scores


def dev(out: Path, small: bool, gpu: bool) -> None:
    schemas, guesses = keys.load(out)
    if small:
        rng = random.Random(SEED)
        schemas = [s for s in schemas if s["split"] in ("train", "validation") and rng.random() < 0.06]
    t0 = time.time()
    clock = lambda what: print(f"{what}: {time.time() - t0:.0f} s", flush=True)  # noqa: E731

    with ProcessPoolExecutor(WORKERS) as pool:
        x, meta = build_all(pool, schemas, "train", guesses)
        clock(f"features, training: {x.shape[0]} pairs")
        xv, meta_v = build_all(pool, schemas, "validation", guesses)
        clock(f"features, validation: {xv.shape[0]} pairs")

    y = np.array([m[LABEL] for m in meta], dtype=bool)
    folds = np.array([fold_of(m[REPO]) for m in meta])
    sample = y | (np.random.default_rng(SEED).random(len(y)) < NEGATIVES)
    oof = np.zeros(len(y))
    for k in range(FOLDS):
        fit_rows = np.flatnonzero(sample & (folds != k))
        model = first_model(gpu)
        model.fit(x[fit_rows], y[fit_rows])
        held = np.flatnonzero(folds == k)
        oof[held] = first_scores(model, x[held], gpu)
        clock(f"first model, fold {k}")
    whole = first_model(gpu)
    rows = np.flatnonzero(sample)
    whole.fit(x[rows], y[rows])
    first_v = first_scores(whole, xv, gpu)
    clock("first model, all training")
    del x

    g = graph_features(meta, oof)
    gv = graph_features(meta_v, first_v)
    clock("graph features")
    second = lgb.LGBMClassifier(**SECOND)
    second.fit(g[sample], y[sample])
    second_v = second.predict_proba(gv)[:, 1]
    clock("second model")

    yv = np.array([m[LABEL] for m in meta_v], dtype=bool)
    rules_v = np.array([m[RULES] for m in meta_v], dtype=bool)
    keys_v = [(m[SID], m[TABLE], m[COLUMN]) for m in meta_v]
    novel_v = np.array([m[NOVEL] for m in meta_v], dtype=bool)
    complete_v = complete_mask(meta_v)
    result = {
        "pairs": int(len(yv)), "positives": int(yv.sum()), "complete_pairs": int(complete_v.sum()),
        "rules": dict(zip(("precision", "recall", "f1"), prf(*counts(yv, rules_v)))),
        "rules_complete": dict(zip(("precision", "recall", "f1"), prf(*counts(yv[complete_v], rules_v[complete_v])))),
        "first": measure(yv, first_v, keys_v, rules_v, novel_v, complete_v),
        "graph": measure(yv, second_v, keys_v, rules_v, novel_v, complete_v),
        "importance": dict(zip(GRAPH_FEATURES, map(int, second.booster_.feature_importance("gain")))),
        "first_library": "xgboost, cuda" if gpu else "lightgbm",
        "kib_first": round(len(whole.get_booster().save_raw(raw_format="ubj") if gpu
                               else whole.booster_.model_to_string().encode()) / 1024),
        "kib_second_text": round(len(second.booster_.model_to_string().encode()) / 1024),
        "seconds": round(time.time() - t0),
    }
    print(json.dumps(result, indent=1))
    name = "graph-dev" + ("-gpu" if gpu else "") + ("-small" if small else "")
    (out / f"{name}.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")


def compact_bytes(nodes: int, leaves: int) -> int:
    """A tree as Rust would carry it: a node a feature (u16), a threshold
    (f32), two children (u16 each) and which way a missing value goes; a
    leaf its value (f32)."""
    return nodes * 11 + leaves * 4


def final(out: Path) -> None:
    """The configuration the second pass chose on validation, measured once
    on the test and on the hard cases; its two models saved beside them."""
    schemas, guesses = keys.load(out)
    hard = [json.loads(line) for line in open(out / "hard-hidden.jsonl", encoding="utf-8")]
    hard_guesses = {}
    for line in open(out / "hard-guesses.jsonl", encoding="utf-8"):
        g = json.loads(line)
        hard_guesses[(g["schema"], g["table"], g["column"])] = g
    t0 = time.time()
    clock = lambda what: print(f"{what}: {time.time() - t0:.0f} s", flush=True)  # noqa: E731

    with ProcessPoolExecutor(WORKERS) as pool:
        x, meta = build_all(pool, schemas, "train", guesses)
        xv, meta_v = build_all(pool, schemas, "validation", guesses)
        xt, meta_t = build_all(pool, schemas, "test", guesses)
        xh, meta_h = build_all(pool, hard, "hard", hard_guesses)
    clock("features")

    y = np.array([m[LABEL] for m in meta], dtype=bool)
    folds = np.array([fold_of(m[REPO]) for m in meta])
    sample = y | (np.random.default_rng(SEED).random(len(y)) < NEGATIVES)
    oof = np.zeros(len(y))
    for k in range(FOLDS):
        fit_rows = np.flatnonzero(sample & (folds != k))
        model = first_model(True)
        model.fit(x[fit_rows], y[fit_rows])
        held = np.flatnonzero(folds == k)
        oof[held] = first_scores(model, x[held], True)
        clock(f"first model, fold {k}")
    whole = first_model(True)
    rows = np.flatnonzero(sample)
    whole.fit(x[rows], y[rows])
    del x
    clock("first model, all training")
    second = lgb.LGBMClassifier(**SECOND)
    second.fit(graph_features(meta, oof)[sample], y[sample])
    clock("second model")

    def predict(xs, ms):
        s = second.predict_proba(graph_features(ms, first_scores(whole, xs, True)))[:, 1]
        return keys.one_target(s, [(m[SID], m[TABLE], m[COLUMN]) for m in ms])

    yv = np.array([m[LABEL] for m in meta_v], dtype=bool)
    threshold = keys.best_threshold(yv, predict(xv, meta_v))
    st, sh = predict(xt, meta_t), predict(xh, meta_h)
    clock("scored")

    rng = np.random.default_rng(SEED)
    yt = np.array([m[LABEL] for m in meta_t], dtype=bool)
    rules_t = np.array([m[RULES] for m in meta_t], dtype=bool)
    repos_t = np.array([m[REPO] for m in meta_t])
    predicted = st >= threshold
    result = {"threshold": threshold, "test": {}, "hard": {}}
    subsets = {"all": np.ones_like(yt), "novel": np.array([m[NOVEL] for m in meta_t], dtype=bool),
               "complete": complete_mask(meta_t)}
    for name, mask in subsets.items():
        yy, pp, rr, ss = yt[mask], predicted[mask], rules_t[mask], st[mask]
        rules = prf(*counts(yy, rr))
        low, high = bootstrap(yy, pp, rr, repos_t[mask], rng)
        result["test"][name] = {
            "pairs": int(mask.sum()), "keys": int(yy.sum()),
            "rules": dict(zip(("precision", "recall", "f1"), rules)),
            "model": dict(zip(("precision", "recall", "f1"), prf(*counts(yy, pp)))),
            "average_precision": float(average_precision_score(yy, ss)),
            "recall_at_rules_precision": keys.recall_at(yy, ss, rules[0]),
            "f1_minus_rules_95": [low, high],
        }

    yh = np.array([m[LABEL] for m in meta_h], dtype=bool)
    rules_h = np.array([m[RULES] for m in meta_h], dtype=bool)
    styles = np.array([m[REPO].split("/")[1] for m in meta_h])
    by_sid = {s["id"]: s for s in hard}
    decoy = np.array([[m[TABLE], m[COLUMN]] in by_sid[m[SID]]["decoys"] for m in meta_h], dtype=bool)
    ph = sh >= threshold
    for name in ["all"] + sorted(set(styles)):
        mask = np.ones_like(yh) if name == "all" else styles == name
        columns = lambda taken: len({(m[SID], m[TABLE], m[COLUMN])  # noqa: E731
                                     for m, d, t, k in zip(meta_h, decoy, taken, mask) if d and t and k})
        result["hard"][name] = {
            "keys": int(yh[mask].sum()),
            "rules": dict(zip(("precision", "recall", "f1"), prf(*counts(yh[mask], rules_h[mask])))),
            "model": dict(zip(("precision", "recall", "f1"), prf(*counts(yh[mask], ph[mask])))),
            "decoys": columns(np.ones_like(yh)),
            "decoys_taken_by_rules": columns(rules_h),
            "decoys_taken_by_model": columns(ph),
        }

    # XGBoost's text dump is a node a line, its leaves marked `leaf=`
    first_dump = whole.get_booster().get_dump()
    first_nodes = sum(len([line for line in t.splitlines() if line.strip()]) for t in first_dump)
    first_leaves = sum(t.count("leaf=") for t in first_dump)
    second_dump = second.booster_.dump_model()["tree_info"]
    second_leaves = sum(t["num_leaves"] for t in second_dump)
    result["size"] = {
        "first_trees": len(first_dump),
        "first_file_kib": round(len(whole.get_booster().save_raw(raw_format="ubj")) / 1024),
        "first_compact_kib": round(compact_bytes(first_nodes - first_leaves, first_leaves) / 1024),
        "second_trees": len(second_dump),
        "second_file_kib": round(len(second.booster_.model_to_string().encode()) / 1024),
        "second_compact_kib": round(compact_bytes(second_leaves - len(second_dump), second_leaves) / 1024),
    }
    result["seconds"] = round(time.time() - t0)
    whole.save_model(str(out / "keys-first.ubj"))
    second.booster_.save_model(str(out / "keys-second.txt"))
    print(json.dumps(result, indent=1))
    (out / "graph-final.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "final":
        final(Path(sys.argv[2]))
    else:
        dev(Path(sys.argv[2]), "--small" in sys.argv, "--gpu" in sys.argv)
