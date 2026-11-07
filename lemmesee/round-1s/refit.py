"""Round 1s: round 1p's `both` and round 1b's base configuration fitted again
with the letter trigrams sorted, and every model measured where rounds 1p to
1r measured it, beside the saved models scored both ways.

    python -I refit.py check <round 1b out> <round 1p out> <out>
    python -I refit.py fit <round 1b out> <out> [<fit>...]
    python -I refit.py measure <round 1b out> <round 1p out> <out> [<scoring>...]
    python -I refit.py summarize <round 1p out> <out>

`check` builds validation's features twice by pools in two sets of worker
processes, with the trigrams sorted and in process order, and says whether
each pair of builds is identical; and counts the candidate pairs whose stem
has more than twelve trigrams. `fit` fits FITS into <out>/models/. `measure`
runs each of SCORINGS as a process of its own — `score`, below, with its
environment set — into <out>/scored/ and <out>/external/, then `summarize`,
which writes <out>/summary.json: intervals and differences.
"""

import json
import os
import shutil
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import letters  # noqa: E402  (first: it sorts the trigrams in this process)

import numpy as np  # noqa: E402
from sklearn.metrics import average_precision_score, roc_auc_score  # noqa: E402

import graph  # noqa: E402
import keys  # noqa: E402
import targets  # noqa: E402
import widen  # noqa: E402
from train import bootstrap, counts, prf  # noqa: E402

FITS = {
    # name: (variant, seed); the first is the fit the paper carries
    "both": ("both", 20261027),
    "both-seed-1": ("both", 1),
    "both-seed-2": ("both", 2),
    "base": ("reported", 20261027),
}
ALL = ("validation", "test", "external")
EXTERNAL_SEED = 20261101  # round 1p's resampling of databases
TS = np.round(np.arange(0.05, 0.96, 0.01), 2)


def scorings(round_1p: Path, out: Path) -> dict:
    """name: (models, variant, trigram order, hash seed, parts)."""
    saved_both, saved_base = round_1p / "variants/both", round_1p / "models-1524905"
    s = {}
    for k in range(5):
        s[f"1p-both-process-{k}"] = (saved_both, "both", "process", k, ALL if k == 0 else ("validation", "external"))
    s["1p-both-sorted"] = (saved_both, "both", "sorted", None, ALL)
    s["1b-base-process-0"] = (saved_base, "reported", "process", 0, ALL)
    s["1b-base-sorted"] = (saved_base, "reported", "sorted", None, ALL)
    for name, (variant, _) in FITS.items():
        s[name] = (out / "models" / name, variant, "sorted", None,
                   ALL if name in ("both", "base") else ("validation", "external"))
    return s


def ckeys(meta):
    return [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta]


def labels(meta):
    return np.array([m[graph.LABEL] for m in meta], dtype=bool)


def pr(t):
    return dict(zip(("precision", "recall", "f1"), t))


# --- check -------------------------------------------------------------------

def count_long(job):
    """Candidate pairs, and those whose column's stem has more than twelve
    trigrams, so that the twelve kept depend on their order."""
    schemas, split = job
    pairs = long_pairs = 0
    columns, long_columns = set(), set()
    for s, t, c, *_ in widen.pairs(schemas, split, {}, own_key=True, pseudo=True):
        stem = keys.stem_of(c["name"])[0]
        is_long = bool(stem) and len(keys.grams(stem)) > 12
        pairs += 1
        long_pairs += is_long
        columns.add((s["id"], t["name"], c["name"]))
        if is_long:
            long_columns.add((s["id"], t["name"], c["name"]))
    return pairs, long_pairs, len(columns), len(long_columns)


