"""Round 1t: the key model fitted to read names in more than one house style.

    python -I fit.py fit <name> [<name> ...]
    python -I fit.py score <name> [<name> ...]
    python -I fit.py summarize

Paths are fixed, under the repository's ignored corpus: SchemaPile as round
1b prepared it (corpus/round-1b), the teachers' schemas (corpus/round-1t/
teacher/generated.jsonl), Spider, BIRD and CTU as round 1s read them
(corpus/round-1s/external), LiveSQLBench as round 1s read it once
(corpus/round-1s/heldout, development data since 8 November 2026). Models go
to corpus/round-1t/models/<name>/, scorings to corpus/round-1t/scored/.

FITS name what each fit adds to round 1s's `both`:

    cues                   version 5's cues (cues.py)
    restyled               half of SchemaPile's training schemas renamed into a
                           drawn house style (restyle.py), and every validation
                           schema renamed beside its original
    cues-restyled          both
    cues-restyled-taught   both, and the teachers' training and validation schemas

Every fit is round 1s's procedure, widen.fit: features once, five folds by
repository for the first stage's scores, the second stage on them, the
threshold the validation split's best F1 over candidates; seed 20261027, the
trigrams sorted. `1s` scores round 1s's refit `both` as it was saved.

`score` measures each model over every declared key on each set: SchemaPile's
test split as it is and renamed (every test schema, its own draw), the
teachers' test domains, Spider, BIRD, CTU and LiveSQLBench; per database
true, false and missed keys, F1 with a 95 % interval from 1,000 resamples of
databases, the macro F1, and each minus round 1s's on the same resamples.
"""

import json
import random
import sys
import time
from collections import defaultdict
from functools import lru_cache
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import cues  # noqa: E402  (first: it puts round 1s, 1p and 1b on the path and sorts the trigrams)
import letters  # noqa: E402

import numpy as np  # noqa: E402

import graph  # noqa: E402
import keys  # noqa: E402
import restyle  # noqa: E402
import stack  # noqa: E402

# Fewer workers than cores: each holds its features' memory while the
# graphics card fits, and fifteen of them took 10 GB on 8 November
graph.WORKERS = 8
import widen  # noqa: E402
from refit import EXTERNAL_SEED, interval, resampled  # noqa: E402
from train import prf  # noqa: E402

ROOT = HERE.parent / "corpus"
OUT = ROOT / "round-1t"
SEED = 20261027
RESTYLE_SEED, TEST_RESTYLE_SEED = 20261108, 20261109
RESTYLED_SHARE = 0.5

