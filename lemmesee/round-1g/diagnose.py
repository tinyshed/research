"""Round 1g: which of capacity, context and objective limits the look. Every
configuration is taught on four of five folds of database families and
scored on the fifth, three draws of the folds; the five is the look's order
with nothing tuned.

    python -I diagnose.py <round 1d look> <round 1e look> <round 1f look> <corpus/see> <out> [--set=words]
    python -I diagnose.py ceilings|errors <the same>

Writes <out>/diagnose.json with every measure, and <out>/scores.json with
each configuration's out-of-fold score of every panel, which sets.py and
pairs.py read; the second set, the words, to diagnose-words.json and
scores-words.json. ceilings counts the five each source of labels would
choose by itself; errors lists where the look is furthest from its
teachers.
"""

from __future__ import annotations

import importlib.util
import json
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


look1f = _load("look1f", HERE.parent / "round-1f" / "look.py")
look1e = look1f.look1e
look1d = look1f.look1d
import see  # noqa: E402  (round 1c's, on the path round 1e's look set)

SEED = 20261101
FOLDS, DRAWS = 5, 3
TEACHERS = look1e.TEACHERS
COUNTERS = ("opus", "sonnet", "gemini")
ALT = ["alt_pool", "alt_same_bm", "alt_same_bm_above", "alt_same_time", "alt_same_by", "alt_base_share",
       "alt_corr_above", "alt_jaccard_above", "alt_same_rows_above"]
NO_PENALTY = {k: 0.0 for k in look1e.choose.SHARED}


# ---- the data

def families(fes: list[str], see_dir: Path, overlap: float = 0.4) -> dict[str, str]:
    """Databases whose table names overlap by this much or more are one family."""
    tables = {fe: {t["name"].lower() for t in json.loads((see_dir / fe / "snapshot.json").read_text(encoding="utf-8"))["tables"]}
              for fe in fes}
    parent = {fe: fe for fe in fes}

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x
    for i, a in enumerate(fes):
        for b in fes[i + 1:]:
            if len(tables[a] & tables[b]) / len(tables[a] | tables[b]) >= overlap:
                parent[find(a)] = find(b)
    return {fe: find(fe) for fe in fes}


def load_all(dirs: list[Path], see_dir: Path) -> dict:
    home = {p.parent.name: d for d in dirs for p in d.glob("*/pool.json")}
    fes = sorted(home)
    pools = {fe: json.loads((home[fe] / fe / "pool.json").read_text(encoding="utf-8"))["panels"] for fe in fes}
    ans = {j: {} for j in TEACHERS + COUNTERS}
    for j in ans:
        for d in dirs:
            ans[j] |= look1d.labels(d, j)
    ident, qfeat = {}, {}
    for fe in fes:
        ident[fe], qfeat[fe] = look1e.questions(see_dir, fe)
        for p in pools[fe]:
            p["question"] = qfeat[fe].get(p["title"], {})
            p["more"] = look1e.more(p)
    look1d.QUESTION[:] = sorted({k for fe in fes for f in qfeat[fe].values() for k in f})
    mine = {fe: [p for p in pools[fe] if "round 1" not in p["in"]] for fe in fes}
    ids = {fe: [ident[fe].get(p["title"], ("?", p["title"])) for p in mine[fe]] for fe in fes}
    import learn
    import rank
    from aq import generate, load
    table = json.loads((HERE.parent / "corpus" / "round-1c" / "learn" / "words.json").read_text(encoding="utf-8"))
    for fe in fes:
        for p, a in zip(mine[fe], alternatives(mine[fe], ids[fe])):
            p["alt"] = a
        # each panel's words by where they stand, and the rate round 1c's
        # teachers asked a question holding them
        schema = load(json.loads((see_dir / fe / "snapshot.json").read_text(encoding="utf-8")), True)
        by_title = {}
        for q in generate(schema):
            by_title.setdefault(q.title(schema), rank.slots(schema, q))
        for p in mine[fe]:
            p["slots"] = by_title.get(p["title"], {s: ["#none"] for s in learn.SLOTS})
        for p, w in zip(mine[fe], learn.word_features({"slots": [p["slots"] for p in mine[fe]]}, table)):
            p["asked"] = w
    lex_file = HERE.parent / "corpus" / "round-1g" / "lexicon" / "lexicon.json"
    if lex_file.exists():
        lex = json.loads(lex_file.read_text(encoding="utf-8"))
        for fe in fes:
            for p in mine[fe]:
                p["lex"] = lexicon(p["slots"], lex)
    # a judge counts a database where it answered every candidate
    covered = {j: [fe for fe in fes if all(p["id"] in ans[j] for p in mine[fe])] for j in COUNTERS}
    return {"fes": fes, "pools": pools, "mine": mine, "ids": ids, "ans": ans, "covered": covered,
            "family": families(fes, see_dir)}


