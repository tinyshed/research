"""Round 1v: the first stage fitted with version 10's cues, its scores kept.

    python -I first.py fit <name>             five folds and the whole, on the graphics card
    python -I first.py score <name> [<set>]   the whole model's scores on the sets dump.py wrote

Round 1t's procedure (widen.fit through fit.py's fit_pairs), seed 20261027,
the trigrams sorted, with one difference in what is kept: every training
pair's score out of fold, the sample the stages are fitted on and the
validation split's scores are written to corpus/round-1v/first/<name>/, so
that a second stage can be fitted again and again without the first.

FITS name what a fit is:

    ten            version 10's pairs; half of SchemaPile's training schemas
                   renamed with the dictionary's shortenings and a key word
                   first, every validation schema renamed beside itself, as
                   `nine-mended`; no hard negatives
    ten-reliable   the same, and a pair the name rules call a key that no
                   declaration names left out of the training sample (round
                   1t specified it and ran out of memory before fitting it)
    ten-sure       the same as `ten`, a second pass: the training sample
                   without the columns `ten` scores as keys out of fold that
                   no declaration names, in the schemas that declare under
                   four fifths of such columns (whole.unlabeled)
    ten-taught     the same as `ten`, with what two teachers add to the
                   declarations (teach.py `taught`): a column both point at
                   the same table is a key of its schema, renamed with the
                   declared ones; a column only one names, or a taught key
                   no candidate reaches, is left out of the sample
    ten-taught-more  the same, taught as well from the teachers' answers on
                   the training schemas of 10 to 19 tables that were answered
                   before the proxy's limit (taught-more.json)

    ten-unsure     the teachers' answers used to doubt, not to teach: every
                   column a teacher names that no declaration names is left
                   out of the sample, none is taught as a key

    eleven         `ten` with version 11's cues for the people of a schema
                   (ten.py `add_people`)
    eleven-taught  the same, taught as `ten-taught-more`, except that a
                   column SchemaPile declares as one of a key of several
                   columns is not taught: every set here counts one-column
                   keys only, and such a column drawn is counted false

    ten-seed2, eleven-seed2   `ten` and `eleven` with seed 20261028 for the
                   sample of negatives and both stages' trees; the renamed
                   training schemas are the same

A fit with teachers keeps, a pair, whether its label is a teacher's
(`taught`) beside the label it was fitted on; the validation split's pairs
keep both too, so that a threshold can be read on either.
"""

import json
import pickle
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "round-1t"))

import fit as round_1t  # noqa: E402  (the earlier rounds on the path, the trigrams sorted)

import numpy as np  # noqa: E402

import graph  # noqa: E402
import keys  # noqa: E402
import letters  # noqa: E402
import restyle  # noqa: E402
import ten  # noqa: E402
from train import NEGATIVES  # noqa: E402

OUT = round_1t.ROOT / "round-1v"
SEED = round_1t.SEED
FITS = {
    "ten": {},
    "ten-reliable": {"reliable negatives": True},
    "ten-sure": {"unlabeled from": "ten"},
    "ten-taught": {"taught": "taught.json"},
    "ten-taught-more": {"taught": "taught-more.json"},
    "ten-unsure": {"taught": "taught-more.json", "teach": "leave out"},
    "eleven": {"people": True},
    # the same recipes with another seed, to say how far a fit moves by its seed alone
    "ten-seed2": {"seed": 20261028},
    "eleven-seed2": {"people": True, "seed": 20261028},
    "eleven-taught": {"people": True, "taught": "taught-more.json", "not taught": "declared-otherwise.json"},
}
SIDE_BY_SIDE = 3  # fits on the graphics card at once
graph.WORKERS = 8  # the machine has sixteen threads and memory for them; round 1t kept eight beside other work
DENSE = (graph.BASE, graph.SAME_TYPE, graph.NAMED_KEY, graph.ENDS_ID, graph.BARE,
         ten.HOME, ten.CARRIED, ten.VIEW, ten.CORE, ten.INSIDE)


def slug(name: str) -> str:
    return name.replace(",", "").replace("'", "").replace("(", "").replace(")", "").replace(" ", "-")