FITS = {
    "cues": ("cues", False, False),
    "restyled": ("both", True, False),
    "cues-restyled": ("cues", True, False),
    "cues-restyled-taught": ("cues", True, True),
    # added during the night of 8 November, after the development reading
    "six": ("six", False, False),
    "six-restyled": ("six", True, False),
    # reliable negatives: a pair the rules name as a key but the schema does
    # not declare is left out of training, as likely undeclared (PU learning)
    "six-restyled-pu": ("six", True, False, {"reliable negatives": True}),
    # version 7: names read as words, split and expanded (words7.py); the
    # training renamed with the dictionary's shortenings too
    "seven-restyled": ("seven", True, False, {"dictionary shortening": True}),
    "seven-restyled-pu": ("seven", True, False, {"dictionary shortening": True, "reliable negatives": True}),
    # the graph stage read against its own schema, fitted on shifted scores (stack.py)
    "seven-relative": ("seven", True, False, {"dictionary shortening": True, "relative graph": True,
                                              "shift-aware": True}),
    "cues-relative": ("cues", True, False, {"relative graph": True, "shift-aware": True}),
    # version 8: version 7 and the related entities (words7.add_related)
    "eight-restyled": ("eight", True, False, {"dictionary shortening": True}),
    "eight-relative": ("eight", True, False, {"dictionary shortening": True, "relative graph": True,
                                              "shift-aware": True}),
    # rename.py: 400 real training schemas renamed by Haiku, against the same 400 restyled once more
    "cues-restyled-llm": ("cues", True, False, {"llm renamed": True}),
    "cues-restyled-more": ("cues", True, False, {"llm control": True}),
    "seven-relative-llm": ("seven", True, False, {"dictionary shortening": True, "relative graph": True,
                                                  "shift-aware": True, "llm renamed": True}),
    # the morning of 8 November: the second stage's leaves bounded (BOUNDED), and
    # the graph stage read against the schema's columns named alike (style.py)
    "seven-bounded": ("seven", True, False, {"dictionary shortening": True, "bounded second": True}),
    "seven-relative-bounded": ("seven", True, False, {"dictionary shortening": True, "relative graph": True,
                                                      "shift-aware": True, "bounded second": True}),
    "seven-style": ("seven", True, False, {"dictionary shortening": True, "style graph": True,
                                           "shift-aware": True, "bounded second": True}),
    # 9 November, the mended fit (README's last entries): every fix together, not a search
    "nine-mended": ("nine", True, False, {"dictionary shortening": True, "bounded second": True,
                                          "hard negatives": True, "key word first": True}),
    # a name of one word has no lead word (style.py, `style2`)
    "seven-style2": ("seven", True, False, {"dictionary shortening": True, "style graph": "style2",
                                            "shift-aware": True, "bounded second": True}),
}
# A second stage whose leaves cannot run away: seven-restyled's held 43
# leaves over 50, up to 15 832, drawing pairs its first stage scored under
# 0.001 (each leaf at most 0.05 x 20 = 1.0, a leaf's hessian at least 1)
BOUNDED = dict(max_delta_step=20.0, min_child_weight=1.0, reg_lambda=1.0)
MODELS = {"1s": (ROOT / "round-1s" / "models" / "both", "both")}
MODELS.update({n: (OUT / "models" / n, f[0]) for n, f in FITS.items()})


HARD_KEY_WORDS = {"id", "key", "code", "cod", "codigo", "no", "num", "nr", "fk", "pk", "ref", "uid", "uuid", "link", "ptr"}


@lru_cache(maxsize=1_000_000)
def looks_like(column: str, target: str) -> bool:
    """An ordinary column named as a shortening of the target or sharing its
    first three letters: `used` and `users`, `puntos` and `productos`."""
    from train import singular, tokens
    toks = tokens(column)
    if not toks or toks[0] in HARD_KEY_WORDS or toks[-1] in HARD_KEY_WORDS:
        return False
    c = "".join(toks)
    t = singular("".join(tokens(target)))
    if len(c) < 3 or c == t or c == t + "s":
        return False
    if len(_common(c, t)) >= 3:
        return True
    it = iter(t)
    return c[0] == t[:1] and len(c) < len(t) and all(ch in it for ch in c)


def _common(a: str, b: str) -> str:
    n = 0
    while n < min(len(a), len(b)) and a[n] == b[n]:
        n += 1
    return a[:n]