# ---- the alternatives' context

def _series(p: dict) -> dict[float, float] | None:
    if p.get("bucket") is None or not p.get("rows") or len(p["rows"][0]) != 2:
        return None
    out = {}
    for r in p["rows"]:
        d, v = see.date_days(str(r[0])), look1d.num(r[-1])
        if d is not None and v is not None:
            out[d] = v
    return out


def _labels(p: dict) -> set | None:
    if p.get("bucket") is not None or not p.get("rows") or len(p["rows"][0]) < 2:
        return None
    return {str(r[0]) for r in p["rows"]}


def _corr(a: dict, b: dict) -> float | None:
    common = sorted(set(a) & set(b))
    if len(common) < 4:
        return None
    x, y = np.array([a[d] for d in common]), np.array([b[d] for d in common])
    if x.std() == 0 or y.std() == 0:
        return None
    return float(np.corrcoef(x, y)[0, 1])


def alternatives(mine: list[dict], ids: list[tuple]) -> list[dict]:
    """What the look could know of the other candidates: how many share the
    panel's base and measure, time or breakdown, and how like a higher-scored
    one its result is."""
    n = len(mine)
    score = [p["score"] for p in mine]
    series = [_series(p) for p in mine]
    labels = [_labels(p) for p in mine]
    rows = [tuple(map(tuple, p.get("rows") or [])) for p in mine]
    out = []
    for i in range(n):
        others = [j for j in range(n) if j != i]
        above = [j for j in others if score[j] > score[i]]
        bm = [j for j in others if ids[j][0] == ids[i][0] and ids[j][1] == ids[i][1]]
        corr = [c for j in above if series[i] and series[j] and (c := _corr(series[i], series[j])) is not None]
        jac = [len(labels[i] & labels[j]) / len(labels[i] | labels[j]) for j in above
               if labels[i] and labels[j] and labels[i] | labels[j]]
        out.append({
            "alt_pool": float(n),
            "alt_same_bm": float(len(bm)),
            "alt_same_bm_above": float(sum(1 for j in bm if score[j] > score[i])),
            "alt_same_time": float(sum(1 for j in others if ids[i][2] is not None and ids[j][2] == ids[i][2])),
            "alt_same_by": float(sum(1 for j in others if ids[i][3] is not None and ids[j][3] == ids[i][3])),
            "alt_base_share": sum(1 for j in range(n) if ids[j][0] == ids[i][0]) / n,
            "alt_corr_above": max(corr) if corr else float("nan"),
            "alt_jaccard_above": max(jac) if jac else float("nan"),
            "alt_same_rows_above": float(any(rows[i] and rows[j] == rows[i] for j in above)),
        })
    return out


# ---- the words

SLOTS = ("base", "measure", "time", "by")
SMOOTH = 10.0
ASKED = [f"w_{s}_{k}" for s in SLOTS for k in ("max", "min")]
KEPT = [f"k_{s}_{k}" for s in SLOTS for k in ("max", "min", "mean")]