def compact(meta: list, schemas: list) -> dict:
    """A pair's meta as indices into the list of schemas, and its dense fields."""
    where = {s["id"]: i for i, s in enumerate(schemas)}
    tables = [{t["name"]: j for j, t in enumerate(s["tables"])} for s in schemas]
    columns = [[{c["name"]: k for k, c in enumerate(t["columns"])} for t in s["tables"]] for s in schemas]
    n = len(meta)
    rec = {k: np.zeros(n, np.int32) for k in ("schema", "table", "column", "target", "key")}
    rec["label"], rec["rules"] = np.zeros(n, bool), np.zeros(n, bool)
    rec["dense"] = np.zeros((n, len(DENSE)), np.float32)
    for i, m in enumerate(meta):
        si = where[m[graph.SID]]
        ti, gi = tables[si][m[graph.TABLE]], tables[si][m[graph.TARGET]]
        rec["schema"][i], rec["table"][i], rec["target"][i] = si, ti, gi
        rec["column"][i] = columns[si][ti][m[graph.COLUMN]]
        rec["key"][i] = columns[si][gi][m[14]]
        rec["label"][i], rec["rules"][i] = m[graph.LABEL], m[graph.RULES]
        rec["dense"][i] = [m[j] for j in DENSE]
    return rec


def expand(rec: dict, schemas: list) -> list:
    """The meta as ten.build wrote it."""
    out = []
    d = rec["dense"]
    for i in range(len(rec["schema"])):
        s = schemas[rec["schema"][i]]
        t, g = s["tables"][rec["table"][i]], s["tables"][rec["target"][i]]
        out.append((s["id"], s["repo"], bool(s.get("novel")), t["name"], t["columns"][rec["column"][i]]["name"], g["name"],
                    rec["table"][i] == rec["target"][i], bool(rec["label"][i]), bool(rec["rules"][i]),
                    float(d[i, 0]), float(d[i, 1]), float(d[i, 2]), float(d[i, 3]), float(d[i, 4]),
                    g["columns"][rec["key"][i]]["name"],
                    float(d[i, 5]), float(d[i, 6]), int(d[i, 7]), float(d[i, 8]), float(d[i, 9])))
    return out


def training_data(opts: dict | None = None):
    opts = opts or {}
    restyle.DICTIONARY, restyle.KEY_FIRST = True, True
    try:
        if not opts.get("taught"):
            return round_1t.training_data(True, False, {})
        return taught_data(opts["taught"], leave_out=opts.get("teach") == "leave out",
                           otherwise=opts.get("not taught"))
    finally:
        restyle.DICTIONARY, restyle.KEY_FIRST = False, False


def taught_data(name: str, undeclared: bool = False, leave_out: bool = False, otherwise: str | None = None):
    """round 1t's training_data with restyling, the teachers' keys added to
    a schema's declarations before it is renamed; a schema keeps, by index,
    which of its keys are taught and which of its columns are unsure."""
    import random
    from collections import defaultdict
    taught = json.load(open(OUT / "lm" / name, encoding="utf-8"))
    if otherwise:
        # a column of a key of several columns stays as the declarations have it: no key of one column
        several = json.load(open(OUT / "lm" / otherwise, encoding="utf-8"))
        for sid, t in taught.items():
            skip = {tuple(k) for k in several.get(sid, {}).get("several", ())}
            t["both"] = [k for k in t["both"] if (k[0], k[1]) not in skip]
            t["one"] = [k for k in t["one"] if tuple(k) not in skip]
    schemas, guesses = keys.load(round_1t.ROOT / "round-1b")
    counts = defaultdict(int)
    out = []
    for s in schemas:
        t = taught.get(s["id"])
        if t and s["split"] in ("train", "validation") and (s["fk"] or undeclared):
            names = [[x["name"] for x in tb["columns"]] for tb in s["tables"]]
            tn = [tb["name"] for tb in s["tables"]]
            add = [] if leave_out else [[tn[a], names[a][c], tn[g]] for a, c, g in t["both"]]
            s = {**s, "fk": s["fk"] + add, "taught": [] if leave_out else [list(k) for k in t["both"]],
                 "unsure": [list(k) for k in t["one"]] + [k[:2] for k in t["both"]]}
            counts["schemas with teachers' answers"] += 1
            counts["taught keys"] += len(add)
            counts["schemas taught from nothing declared"] += (len(add) == len(s["fk"])) and bool(add)
        if s["split"] == "train" and s["fk"]:
            if random.Random(f"{round_1t.RESTYLE_SEED}:share:{s['id']}").random() < round_1t.RESTYLED_SHARE:
                r = round_1t.restyled(s, round_1t.RESTYLE_SEED)
                counts["train renamed" if r else "train left, the style did not fit"] += 1
                out.append(r or s)
                continue
        out.append(s)
        if s["split"] == "validation" and s["fk"]:
            r = round_1t.restyled(s, round_1t.RESTYLE_SEED, "~restyled")
            if r:
                out.append(r)
                counts["validation renamed beside"] += 1
    return out, guesses, dict(counts)