def fit_pairs(pool, schemas, guesses, variant, opts):
    """widen.fit, with the options a FITS entry may carry."""
    import lightgbm as lgb  # noqa: E402
    from sklearn.metrics import average_precision_score  # noqa: E402
    from train import NEGATIVES  # noqa: E402
    import stack  # noqa: E402
    if not (opts.get("reliable negatives") or opts.get("relative graph") or opts.get("shift-aware")
            or opts.get("bounded second") or opts.get("style graph") or opts.get("hard negatives")):
        return widen.fit(pool, schemas, guesses, variant)
    gf = stack.graph_features if opts.get("relative graph") else graph.graph_features
    if opts.get("style graph"):
        import style  # noqa: E402
        gf = style.graph_features2 if opts["style graph"] == "style2" else style.graph_features
    t0 = time.time()
    clock = lambda what: print(f"{variant}: {what}: {time.time() - t0:.0f} s", flush=True)  # noqa: E731
    x, meta = cues.build_all(pool, schemas, "train", guesses, variant)
    xv, meta_v = cues.build_all(pool, schemas, "validation", guesses, variant)
    clock(f"features, {len(meta)} training pairs")
    y = np.array([m[graph.LABEL] for m in meta], dtype=bool)
    folds = np.array([graph.fold_of(m[graph.REPO]) for m in meta])
    sample = y | (np.random.default_rng(SEED).random(len(y)) < NEGATIVES)
    left_out = 0
    hard_added = 0
    if opts.get("hard negatives"):
        # every ordinary column sharing a prefix or a shortening with its
        # candidate target's name, no key of the schema, kept, not sampled
        keyed = {(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta if m[graph.LABEL]}
        hard = np.fromiter((not m[graph.LABEL] and (m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) not in keyed
                            and looks_like(m[graph.COLUMN], m[graph.TARGET]) for m in meta), dtype=bool, count=len(meta))
        hard_added = int((hard & ~sample).sum())
        sample |= hard
        clock(f"hard negatives: {int(hard.sum())}, {hard_added} not sampled before")
    if opts.get("reliable negatives"):
        ruled = np.array([m[graph.RULES] for m in meta], dtype=bool)
        left_out = int((sample & ruled & ~y).sum())
        sample &= ~(ruled & ~y)
    oof = np.zeros(len(y))
    renamed_ids = {s["id"] for s in schemas if s.get("restyled") and s["split"] == "train"}
    renamed = np.array([m[graph.SID] in renamed_ids for m in meta], dtype=bool)
    for k in range(graph.FOLDS):
        rows = np.flatnonzero(sample & (folds != k))
        model = graph.first_model(True)
        model.fit(x[rows], y[rows])
        held = np.flatnonzero(folds == k)
        oof[held] = graph.first_scores(model, x[held], True)
        if opts.get("shift-aware") and renamed.any():
            # the renamed schemas scored by a first stage that saw none of them
            rows = np.flatnonzero(sample & (folds != k) & ~renamed)
            model = graph.first_model(True)
            model.fit(x[rows], y[rows])
            held = np.flatnonzero((folds == k) & renamed)
            oof[held] = graph.first_scores(model, x[held], True)
    clock("five folds")
    whole = graph.first_model(True)
    rows = np.flatnonzero(sample)
    whole.fit(x[rows], y[rows])
    del x
    second = lgb.LGBMClassifier(**graph.SECOND, **(BOUNDED if opts.get("bounded second") else {}))
    second.fit(gf(meta, oof)[sample], y[sample])
    clock("both stages")
    sv = second.predict_proba(gf(meta_v, graph.first_scores(whole, xv, True)))[:, 1]
    yv = np.array([m[graph.LABEL] for m in meta_v], dtype=bool)
    kept = keys.one_target(sv, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta_v])
    threshold = keys.best_threshold(yv, kept)
    p, r, f = keys.at_threshold(yv, kept, threshold)
    result = {"variant": variant, "options": opts, "training pairs": len(meta), "negatives left out": left_out,
              "hard negatives added": hard_added,
              "validation pairs": len(meta_v), "validation keys among candidates": int(yv.sum()),
              "threshold": threshold, "validation": {"precision": p, "recall": r, "f1": f,
                                                     "average_precision": float(average_precision_score(yv, kept))},
              "seconds": round(time.time() - t0)}
    if opts.get("relative graph"):
        result["graph"] = "relative"
    if opts.get("style graph"):
        result["graph"] = "style2" if opts["style graph"] == "style2" else "style"
    return result, whole, second


def restyled(s: dict, seed: int, suffix: str = ""):
    r = restyle.restyle(s, random.Random(f"{seed}:{s['id']}"))
    if r is None:
        return None
    return {**r, "id": s["id"] + suffix}