def check(data: Path, round_1p: Path, out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    schemas, guesses = keys.load(data)
    result = {}
    for how in ("process", "sorted"):
        os.environ["LEMMESEE_GRAMS"] = how  # inherited by the workers of the pools below
        builds = []
        for _ in range(2):
            with letters.pool(graph.WORKERS) as pool:
                builds.append(widen.build_all(pool, schemas, "validation", guesses, "both"))
        (x1, m1), (x2, m2) = builds
        same_meta = m1 == m2
        diff = (x1 != x2)
        rows = int(np.unique(diff.nonzero()[0]).size)
        result[how] = {"pairs": int(x1.shape[0]), "meta identical": same_meta,
                       "pairs whose features differ between the two builds": rows,
                       "identical": same_meta and rows == 0}
        print(how, json.dumps(result[how]), flush=True)
    os.environ["LEMMESEE_GRAMS"] = "sorted"
    ext = [json.loads(line) for line in open(round_1p / "hidden.jsonl", encoding="utf-8")]
    long = {}
    with letters.pool(graph.WORKERS) as pool:
        for split, chosen in (("train", None), ("validation", None), ("external", ext)):
            rows = chosen or [s for s in schemas if s["split"] == split and s["fk"]]
            n = graph.WORKERS * 4
            jobs = [(rows[i::n], split) for i in range(n) if rows[i::n]]
            tot = np.zeros(4, dtype=np.int64)
            for r in pool.map(count_long, jobs):
                tot += np.array(r)
            long[split] = {"candidate pairs": int(tot[0]), "pairs whose stem has over twelve trigrams": int(tot[1]),
                           "columns with a candidate": int(tot[2]), "columns whose stem has over twelve trigrams": int(tot[3])}
            print(split, json.dumps(long[split]), flush=True)
    result["long stems"] = long
    (out / "check.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")


# --- fit -----------------------------------------------------------------------

def fit(data: Path, out: Path, names: list[str]) -> None:
    assert letters.is_sorted()
    schemas, guesses = keys.load(data)
    first, second, seed = dict(graph.FIRST_GPU), dict(graph.SECOND), widen.SEED
    with letters.pool(graph.WORKERS) as pool:
        for name in names:
            variant, s = FITS[name]
            graph.FIRST_GPU = {**first, "random_state": s}
            graph.SECOND = {**second, "random_state": s}
            widen.SEED = s
            result, whole, model2 = widen.fit(pool, schemas, guesses, variant)
            result.update(fit=name, seed=s, grams="sorted")
            d = out / "models" / name
            d.mkdir(parents=True, exist_ok=True)
            whole.save_model(str(d / "keys-first.ubj"))
            model2.booster_.save_model(str(d / "keys-second.txt"))
            (d / "graph-final.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
            print(json.dumps(result), flush=True)
    graph.FIRST_GPU, graph.SECOND, widen.SEED = first, second, seed


# --- one scoring ---------------------------------------------------------------

def curve(meta, kept, declared: int):
    """P, R and F1 over every declared key at each threshold of TS."""
    y = labels(meta)
    rows = []
    for t in TS:
        d = kept >= t
        tp = int((d & y).sum())
        p = tp / max(int(d.sum()), 1)
        r = tp / max(declared, 1)
        rows.append([float(t), p, r, 2 * p * r / (p + r) if p + r else 0.0])
    return rows


def per_schema(meta, kept, threshold, schemas, split):
    """tp, fp, fn over every declared key, each schema declaring one."""
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["split"] == split and s["fk"]}
    preds = defaultdict(set)
    for i in np.flatnonzero(kept >= threshold):
        m = meta[i]
        preds[m[graph.SID]].add((m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]))
    return {i: [len(truth[i] & preds[i]), len(preds[i] - truth[i]), len(truth[i] - preds[i])] for i in truth}