def teachers_marks(rec: dict, schemas: list) -> tuple:
    """A pair's label a teacher's, and its column unsure with no key among its pairs."""
    taught = set()
    unsure = set()
    for si, s in enumerate(schemas):
        for a, c, g in s.get("taught", ()):
            taught.add((si, a, c, g))
        for a, c in s.get("unsure", ()):
            unsure.add((si, a, c))
    n = len(rec["schema"])
    is_taught = np.zeros(n, dtype=bool)
    is_unsure = np.zeros(n, dtype=bool)
    if taught or unsure:
        sc, tb, cl, tg = rec["schema"], rec["table"], rec["column"], rec["target"]
        for i in range(n):
            k = (int(sc[i]), int(tb[i]), int(cl[i]))
            if k in unsure:
                is_unsure[i] = True
                if rec["label"][i] and (*k, int(tg[i])) in taught:
                    is_taught[i] = True
    import whole  # noqa: E402
    ck = whole.column_keys(rec)
    uniq, inv = np.unique(ck, return_inverse=True)
    keyed = np.zeros(len(uniq), dtype=bool)
    np.logical_or.at(keyed, inv, rec["label"])
    return is_taught, is_unsure & ~keyed[inv]


def fit(name: str, small: bool = False) -> None:
    assert letters.is_sorted()
    opts = FITS[name]
    d = OUT / "first" / (name + ("-small" if small else ""))
    d.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    clock = lambda what: print(f"{name}: {what}: {time.time() - t0:.0f} s", flush=True)  # noqa: E731
    schemas, guesses, counts = training_data(opts)
    train = [s for s in schemas if s["split"] == "train" and s["fk"]]
    val = [s for s in schemas if s["split"] == "validation" and s["fk"]]
    if small:  # a twentieth of the schemas, to try the code
        train, val = train[::20], val[::20]
    clock(f"{len(train)} training and {len(val)} validation schemas {counts}")
    with letters.pool(graph.WORKERS) as pool:
        x, meta = ten.build_all(pool, train, "train", guesses, bool(opts.get("people")))
        xv, meta_v = ten.build_all(pool, val, "validation", guesses, bool(opts.get("people")))
    clock(f"features, {len(meta)} training pairs, {len(meta_v)} validation pairs")
    y = np.array([m[graph.LABEL] for m in meta], dtype=bool)
    folds = np.array([graph.fold_of(m[graph.REPO]) for m in meta])
    seed = opts.get("seed", SEED)
    graph.FIRST_GPU["random_state"] = seed
    sample = y | (np.random.default_rng(seed).random(len(y)) < NEGATIVES)
    left_out = 0
    if opts.get("reliable negatives"):
        ruled = np.array([m[graph.RULES] for m in meta], dtype=bool)
        left_out = int((sample & ruled & ~y).sum())
        sample &= ~(ruled & ~y)
    taught_rows = unsure_rows = None
    if opts.get("taught"):
        rec0 = compact(meta, train)
        taught_rows, unsure_rows = teachers_marks(rec0, train)
        left_out = int((sample & unsure_rows).sum())
        sample &= ~unsure_rows
        clock(f"teachers: {int(taught_rows.sum())} taught keys among the pairs, {left_out} sampled pairs of unsure columns left out")
        del rec0
    if opts.get("unlabeled from"):
        import whole  # noqa: E402
        base = pickle.load(open(OUT / "first" / opts["unlabeled from"] / "train.pkl", "rb"))["pairs"]
        assert len(base["label"]) == len(y) and (base["label"] == y).all(), "the pairs are not the first pass's"
        u = whole.unlabeled(base, base["first"])
        left_out = int((sample & u).sum())
        sample &= ~u
        del base
    oof = np.zeros(len(y), dtype=np.float32)

    def one(k):
        """A fold's model and its held rows' scores; fold -1 is the whole, scoring the validation split."""
        rows = np.flatnonzero(sample & (folds != k)) if k >= 0 else np.flatnonzero(sample)
        model = graph.first_model(True)
        model.fit(x[rows], y[rows])
        if k < 0:
            return k, model, graph.first_scores(model, xv, True).astype(np.float32)
        held = np.flatnonzero(folds == k)
        return k, None, graph.first_scores(model, x[held], True)

    # the fits side by side: each is its own model, the card and the processors have room for three
    from concurrent.futures import ThreadPoolExecutor
    whole = first_v = None
    with ThreadPoolExecutor(SIDE_BY_SIDE) as fits:
        for k, model, scores in fits.map(one, [*range(graph.FOLDS), -1]):
            if k < 0:
                whole, first_v = model, scores
            else:
                oof[folds == k] = scores
            clock(f"fold {k}" if k >= 0 else "the whole")
    del x, xv
    whole.save_model(str(d / "keys-first.ubj"))
    rec = compact(meta, train)
    rec.update(first=oof, sample=sample, fold=folds.astype(np.int8))
    if taught_rows is not None:
        rec["taught"] = taught_rows
    pickle.dump({"schemas": train, "pairs": rec}, open(d / "train.pkl", "wb"), protocol=4)
    rec_v = compact(meta_v, val)
    rec_v.update(first=first_v)
    if opts.get("taught"):
        rec_v["taught"], rec_v["unsure"] = teachers_marks(rec_v, val)
    pickle.dump({"schemas": val, "pairs": rec_v}, open(d / "validation.pkl", "wb"), protocol=4)
    result = {"fit": name, "options": opts, "seed": seed, "grams": "sorted", "data": counts,
              "training pairs": len(meta), "validation pairs": len(meta_v), "sample": int(sample.sum()),
              "negatives left out": left_out, "seconds": round(time.time() - t0)}
    (d / "first.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result), flush=True)


def pairs_of(pool, n: str, people: bool = False):
    """A set's pairs, built once and kept: the hashed features and the compact rows.
    They are the same for every first stage of one version of the cues."""
    import scipy.sparse as sp  # noqa: E402
    d = OUT / ("pairs-people" if people else "pairs")
    d.mkdir(exist_ok=True)
    data = pickle.load(open(OUT / "sets" / f"{slug(n)}.pkl", "rb"))
    if not (d / f"{slug(n)}.npz").exists():
        schemas, split = data["schemas"], data["split"]
        guesses = {k: {"fk": v} for k, v in data["rules"].items()}
        x, meta = ten.build_all(pool, schemas, split, guesses, people)
        sp.save_npz(d / f"{slug(n)}.npz", x, compressed=False)
        pickle.dump(compact(meta, schemas), open(d / f"{slug(n)}.pkl", "wb"), protocol=4)
    return data, sp.load_npz(d / f"{slug(n)}.npz"), pickle.load(open(d / f"{slug(n)}.pkl", "rb"))


def score(name: str, names: list[str]) -> None:
    import xgboost as xgb  # noqa: E402
    assert letters.is_sorted()
    d = OUT / "first" / name
    names = names or [p.stem for p in sorted((OUT / "sets").glob("*.pkl"))]
    people = name == "pairs-people" or bool(FITS.get(name.replace("-small", ""), {}).get("people"))
    first = None
    if name not in ("pairs", "pairs-people"):  # `score pairs` only builds and keeps the sets' pairs
        first = xgb.XGBClassifier()
        first.load_model(str(d / "keys-first.ubj"))
        first.set_params(device="cpu")
    with letters.pool(graph.WORKERS) as pool:
        for n in names:
            t0 = time.time()
            data, x, rec = pairs_of(pool, n, people)
            if first is not None:
                rec["first"] = first.predict_proba(x)[:, 1].astype(np.float32)
                pickle.dump({"set": data["name"], "pairs": rec}, open(d / f"{slug(n)}.pkl", "wb"), protocol=4)
            print(f"{name:14} {data['name']:34} pairs {x.shape[0]:8d} keys among them {int(rec['label'].sum()):6d}"
                  f"  ({time.time() - t0:.0f} s)", flush=True)
            del x


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "fit":
        fit(sys.argv[2], "--small" in sys.argv)
    else:
        score(sys.argv[2], sys.argv[3:])