def training_data(with_restyle: bool, with_teachers: bool, opts: dict | None = None):
    opts = opts or {}
    schemas, guesses = keys.load(ROOT / "round-1b")
    counts = defaultdict(int)
    if with_restyle:
        out = []
        for s in schemas:
            if s["split"] == "train" and s["fk"]:
                if random.Random(f"{RESTYLE_SEED}:share:{s['id']}").random() < RESTYLED_SHARE:
                    r = restyled(s, RESTYLE_SEED)
                    counts["train renamed" if r else "train left, the style did not fit"] += 1
                    out.append(r or s)
                    continue
            out.append(s)
            if s["split"] == "validation" and s["fk"]:
                r = restyled(s, RESTYLE_SEED, "~restyled")
                if r:
                    out.append(r)
                    counts["validation renamed beside"] += 1
        schemas = out
    if opts.get("llm renamed") or opts.get("llm control"):
        # rename.py's schemas: a language model's names (or, for the control,
        # the same originals drawn into restyle.py's styles once more)
        renamed = [json.loads(line) for line in open(OUT / "renamed-llm.jsonl", encoding="utf-8")]
        if opts.get("llm control"):
            by_id = {s["id"]: s for s in schemas}
            renamed = [r for r in (restyled(by_id[x["id"][:-len("~llm")]], RESTYLE_SEED + 7, "~more")
                                   for x in renamed) if r]
        for r in renamed:
            r["split"] = "train"
        counts["language-model renamed" if opts.get("llm renamed") else "restyled once more"] = len(renamed)
        schemas = schemas + renamed
    if with_teachers:
        taught = [json.loads(line) for line in open(OUT / "teacher" / "generated.jsonl", encoding="utf-8")]
        taught = [s for s in taught if s["split"] in ("train", "validation")]
        counts["teacher schemas"] = len(taught)
        schemas = schemas + taught
    return schemas, guesses, dict(counts)


def fit(names: list[str]) -> None:
    assert letters.is_sorted()
    widen.build_all = cues.build_all  # widen.fit builds its pairs through this name
    with letters.pool(graph.WORKERS) as pool:
        for name in names:
            variant, with_restyle, with_teachers, *rest = FITS[name]
            opts = rest[0] if rest else {}
            t0 = time.time()
            restyle.DICTIONARY = bool(opts.get("dictionary shortening"))
            restyle.KEY_FIRST = bool(opts.get("key word first"))
            schemas, guesses, counts = training_data(with_restyle, with_teachers, opts)
            restyle.DICTIONARY = False
            restyle.KEY_FIRST = False
            print(f"{name}: {len(schemas)} schemas {counts} ({time.time() - t0:.0f} s)", flush=True)
            result, whole, second = fit_pairs(pool, schemas, guesses, variant, opts)
            result.update(fit=name, seed=SEED, grams="sorted", data=counts)
            d = OUT / "models" / name
            d.mkdir(parents=True, exist_ok=True)
            whole.save_model(str(d / "keys-first.ubj"))
            second.booster_.save_model(str(d / "keys-second.txt"))
            (d / "graph-final.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
            print(json.dumps(result), flush=True)
            del schemas, whole, second


def sets():
    """name: (schemas, split, guesses)."""
    sp, sp_guesses = keys.load(ROOT / "round-1b")
    test = [s for s in sp if s["split"] == "test" and s["fk"]]
    renamed = [r for r in (restyled(s, TEST_RESTYLE_SEED) for s in test) if r]
    out = {"schemapile test": (test, "test", sp_guesses),
           "schemapile test, renamed": (renamed, "test", {})}
    taught = [json.loads(line) for line in open(OUT / "teacher" / "generated.jsonl", encoding="utf-8")]
    out["teachers' test domains"] = ([s for s in taught if s["split"] == "test"], "test", {})
    ext, ext_guesses = keys.load(ROOT / "round-1s" / "external")
    for src in ("spider", "bird", "ctu"):
        out[src] = ([s for s in ext if s["source"] == src], "external", ext_guesses)
    lsb, lsb_guesses = keys.load(ROOT / "round-1s" / "heldout")
    out["livesqlbench (development)"] = (lsb, "external", lsb_guesses)
    return out


def per_database(schemas, split, meta, kept, threshold):
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["split"] == split and s["fk"]}
    preds = defaultdict(set)
    for i in np.flatnonzero(kept >= threshold):
        m = meta[i]
        preds[m[graph.SID]].add((m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]))
    ids = sorted(truth)
    return ids, np.array([[len(truth[i] & preds[i]), len(preds[i] - truth[i]), len(truth[i] - preds[i])]
                          for i in ids], dtype=np.int64)