def score(data: Path, round_1p: Path, out: Path, name: str) -> None:
    import external as ext  # noqa: E402
    models, variant, order, _, parts = scorings(round_1p, out)[name]
    assert letters.order() == order and letters.is_sorted() == (order == "sorted"), (letters.order(), order)
    t0 = time.time()
    first, second = widen.load_models(models)
    threshold = json.load(open(models / "graph-final.json", encoding="utf-8"))["threshold"]
    schemas, guesses = keys.load(data)
    referred = targets.load_referred(data)
    result = {"scoring": name, "variant": variant, "grams": order,
              "hash seed": os.environ.get("PYTHONHASHSEED"), "threshold": threshold}

    def predict(x, meta):
        s1 = first.predict_proba(x)[:, 1]
        raw = second.predict(graph.graph_features(meta, s1))
        return s1, raw, keys.one_target(raw, ckeys(meta))

    if "validation" in parts or "test" in parts:
        with letters.pool(graph.WORKERS) as pool:
            xv, mv = widen.build_all(pool, schemas, "validation", guesses, variant)
            if "test" in parts:
                xt, mt = widen.build_all(pool, schemas, "test", guesses, variant)
                hard = [json.loads(line) for line in open(data / "hard-hidden.jsonl", encoding="utf-8")]
                hard_guesses = {}
                for line in open(data / "hard-guesses.jsonl", encoding="utf-8"):
                    g = json.loads(line)
                    hard_guesses[(g["schema"], g["table"], g["column"])] = g
                xh, mh = widen.build_all(pool, hard, "hard", hard_guesses, variant)
        s1v, rawv, kv = predict(xv, mv)
        yv = labels(mv)
        declared_v = sum(len(s["fk"]) for s in schemas if s["split"] == "validation" and s["fk"])
        # The first stage alone and the name score alone, each at its own
        # validation threshold, as round 1p's ablation chose the first's
        first_t = keys.best_threshold(yv, keys.one_target(s1v, ckeys(mv)))
        base_v = np.array([m[graph.BASE] for m in mv], dtype=np.float64)
        name_t = keys.best_threshold(yv, keys.one_target(base_v, ckeys(mv)))
        result["validation"] = {
            "pairs": len(mv), "keys among candidates": int(yv.sum()),
            "over candidates": pr(keys.at_threshold(yv, kv, threshold)),
            "over every declared key": targets.every_key(schemas, "validation", mv, kv, threshold),
            "the paper's task, column and referred column": targets.forced_choice(schemas, "validation", mv, rawv,
                                                                                  referred),
            "the paper's task, column only": widen.forced_choice(schemas, "validation", mv, rawv),
            "threshold grid, every declared key": curve(mv, kv, declared_v),
            "first stage alone, threshold": first_t, "name score alone, threshold": name_t,
        }
        print(f"{name}: validation {time.time() - t0:.0f} s", flush=True)

    if "test" in parts:
        s1t, rawt, kt = predict(xt, mt)
        yt, rules_t = labels(mt), np.array([m[graph.RULES] for m in mt], dtype=bool)
        repos_t = np.array([m[graph.REPO] for m in mt])
        predicted = kt >= threshold
        rng = np.random.default_rng(widen.SEED)
        test = {}
        subsets = {"all": np.ones_like(yt), "novel": np.array([m[graph.NOVEL] for m in mt], dtype=bool),
                   "complete": graph.complete_mask(mt)}
        for sub, mask in subsets.items():
            low, high = bootstrap(yt[mask], predicted[mask], rules_t[mask], repos_t[mask], rng)
            test[sub] = {"pairs": int(mask.sum()), "keys": int(yt[mask].sum()),
                         "rules": pr(prf(*counts(yt[mask], rules_t[mask]))),
                         "model": pr(prf(*counts(yt[mask], predicted[mask]))), "f1_minus_rules_95": [low, high]}
        repo_of = {s["id"]: s["repo"] for s in schemas}
        _, _, kh = predict(xh, mh)
        yh, rules_h = labels(mh), np.array([m[graph.RULES] for m in mh], dtype=bool)
        result["test"] = {
            "over candidates": test,
            "over every declared key": targets.every_key(schemas, "test", mt, kt, threshold),
            "over every declared key, by schema": {i: [repo_of[i], *c] for i, c in
                                                   per_schema(mt, kt, threshold, schemas, "test").items()},
            "the paper's task, column and referred column": targets.forced_choice(schemas, "test", mt, rawt, referred),
            "first stage alone, over every declared key": targets.every_key(
                schemas, "test", mt, keys.one_target(s1t, ckeys(mt)), first_t),
            "name score alone, over every declared key": targets.every_key(
                schemas, "test", mt, keys.one_target(np.array([m[graph.BASE] for m in mt], dtype=np.float64),
                                                     ckeys(mt)), name_t),
            "hard": {"keys": int(yh.sum()), "rules": pr(prf(*counts(yh, rules_h))),
                     "model": pr(prf(*counts(yh, kh >= threshold)))},
        }
        print(f"{name}: test {time.time() - t0:.0f} s", flush=True)

    if "external" in parts:
        d = out / "external"
        calls = []

        def build(job):
            r = widen.build((*job, variant))
            calls.append(r)
            return r

        ext.score(models, d, build=build, suffix="-" + name)
        x, meta = calls[0]  # the open task's: every database whole
        _, raw, kept = predict(x, meta)
        schemas_e = [json.loads(line) for line in open(d / "hidden.jsonl", encoding="utf-8")]
        src = {s["id"]: s["source"] for s in schemas_e}
        grid = {}
        for which in ("spider", "bird", "ctu", "all"):
            mask = np.array([which in ("all", src[m[graph.SID]]) for m in meta])
            n = sum(len(s["fk"]) for s in schemas_e if which in ("all", s["source"]))
            grid[which] = curve([m for m, k in zip(meta, mask) if k], kept[mask], n)
        # Related or not, as round 1p's separate.py: the best score of any
        # column of the first table into the second
        best = defaultdict(float)
        for i, m in enumerate(meta):
            k = (m[graph.SID], m[graph.TABLE], m[graph.TARGET])
            best[k] = max(best[k], float(raw[i]))
        rows = [json.loads(line) for line in open(d / "t5-negatives.jsonl", encoding="utf-8")]
        separate = {}
        for which in ("spider", "bird", "ctu", "all"):
            r = [x for x in rows if which in ("all", x["source"])]
            y = np.array([x["related"] for x in r], dtype=bool)
            ours = np.array([best[(f"{x['source']}/{x['db']}", x["table"], x["other"])] for x in r])
            cut = np.sort(ours[y])[::-1][int(np.ceil(0.8 * y.sum())) - 1]
            separate[which] = {"auc": float(roc_auc_score(y, ours)), "average precision": float(
                average_precision_score(y, ours)), "unrelated accepted at 80 % of related": float(
                (ours[~y] >= cut).mean()) if cut > ours.min() else 1.0}
        result["external"] = {"threshold grid, every declared key": grid, "related or not": separate,
                              "pairs": len(meta)}
        print(f"{name}: external {time.time() - t0:.0f} s", flush=True)

    result["seconds"] = round(time.time() - t0)
    (out / "scored").mkdir(parents=True, exist_ok=True)
    (out / "scored" / f"{name}.json").write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")