def kept_words(D: dict, train: list[str], test: list[str]) -> None:
    """Each word's rate of being kept by the teachers, by where it stands,
    counted on the databases taught and smoothed toward their mean; a
    database taught sees the rates without its own panels."""
    total = {s: {} for s in SLOTS}
    own = {fe: {s: {} for s in SLOTS} for fe in train}
    ts = []
    for fe in train:
        for p in D["mine"][fe]:
            t = target(p, D["ans"])
            if t is None:
                continue
            ts.append(t)
            for s in SLOTS:
                for w in set(p["slots"].get(s, [])):
                    for d in (total[s], own[fe][s]):
                        c = d.setdefault(w, [0.0, 0.0])
                        c[0] += t
                        c[1] += 1.0
    prior = float(np.mean(ts))
    for fe in train + test:
        mine_own = own.get(fe)
        for p in D["mine"][fe]:
            f = {}
            for s in SLOTS:
                rates = []
                for w in set(p["slots"].get(s, [])):
                    c = total[s].get(w)
                    if c is None:
                        continue
                    pos, n = c
                    if mine_own and w in mine_own[s]:
                        pos, n = pos - mine_own[s][w][0], n - mine_own[s][w][1]
                    if n <= 0:
                        continue
                    rates.append((pos + SMOOTH * prior) / (n + SMOOTH))
                f[f"k_{s}_max"] = max(rates) if rates else float("nan")
                f[f"k_{s}_min"] = min(rates) if rates else float("nan")
                f[f"k_{s}_mean"] = float(np.mean(rates)) if rates else float("nan")
            p["kept"] = f


PROPS = ("id", "money", "count", "sum_ok", "audit_time", "event_time", "good_group", "cryptic", "importance")
LEX = [f"x_{s}_{k}" for s in SLOTS for k in PROPS] + [f"x_{s}_known" for s in SLOTS]


def lexicon(slots: dict, lex: dict) -> dict:
    """The lexicon's nine numbers for the words in each place of a
    question, the largest of each, and how many of its words the lexicon
    knows; a measure's words come as `column:population`."""
    words = lex["words"]
    f = {}
    for s in SLOTS:
        ws = [w.split(":", 1)[-1] for w in slots.get(s, []) if not w.startswith("#")]
        known = [words[w] for w in ws if w in words]
        for i, k in enumerate(PROPS):
            f[f"x_{s}_{k}"] = max(v[i] for v in known) if known else float("nan")
        f[f"x_{s}_known"] = len(known) / len(ws) if ws else float("nan")
    return f


# ---- the looks

def vector(p: dict, feats: str) -> list[float]:
    """Round 1e's numbers, and what the configuration adds: the
    alternatives' context, and the words as round 1c's teachers asked them
    and as this look's teachers kept them."""
    v = look1e.vector(p)
    if "context" in feats:
        v = v + [p["alt"][k] for k in ALT]
    if "words" in feats:
        v = v + [p["asked"].get(k, float("nan")) for k in ASKED] + [p["kept"][k] for k in KEPT]
    if "lexicon" in feats:
        v = v + [p["lex"][k] for k in LEX]
    return v


def target(p: dict, ans: dict) -> float | None:
    return look1e.target(p, ans)