def score(names: list[str]) -> None:
    assert letters.is_sorted()
    data = sets()
    (OUT / "scored").mkdir(parents=True, exist_ok=True)
    with letters.pool(graph.WORKERS) as pool:
        built = {}
        for name in names:
            d, variant = MODELS[name]
            first, second = widen.load_models(d)
            threshold = json.load(open(d / "graph-final.json", encoding="utf-8"))["threshold"]
            result = {"model": name, "threshold": threshold, "sets": {}}
            t0 = time.time()
            for set_name, (schemas, split, guesses) in data.items():
                key = (set_name, variant)
                if key not in built:
                    built = {k: v for k, v in built.items() if k[1] == variant}  # keep one variant's in memory
                    built[key] = cues.build_all(pool, schemas, split, guesses, variant)
                x, meta = built[key]
                raw = second.predict(stack.of(json.load(open(d / 'graph-final.json', encoding='utf-8')))(meta, first.predict_proba(x)[:, 1]))
                kept = keys.one_target(raw, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
                ids, per = per_database(schemas, split, meta, kept, threshold)
                _, at_half = per_database(schemas, split, meta, kept, 0.5)
                result["sets"][set_name] = {"databases": ids, "per database": per.tolist(),
                                            "per database at 0.5": at_half.tolist()}
                p, r, f = prf(*per.sum(axis=0))
                print(f"{name:22} {set_name:30} P {p:.3f} R {r:.3f} F1 {f:.3f}  ({time.time() - t0:.0f} s)", flush=True)
            (OUT / "scored" / f"{name}.json").write_text(json.dumps(result) + "\n", encoding="utf-8")


def summarize() -> None:
    scored = {p.stem: json.load(open(p, encoding="utf-8")) for p in sorted((OUT / "scored").glob("*.json"))}
    base = scored["1s"]
    out = {}
    pooled = ("spider", "bird", "ctu", "livesqlbench (development)")
    for r in scored.values():
        parts = [r["sets"][k] for k in pooled if k in r["sets"]]
        if len(parts) == len(pooled):
            r["sets"]["development, pooled"] = {
                "databases": sum((p["databases"] for p in parts), []),
                "per database": sum((p["per database"] for p in parts), []),
                "per database at 0.5": sum((p["per database at 0.5"] for p in parts), [])}
    for name, r in scored.items():
        out[name] = {"threshold": r["threshold"]}
        for set_name, s in r["sets"].items():
            per = np.array(s["per database"])
            idx = np.random.default_rng(EXTERNAL_SEED).integers(0, len(per), (1000, len(per)))
            boot = resampled(per, idx)
            p, rr, f = prf(*per.sum(axis=0))
            half = prf(*np.array(s["per database at 0.5"]).sum(axis=0))
            e = {"databases": len(per), "keys": int(per[:, 0].sum() + per[:, 2].sum()),
                 "precision": p, "recall": rr, "f1": f, "f1_95": interval(boot[:, 2]),
                 "macro f1": float(np.mean([prf(*c)[2] for c in per])), "f1 at 0.5": half[2]}
            b = base["sets"].get(set_name)
            if b and name != "1s" and b["databases"] == s["databases"]:
                bper = np.array(b["per database"])
                bb = resampled(bper, idx)
                e["minus 1s, f1"] = f - prf(*bper.sum(axis=0))[2]
                e["minus 1s, f1_95"] = interval(boot[:, 2] - bb[:, 2])
            out[name][set_name] = e
    # The README's rule: the best pooled development F1 among the fits whose
    # SchemaPile test F1 is within 0.01 of round 1s's
    floor = out["1s"]["schemapile test"]["f1"] - 0.01
    eligible = [n for n in out if n != "1s" and out[n]["schemapile test"]["f1"] >= floor]
    out["chosen"] = max(eligible, key=lambda n: out[n]["development, pooled"]["f1"]) if eligible else None
    (OUT / "summary.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print("chosen:", out["chosen"])
    out.pop("chosen")
    names = list(out)
    sets_ = [k for k in out[names[0]] if k != "threshold"]
    print(f"{'':30}" + "".join(f"{n[:20]:>22}" for n in names))
    for s in sets_:
        row = f"{s[:30]:30}"
        for n in names:
            e = out[n].get(s)
            row += f"{e['f1']:>10.3f} [{e['f1_95'][0]:.2f},{e['f1_95'][1]:.2f}]" if e else f"{'':>22}"
        print(row)


HELDOUT = {"github 2025": OUT / "heldout-github", "beaver": OUT / "heldout-beaver",
           # the 177 the rule kept beyond the first 80, written on the morning of 8 November
           "github 2025, rest": OUT / "heldout-github-rest",
           # 100 schemas in another house style, written on 9 November (github.py parse-style)
           "github style": OUT / "heldout-github-style",
           # the same 100, their primary keys read by the fixed parser: a corrected reading of a seen set
           "github style, fixed": OUT / "heldout-github-style-fixed",
           # and their types in the training's families as well
           "github style, typed": OUT / "heldout-github-style-typed",
           # the fresh clean set, its rule written at 14:06 UTC on 9 November (github.py parse-style fresh)
           "github fresh": OUT / "heldout-github-style-fresh"}


def freeze(names: list[str]) -> None:
    """The models to be read on the held-out sets, by hash, before they are read."""
    import hashlib
    from datetime import datetime, timezone
    path = OUT / "frozen.json"
    if path.exists():
        raise SystemExit("already frozen")
    frozen = {"frozen at": datetime.now(timezone.utc).isoformat(timespec="seconds"), "models": {}}
    for n in names:
        d, variant = MODELS[n]
        frozen["models"][n] = {"variant": variant, **{f: hashlib.sha256((d / f).read_bytes()).hexdigest()
                                                     for f in ("keys-first.ubj", "keys-second.txt", "graph-final.json")}}
    for s, d in HELDOUT.items():
        if (d / "hidden.jsonl").exists():
            frozen[s] = hashlib.sha256((d / "hidden.jsonl").read_bytes()).hexdigest()
    path.write_text(json.dumps(frozen, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(frozen, indent=1))


def freeze_set(name: str) -> None:
    """A held-out set written after the models were frozen, hashed before it is read."""
    import hashlib
    from datetime import datetime, timezone
    path = OUT / "frozen.json"
    frozen = json.load(open(path, encoding="utf-8"))
    if name in frozen:
        raise SystemExit("already frozen")
    frozen[name] = hashlib.sha256((HELDOUT[name] / "hidden.jsonl").read_bytes()).hexdigest()
    frozen[f"{name}, frozen at"] = datetime.now(timezone.utc).isoformat(timespec="seconds")
    path.write_text(json.dumps(frozen, indent=1) + "\n", encoding="utf-8")
    print(name, frozen[name])


def freeze_final(names: list[str]) -> None:
    """The night's models, frozen by hash before the GitHub set is read;
    the set's hash as frozen at 00:34 UTC, checked unchanged."""
    import hashlib
    from datetime import datetime, timezone
    path = OUT / "frozen-final.json"
    if path.exists():
        raise SystemExit("already frozen")
    first = json.load(open(OUT / "frozen.json", encoding="utf-8"))
    gh = hashlib.sha256((HELDOUT["github 2025"] / "hidden.jsonl").read_bytes()).hexdigest()
    assert gh == first["github 2025"], "the GitHub set changed since it was frozen"
    frozen = {"frozen at": datetime.now(timezone.utc).isoformat(timespec="seconds"), "models": {},
              "github 2025": gh, "github 2025, frozen at": first["github 2025, frozen at"],
              "chosen by the rule": names[0]}
    for n in names:
        d, variant = MODELS[n]
        frozen["models"][n] = {"variant": variant, **{f: hashlib.sha256((d / f).read_bytes()).hexdigest()
                                                     for f in ("keys-first.ubj", "keys-second.txt", "graph-final.json")}}
    path.write_text(json.dumps(frozen, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(frozen, indent=1))


def freeze_rest(names: list[str]) -> None:
    """The models read on the second GitHub set, frozen by hash before it is
    read; a blend (`a+b`, blend.py) by its parts and its validation threshold;
    the set's hash as frozen at 06:52 UTC, checked unchanged."""
    import hashlib
    from datetime import datetime, timezone
    path = OUT / "frozen-rest.json"
    if path.exists():
        raise SystemExit("already frozen")
    first = json.load(open(OUT / "frozen-rest-set.json", encoding="utf-8"))
    which = "github 2025, rest"
    h = hashlib.sha256((HELDOUT[which] / "hidden.jsonl").read_bytes()).hexdigest()
    assert h == first[which], "the second GitHub set changed since it was frozen"
    frozen = {"frozen at": datetime.now(timezone.utc).isoformat(timespec="seconds"), "models": {},
              which: h, f"{which}, frozen at": first[f"{which}, frozen at"], "chosen by the rule": names[0]}
    for n in names:
        parts = n.split("+")
        rec = {"parts": {}}
        for part in parts:
            d, variant = MODELS[part]
            rec["parts"][part] = {"variant": variant, **{f: hashlib.sha256((d / f).read_bytes()).hexdigest()
                                                         for f in ("keys-first.ubj", "keys-second.txt",
                                                                   "graph-final.json")}}
        if len(parts) > 1:
            rec["threshold"] = json.load(open(OUT / "scored" / f"{n}.json", encoding="utf-8"))["threshold"]
        frozen["models"][n] = rec
    path.write_text(json.dumps(frozen, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(frozen, indent=1))


def freeze_style(which: str = "github style", name: str = "frozen-style.json") -> None:
    """The set in another house style, hashed before it is read, and the four
    models exactly as frozen-rest.json holds them, their files checked
    unchanged since; written to frozen-style.json."""
    import hashlib
    from datetime import datetime, timezone
    path = OUT / name
    if path.exists():
        raise SystemExit("already frozen")
    rest = json.load(open(OUT / "frozen-rest.json", encoding="utf-8"))
    for n, rec in rest["models"].items():
        for part, pf in rec["parts"].items():
            d = MODELS[part][0]
            for k in ("keys-first.ubj", "keys-second.txt", "graph-final.json"):
                assert hashlib.sha256((d / k).read_bytes()).hexdigest() == pf[k], (part, k)
    frozen = {"frozen at": datetime.now(timezone.utc).isoformat(timespec="seconds"), "models": rest["models"],
              "models as frozen at": rest["frozen at"], "chosen by the rule": rest["chosen by the rule"],
              which: hashlib.sha256((HELDOUT[which] / "hidden.jsonl").read_bytes()).hexdigest()}
    path.write_text(json.dumps(frozen, indent=1) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in frozen.items() if k != "models"}, indent=1))


def freeze_fresh(stage: str) -> None:
    """`models`: 1s, seven-restyled, seven-bounded and nine-mended frozen by
    hash into frozen-fresh.json before the fresh set is fetched; `set`: the
    set's hash added once it is written, before it is read."""
    import hashlib
    from datetime import datetime, timezone
    path = OUT / "frozen-fresh.json"
    now = datetime.now(timezone.utc).isoformat(timespec="seconds")
    if stage == "models":
        if path.exists():
            raise SystemExit("already frozen")
        frozen = {"frozen at": now, "chosen as the paper's model": "nine-mended", "models": {}}
        for n in ("nine-mended", "1s", "seven-restyled", "seven-bounded"):
            d, variant = MODELS[n]
            frozen["models"][n] = {"parts": {n: {"variant": variant, **{
                f: hashlib.sha256((d / f).read_bytes()).hexdigest()
                for f in ("keys-first.ubj", "keys-second.txt", "graph-final.json")}}}}
    else:
        frozen = json.load(open(path, encoding="utf-8"))
        if "github fresh" in frozen:
            raise SystemExit("the set is frozen already")
        frozen["github fresh"] = hashlib.sha256((HELDOUT["github fresh"] / "hidden.jsonl").read_bytes()).hexdigest()
        frozen["github fresh, frozen at"] = now
    path.write_text(json.dumps(frozen, indent=1) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in frozen.items() if k != "models"}, indent=1))


def heldout(which: str, frozen_file: str = "frozen.json") -> None:
    """The frozen models, once, on one held-out set."""
    import hashlib
    frozen = json.load(open(OUT / frozen_file, encoding="utf-8"))
    done = OUT / f"heldout-{which.replace(' ', '-')}.json"
    if done.exists():
        raise SystemExit("this held-out set has been read once already")
    data = {}
    for s, d in HELDOUT.items():
        if s != which:
            continue
        assert hashlib.sha256((d / "hidden.jsonl").read_bytes()).hexdigest() == frozen[s], s
        schemas = [json.loads(line) for line in open(d / "hidden.jsonl", encoding="utf-8")]
        data[s] = schemas
    result = {"frozen": frozen, "sets": {}}
    with letters.pool(graph.WORKERS) as pool:
        for n, f in frozen["models"].items():
            # a model, or a blend of models of one variant (frozen-rest.json)
            parts = f["parts"] if "parts" in f else {n: f}
            loaded = []
            for part, pf in parts.items():
                d, variant = MODELS[part]
                for k in ("keys-first.ubj", "keys-second.txt", "graph-final.json"):
                    assert hashlib.sha256((d / k).read_bytes()).hexdigest() == pf[k], (part, k)
                final = json.load(open(d / "graph-final.json", encoding="utf-8"))
                loaded.append((*widen.load_models(d), final))
            threshold = f["threshold"] if len(parts) > 1 else loaded[0][2]["threshold"]
            for s, schemas in data.items():
                x, meta = cues.build_all(pool, schemas, "external", {}, variant)
                raw = sum(second.predict(stack.of(final)(meta, first.predict_proba(x)[:, 1]))
                          for first, second, final in loaded) / len(loaded)
                kept = keys.one_target(raw, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
                ids, per = per_database(schemas, "external", meta, kept, threshold)
                _, half = per_database(schemas, "external", meta, kept, 0.5)
                result["sets"].setdefault(s, {})[n] = {"databases": ids, "per database": per.tolist(),
                                                       "per database at 0.5": half.tolist(),
                                                       "candidates hold": int(sum(m[graph.LABEL] for m in meta))}
    for s, by in result["sets"].items():
        names = list(by)
        idx = None
        boots = {}
        for n in names:
            per = np.array(by[n]["per database"])
            if idx is None:
                idx = np.random.default_rng(EXTERNAL_SEED).integers(0, len(per), (1000, len(per)))
            boots[n] = resampled(per, idx)
            p, r, f = prf(*per.sum(axis=0))
            by[n].update(precision=p, recall=r, f1=f, f1_95=interval(boots[n][:, 2]),
                         keys=int(per[:, 0].sum() + per[:, 2].sum()),
                         **{"macro f1": float(np.mean([prf(*c)[2] for c in per])),
                            "f1 at 0.5": prf(*np.array(by[n]["per database at 0.5"]).sum(axis=0))[2]})
            print(f"{s:12} {n:22} P {p:.3f} R {r:.3f} F1 {f:.3f} {by[n]['f1_95']}")
        for n in names:
            if n != "1s":
                by[n]["minus 1s, f1_95"] = interval(boots[n][:, 2] - boots["1s"][:, 2])
                print(f"{s:12} {n:22} minus 1s {by[n]['f1'] - by['1s']['f1']:+.3f} {by[n]['minus 1s, f1_95']}")
    done.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "fit":
        fit(sys.argv[2:])
    elif cmd == "score":
        score(sys.argv[2:])
    elif cmd == "freeze":
        freeze(sys.argv[2:])
    elif cmd == "freeze-set":
        freeze_set(sys.argv[2])
    elif cmd == "freeze-final":
        freeze_final(sys.argv[2:])
    elif cmd == "freeze-rest":
        freeze_rest(sys.argv[2:])
    elif cmd == "freeze-fresh":
        freeze_fresh(sys.argv[2])
    elif cmd == "freeze-style":
        if sys.argv[2:] == ["fixed"]:
            freeze_style("github style, fixed", "frozen-style-fixed.json")
        elif sys.argv[2:] == ["typed"]:
            freeze_style("github style, typed", "frozen-style-typed.json")
        else:
            freeze_style()
    elif cmd == "heldout":
        heldout(sys.argv[2], *sys.argv[3:4])
    else:
        summarize()