def measure(data: Path, round_1p: Path, out: Path, names: list[str]) -> None:
    d = out / "external"
    d.mkdir(parents=True, exist_ok=True)
    for f in ("hidden.jsonl", "guesses.jsonl", "bench.jsonl", "t5-negatives.jsonl"):
        if not (d / f).exists():
            shutil.copyfile(round_1p / f, d / f)
    (out / "logs").mkdir(exist_ok=True)
    all_ = scorings(round_1p, out)
    for name in names or list(all_):
        _, _, order, hseed, _ = all_[name]
        env = {**os.environ, "LEMMESEE_GRAMS": order}
        if hseed is None:
            flags = ["-I"]
            env.pop("PYTHONHASHSEED", None)
        else:
            # -I would ignore PYTHONHASHSEED; the workers inherit both
            flags = ["-P", "-s"]
            env["PYTHONHASHSEED"] = str(hseed)
        cmd = [sys.executable, *flags, str(Path(__file__).resolve()), "score", str(data), str(round_1p), str(out), name]
        t0 = time.time()
        with open(out / "logs" / f"{name}.log", "w", encoding="utf-8") as log:
            code = subprocess.call(cmd, env=env, stdout=log, stderr=subprocess.STDOUT)
        print(f"{name}: exit {code}, {time.time() - t0:.0f} s", flush=True)
        if code:
            raise SystemExit(code)
    summarize(round_1p, out)


# --- summary -------------------------------------------------------------------