def fit(D: dict, train: list[str], capacity: str, feats: str, objective: str, seed: int):
    """A look taught on these databases. The larger one stops on a fifth of
    their families held out of its own training."""
    import lightgbm as lgb
    rng = np.random.default_rng(seed)

    def data(fes):
        X, y, groups = [], [], []
        for fe in fes:
            rows = [(vector(p, feats), t) for p in D["mine"][fe] if (t := target(p, D["ans"])) is not None]
            if not rows:
                continue
            X += [r[0] for r in rows]
            y += [r[1] for r in rows]
            groups.append(len(rows))
        y = np.array(y)
        if objective == "rank":
            y = np.round(y * 3).astype(int)
        return np.array(X, dtype=float), y, groups

    base = dict(feature_fraction=0.5, bagging_fraction=0.9, bagging_freq=1, lambda_l2=1.0, verbose=-1, seed=seed,
                deterministic=True, force_row_wise=True)
    base |= ({"objective": "cross_entropy", "metric": "cross_entropy"} if objective == "point"
             else {"objective": "lambdarank", "metric": "ndcg", "eval_at": [5], "label_gain": [0, 1, 3, 7]})
    if capacity == "small":
        X, y, g = data(train)
        ds = lgb.Dataset(X, y, group=g if objective == "rank" else None)
        return lgb.train(base | dict(learning_rate=0.05, num_leaves=4, min_data_in_leaf=10), ds, num_boost_round=150)
    fams = sorted({D["family"][fe] for fe in train})
    rng.shuffle(fams)
    stop = set(fams[: max(1, len(fams) // 5)])
    inner = [fe for fe in train if D["family"][fe] not in stop]
    held = [fe for fe in train if D["family"][fe] in stop]
    X, y, g = data(inner)
    Xv, yv, gv = data(held)
    ds = lgb.Dataset(X, y, group=g if objective == "rank" else None)
    dv = lgb.Dataset(Xv, yv, group=gv if objective == "rank" else None, reference=ds)
    params = base | dict(learning_rate=0.03, num_leaves=31, min_data_in_leaf=20)
    model = lgb.train(params, ds, num_boost_round=2000, valid_sets=[dv],
                      callbacks=[lgb.early_stopping(100, verbose=False)])
    return model


# ---- the measures

def auc(y, s) -> float | None:
    y, s = np.asarray(y, dtype=int), np.asarray(s, dtype=float)
    pos, neg = s[y == 1], s[y == 0]
    if not len(pos) or not len(neg):
        return None
    return float(((pos[:, None] > neg[None, :]).sum() + 0.5 * (pos[:, None] == neg[None, :]).sum()) / (len(pos) * len(neg)))


def five(D: dict, fe: str, s) -> list[int]:
    return look1e.Chooser(D["ids"][fe]).choose(np.asarray(s, dtype=float), NO_PENALTY)


def ndcg5(keep: np.ndarray, order: list[int]) -> float | None:
    if keep.sum() == 0:
        return None
    dcg = sum(keep[i] / math.log2(k + 2) for k, i in enumerate(order[:5]))
    ideal = sorted(keep, reverse=True)[:5]
    return float(dcg / sum(g / math.log2(k + 2) for k, g in enumerate(ideal)))


# the first pass: capacity, context and objective; the second: the words,
# which the first pass's errors pointed to
SETS = {
    "first": [(c, x, o) for o in ("point", "rank") for c in ("small", "large") for x in ("numbers", "context")],
    "words": [("small", "numbers", "point"), ("large", "numbers", "point"), ("small", "words", "point"),
              ("large", "words", "point"), ("small", "words and context", "point")],
    "lexicon": [("small", "numbers", "point"), ("large", "numbers", "point"), ("small", "lexicon", "point"),
                ("large", "lexicon", "point"), ("large", "words and lexicon", "point")],
}
FEATS = {"numbers": "round 1e numbers", "context": "with context", "words": "with words",
         "words and context": "with words and context", "lexicon": "with the lexicon",
         "words and lexicon": "with words and the lexicon"}


def name(c, x, o) -> str:
    return f"{c}, {FEATS[x]}, {'per panel' if o == 'point' else 'LambdaRank'}"


def main(dirs: list[Path], see_dir: Path, out: Path, which: str = "first") -> None:
    CONFIGS = SETS[which]
    stem = "" if which == "first" else f"-{which}"
    out.mkdir(parents=True, exist_ok=True)
    D = load_all(dirs, see_dir)
    fes, ans = D["fes"], D["ans"]
    fams = sorted(set(D["family"].values()))
    counted = sorted({fe for j in ("sonnet", "gemini") for fe in D["covered"][j]})
    print(f"{len(fes)} databases, {len(fams)} families; counted by Sonnet {len(D['covered']['sonnet'])}, "
          f"Gemini {len(D['covered']['gemini'])}, Opus {len(D['covered']['opus'])}", flush=True)

    scores = {name(*cfg): {fe: np.zeros(len(D["mine"][fe])) for fe in fes} for cfg in CONFIGS}
    per_db = {name(*cfg): {fe: [] for fe in fes} for cfg in CONFIGS}
    fit_auc = {name(*cfg): [] for cfg in CONFIGS}
    rounds = {name(*cfg): [] for cfg in CONFIGS}
    for draw in range(DRAWS):
        order = list(fams)
        np.random.default_rng(SEED + draw).shuffle(order)
        fold_of = {f: k % FOLDS for k, f in enumerate(order)}
        for k in range(FOLDS):
            test = [fe for fe in fes if fold_of[D["family"][fe]] == k]
            train = [fe for fe in fes if fold_of[D["family"][fe]] != k]
            kept_words(D, train, test)
            for cfg in CONFIGS:
                nm = name(*cfg)
                model = fit(D, train, *cfg, seed=SEED + 100 * draw + k)
                rounds[nm].append(model.num_trees())
                # the folds taught on, against the teachers, for memorising
                ins = []
                for fe in train:
                    t = [target(p, ans) for p in D["mine"][fe]]
                    if None in t:
                        continue
                    a = auc([x >= 0.5 for x in t], model.predict(np.array([vector(p, cfg[1]) for p in D["mine"][fe]], dtype=float)))
                    if a is not None:
                        ins.append(a)
                fit_auc[nm].append(float(np.mean(ins)))
                for fe in test:
                    s = model.predict(np.array([vector(p, cfg[1]) for p in D["mine"][fe]], dtype=float))
                    scores[nm][fe] += s / DRAWS
                    per_db[nm][fe].append(s)
            print(f"draw {draw + 1} fold {k + 1} done", flush=True)

    def measures(nm: str) -> dict[str, dict[str, float]]:
        """Each measure by database, averaged over the draws."""
        m: dict[str, dict[str, float]] = {}
        for fe in fes:
            runs = per_db[nm][fe]
            t = [target(p, ans) for p in D["mine"][fe]]
            if None not in t:
                vals = [a for s in runs if (a := auc([x >= 0.5 for x in t], s)) is not None]
                if vals:
                    m.setdefault("teachers, AUC", {})[fe] = float(np.mean(vals))
            for j in COUNTERS:
                if fe not in D["covered"][j]:
                    continue
                keep = np.array([bool(ans[j][p["id"]]["keep"]) for p in D["mine"][fe]], dtype=int)
                a = [x for s in runs if (x := auc(keep, s)) is not None]
                n = [x for s in runs if (x := ndcg5(keep, five(D, fe, s))) is not None]
                k5 = [float(keep[five(D, fe, s)].sum()) for s in runs]
                if a:
                    m.setdefault(f"{j}, AUC", {})[fe] = float(np.mean(a))
                if n:
                    m.setdefault(f"{j}, NDCG@5", {})[fe] = float(np.mean(n))
                m.setdefault(f"{j}, kept of five", {})[fe] = float(np.mean(k5))
        return m

    result: dict = {"databases": len(fes), "families": len(fams), "counted": {j: len(D["covered"][j]) for j in COUNTERS},
                    "folds": FOLDS, "draws": DRAWS}
    base_nm = name("small", "numbers", "point")
    M = {nm: measures(nm) for nm in scores}
    rng = np.random.default_rng(SEED)
    for nm, m in M.items():
        row = {k: round(float(np.mean(list(v.values()))), 3) for k, v in sorted(m.items())}
        row["teachers, AUC on the folds taught"] = round(float(np.mean(fit_auc[nm])), 3)
        row["trees, median"] = int(np.median(rounds[nm]))
        if nm != base_nm:
            for k in ("opus, kept of five", "sonnet, kept of five", "gemini, kept of five", "sonnet, AUC", "gemini, AUC", "opus, AUC"):
                common = [fe for fe in m.get(k, {}) if fe in M[base_nm].get(k, {})]
                d = np.array([m[k][fe] - M[base_nm][k][fe] for fe in common])
                if len(d):
                    boots = [rng.choice(d, len(d)).mean() for _ in range(10000)]
                    row[f"{k} − baseline"] = {"mean": round(float(d.mean()), 3),
                                              "95%": [round(float(np.percentile(boots, q)), 3) for q in (2.5, 97.5)]}
        result[nm] = row
    # the fives of rounds 1 and 1c, as they ran, by the same judges on the same databases
    for label, tag in (("round 1, B1", "round 1"), ("round 1c", "round 1c")):
        row = {}
        for j in COUNTERS:
            vals = [sum(1 for p in D["pools"][fe] if tag in p["in"] and ans[j].get(p["id"], {}).get("keep"))
                    for fe in D["covered"][j]]
            row[f"{j}, kept of five"] = round(float(np.mean(vals)), 3)
        result[label] = row
    row = {}
    for j in COUNTERS:
        row[f"{j}, kept of five"] = round(float(np.mean([min(5, len({D["ids"][fe][i] for i, p in enumerate(D["mine"][fe])
                                                                     if ans[j][p["id"]]["keep"]})) for fe in D["covered"][j]])), 3)
    result["the best five the pool holds"] = row
    (out / f"diagnose{stem}.json").write_text(json.dumps(result, indent=1, ensure_ascii=False), encoding="utf-8")
    (out / f"scores{stem}.json").write_text(json.dumps({nm: {fe: [round(float(x), 5) for x in s] for fe, s in v.items()}
                                                        for nm, v in scores.items()}), encoding="utf-8")
    print(json.dumps(result, indent=1, ensure_ascii=False))


def ceilings(dirs: list[Path], see_dir: Path, out: Path) -> None:
    """What a perfect student of each source of labels would keep: the five
    chosen by one judge's own answers, or the teachers' mean, counted by the
    others — the most any look taught by that source could reach."""
    D = load_all(dirs, see_dir)
    ans = D["ans"]
    sources = {"the teachers' mean": lambda p: look1e.target(p, ans)}
    for j in TEACHERS:
        sources[j] = (lambda j: lambda p: (float(ans[j][p["id"]]["p_keep"]) if "p_keep" in ans[j][p["id"]]
                                           else float(bool(ans[j][p["id"]]["keep"]))) if p["id"] in ans[j] else None)(j)
    for j in COUNTERS:
        sources[j] = (lambda j: lambda p: float(bool(ans[j][p["id"]]["keep"])) if p["id"] in ans[j] else None)(j)
    result: dict = {}
    for src, f in sources.items():
        row = {}
        for j in COUNTERS:
            if j == src:
                continue
            kept, aucs = [], []
            for fe in D["covered"][j]:
                s = [f(p) for p in D["mine"][fe]]
                if None in s:
                    continue
                keep = np.array([bool(ans[j][p["id"]]["keep"]) for p in D["mine"][fe]], dtype=int)
                # ties broken by the ranker's score, as the look's would be
                s = np.array(s) + 1e-6 * np.array([p["score"] for p in D["mine"][fe]])
                kept.append(float(keep[five(D, fe, s)].sum()))
                if (a := auc(keep, s)) is not None:
                    aucs.append(a)
            if kept:
                row[f"{j}, kept of five"] = round(float(np.mean(kept)), 2)
                row[f"{j}, AUC"] = round(float(np.mean(aucs)), 3)
                row[f"{j}, databases"] = len(kept)
        result[src] = row
    (out / "ceilings.json").write_text(json.dumps(result, indent=1, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(result, indent=1, ensure_ascii=False))


def errors(dirs: list[Path], see_dir: Path, out: Path, config: str = "small, round 1e numbers, per panel") -> None:
    """Where the look, out of fold, is furthest from its teachers: what the
    teachers keep and it drops, and the reverse, with what a judge saw."""
    D = load_all(dirs, see_dir)
    scores = json.loads((out / "scores.json").read_text(encoding="utf-8"))[config]
    rows = []
    for fe in D["fes"]:
        for p, s in zip(D["mine"][fe], scores[fe]):
            t = look1e.target(p, D["ans"])
            if t is None:
                continue
            first = [" | ".join("∅" if v is None else str(v)[:24] for v in r) for r in (p.get("rows") or [])[:4]]
            rows.append({"db": fe.rsplit("-", 1)[0], "title": p["title"], "teachers": round(t, 2), "look": round(float(s), 2),
                         "rows": len(p.get("rows") or []), "first": first,
                         "counters": {j: D["ans"][j][p["id"]]["keep"] for j in COUNTERS if p["id"] in D["ans"][j]}})
    rows.sort(key=lambda r: r["teachers"] - r["look"])
    result = {"the look keeps, the teachers drop": rows[:60], "the teachers keep, the look drops": rows[::-1][:60]}
    (out / "errors.json").write_text(json.dumps(result, indent=1, ensure_ascii=False), encoding="utf-8")
    for k, v in result.items():
        print("==", k)
        for r in v[:40]:
            print(f"  {r['teachers']:.2f} {r['look']:.2f} {r['db'][:14]:14} {r['title'][:78]} | {' / '.join(r['first'][:2])[:60]}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "errors":
        a = [Path(x) for x in sys.argv[2:]]
        errors(a[:3], a[3], a[4])
    elif sys.argv[1] == "ceilings":
        a = [Path(x) for x in sys.argv[2:]]
        ceilings(a[:3], a[3], a[4])
    else:
        which = next((x.split("=")[1] for x in sys.argv if x.startswith("--set=")), "first")
        a = [Path(x) for x in sys.argv[1:] if not x.startswith("--")]
        main(a[:3], a[3], a[4], which)