def resampled(per: np.ndarray, idx: np.ndarray) -> np.ndarray:
    """F1 of each resample: per is (units, 3) of tp, fp, fn."""
    s = per[idx].sum(axis=1)
    tp, fp, fn = s[:, 0], s[:, 1], s[:, 2]
    p = np.where(tp + fp > 0, tp / np.maximum(tp + fp, 1), 0.0)
    r = np.where(tp + fn > 0, tp / np.maximum(tp + fn, 1), 0.0)
    return np.stack([p, r, np.where(p + r > 0, 2 * p * r / np.maximum(p + r, 1e-12), 0.0)], axis=1)


def interval(v: np.ndarray) -> list[float]:
    return [float(np.percentile(v, 2.5)), float(np.percentile(v, 97.5))]


def external_counts(out: Path, name: str, schemas: list[dict]) -> dict:
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["fk"]}
    preds = defaultdict(set)
    for line in open(out / "external" / f"open-predictions-{name}.jsonl", encoding="utf-8"):
        sid, t, c, r, _ = json.loads(line)
        preds[sid].add((t, c, r))
    return {i: [len(truth[i] & preds[i]), len(preds[i] - truth[i]), len(truth[i] - preds[i])] for i in truth}


def summarize(round_1p: Path, out: Path) -> None:
    all_ = scorings(round_1p, out)
    done = [n for n in all_ if (out / "scored" / f"{n}.json").exists()]
    schemas = [json.loads(line) for line in open(out / "external" / "hidden.jsonl", encoding="utf-8")]
    source = {s["id"]: s["source"] for s in schemas}
    novel = {s["id"]: s["novel"] for s in schemas}
    ids = sorted(s["id"] for s in schemas if s["fk"])
    ext = {n: external_counts(out, n, schemas) for n in done if "external" in all_[n][4]}
    rng = np.random.default_rng(EXTERNAL_SEED)
    subsets = {}
    for which in ("spider", "bird", "ctu", "all"):
        for sub in ("all", "novel"):
            sel = [i for i in ids if which in ("all", source[i]) and (sub == "all" or novel[i])]
            subsets[f"{which}, {sub}"] = (sel, rng.integers(0, len(sel), (1000, len(sel))))
    summary = {"external, over every declared key": {}, "external, differences": {}, "test, differences": {},
               "the paper's task, differences": {}}
    for n, c in ext.items():
        entry = {}
        for k, (sel, idx) in subsets.items():
            per = np.array([c[i] for i in sel], dtype=np.int64)
            p, r, f = prf(*per.sum(axis=0))
            boot = resampled(per, idx)
            macro = float(np.mean([prf(*c[i])[2] for i in sel]))
            entry[k] = {"databases": len(sel), "keys": int(per[:, 0].sum() + per[:, 2].sum()),
                        "precision": p, "recall": r, "f1": f, "f1_95": interval(boot[:, 2]),
                        "macro f1 over databases": macro}
        summary["external, over every declared key"][n] = entry
    # The refits, and the saved models scored sorted, against round 1p's
    # `both` as it was scored, and against itself sorted
    pairs = [(a, b) for a in ("both", "both-seed-1", "both-seed-2", "1p-both-sorted", "base", "1b-base-sorted")
             for b in ("1p-both-process-0", "1p-both-sorted", "1b-base-process-0") if a in ext and b in ext and a != b]
    for a, b in pairs:
        entry = {}
        for k, (sel, idx) in subsets.items():
            pa = np.array([ext[a][i] for i in sel], dtype=np.int64)
            pb = np.array([ext[b][i] for i in sel], dtype=np.int64)
            da, db = resampled(pa, idx), resampled(pb, idx)
            point = np.array(prf(*pa.sum(axis=0))) - np.array(prf(*pb.sum(axis=0)))
            entry[k] = {what: {"point": float(point[j]), "95": interval(da[:, j] - db[:, j])}
                        for j, what in enumerate(("precision", "recall", "f1"))}
        summary["external, differences"][f"{a} minus {b}"] = entry
    # The paper's task, pair by pair, resampling databases
    bench = {}
    for n in ext:
        rows = [json.loads(line) for line in open(out / "external" / f"bench-outcomes-{n}.jsonl", encoding="utf-8")]
        bench[n] = [r for r in rows if not r["occurs_in_perm"] and not r["too_large"]]
    for a, b in pairs:
        entry = {}
        for which in ("spider", "bird", "ctu"):
            ra = [r for r in bench[a] if r["source"] == which]
            rb = [r for r in bench[b] if r["source"] == which]
            assert [(r["db"], r["table"], r["foreign_table"]) for r in ra] == \
                   [(r["db"], r["table"], r["foreign_table"]) for r in rb]
            dbs = sorted({r["db"] for r in ra})
            at = {x: k for k, x in enumerate(dbs)}
            per = np.zeros((len(dbs), 3))
            for x, y in zip(ra, rb):
                per[at[x["db"]]] += [x["hit whole database"], y["hit whole database"], 1]
            idx = np.random.default_rng(EXTERNAL_SEED).integers(0, len(dbs), (1000, len(dbs)))
            s = per[idx].sum(axis=1)
            entry[which] = {"pairs": len(ra), "point": float((per[:, 0].sum() - per[:, 1].sum()) / per[:, 2].sum()),
                            "95": interval((s[:, 0] - s[:, 1]) / s[:, 2]),
                            "right in one only": [sum(1 for x, y in zip(ra, rb) if x["hit whole database"] and not
                                                      y["hit whole database"]),
                                                  sum(1 for x, y in zip(ra, rb) if y["hit whole database"] and not
                                                      x["hit whole database"])]}
        summary["the paper's task, differences"][f"{a} minus {b}"] = entry
    # SchemaPile-Perm's test, resampling repositories
    scored = {n: json.load(open(out / "scored" / f"{n}.json", encoding="utf-8")) for n in done}
    with_test = [n for n in done if "test" in scored[n]]
    for a in with_test:
        for b in with_test:
            if a == b or (a, b) not in [(x, y) for x, y in pairs] + [("both", "1b-base-process-0"),
                                                                       ("both", "base")]:
                continue
            ca, cb = scored[a]["test"]["over every declared key, by schema"], \
                scored[b]["test"]["over every declared key, by schema"]
            common = sorted(set(ca) & set(cb))
            repos = sorted({ca[i][0] for i in common})
            at = {r: k for k, r in enumerate(repos)}
            pa, pb = np.zeros((len(repos), 3)), np.zeros((len(repos), 3))
            for i in common:
                pa[at[ca[i][0]]] += ca[i][1:]
                pb[at[cb[i][0]]] += cb[i][1:]
            idx = np.random.default_rng(widen.SEED).integers(0, len(repos), (1000, len(repos)))
            da, db = resampled(pa, idx), resampled(pb, idx)
            point = np.array(prf(*pa.sum(axis=0))) - np.array(prf(*pb.sum(axis=0)))
            summary["test, differences"][f"{a} minus {b}"] = {
                what: {"point": float(point[j]), "95": interval(da[:, j] - db[:, j])}
                for j, what in enumerate(("precision", "recall", "f1"))}
    # How far round 1p's model moves between scorings in process order
    proc = [f"1p-both-process-{k}" for k in range(5) if f"1p-both-process-{k}" in ext]
    if proc:
        summary["round 1p's both in process order, five hash seeds"] = {
            k: [min(summary["external, over every declared key"][n][k]["f1"] for n in proc),
                max(summary["external, over every declared key"][n][k]["f1"] for n in proc)]
            for k in subsets}
    (out / "summary.json").write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    print(json.dumps({k: {n: {s: round(e["f1"], 4) for s, e in v.items() if s.endswith(", all")}
                          for n, v in summary["external, over every declared key"].items()}
                      for k in ["x"]}, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    a = sys.argv
    if a[1] == "check":
        check(Path(a[2]), Path(a[3]), Path(a[4]))
    elif a[1] == "fit":
        fit(Path(a[2]), Path(a[3]), a[4:] or list(FITS))
    elif a[1] == "score":
        score(Path(a[2]), Path(a[3]), Path(a[4]), a[5])
    elif a[1] == "measure":
        measure(Path(a[2]), Path(a[3]), Path(a[4]), a[5:])
    else:
        summarize(Path(a[2]), Path(a[3]))
