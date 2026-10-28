"""Round 1c's ranker, learned from teachers: every schema's questions, their
features, and how each teacher's ten questions label them; a gradient-
boosted ranker fitted over folds of schemas, measured against the teachers
it did not see, and against round 1c's hand prior.

    python -I learn.py build <teacher dir> <out>
    python -I learn.py cv <out>
    python -I learn.py holdout <out> <first key held out>
    python -I learn.py fit <out>
    python -I learn.py fixtures <out> <corpus/see> <gold.json>... [-v]

build writes <out>/data.pkl; cv and holdout print and write <out>/cv.json
and <out>/holdout.json; fit writes <out>/ranker.txt, which pack.py packs into
the bytes a binary would carry, and <out>/words.json, the per-word rates
that were tried and are off; fixtures writes <out>/fixtures.json.
"""

from __future__ import annotations

import json
import math
import pickle
import random
import sys
from collections import Counter, defaultdict
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent))
import gold  # noqa: E402
import rank  # noqa: E402
from aq import generate, identity, load  # noqa: E402

TEACHERS = ("haiku", "gemini")


def teacher_answers(tdir: Path) -> dict[str, dict[str, list]]:
    """{teacher: {schema key: [questions]}}"""
    out: dict[str, dict[str, list]] = {}
    for teacher in TEACHERS:
        out[teacher] = {}
        for p in sorted((tdir / f"answers-{teacher}").glob("*.json")):
            try:
                answer = json.loads(p.read_text(encoding="utf-8"))
            except json.JSONDecodeError:
                continue
            for key, qs in answer.items():
                if isinstance(qs, list) and qs:
                    out[teacher][key] = qs
    return out


def labels_of(schema, qs_ids, teacher_qs):
    """Per candidate: 3 when a teacher put it in its first five, 2 in its
    ten, 1 when it shares the base and the measure, or the base and both
    dimensions, with one of them; and the teacher's parsed identities."""
    asked = []
    for r, tq in enumerate(teacher_qs):
        gid, _ = gold.identity(schema, tq)
        asked.append((r, gid))
    exact = {}
    partial_m, partial_d = set(), set()
    for r, gid in asked:
        if gid is None:
            continue
        g = 3 if r < 5 else 2
        exact[gid] = max(exact.get(gid, 0), g)
        partial_m.add((gid[0], gid[1]))
        partial_d.add((gid[0], gid[2], gid[3]))
    lab = []
    for ident in qs_ids:
        if ident in exact:
            lab.append(exact[ident])
        elif (ident[0], ident[1]) in partial_m or (ident[0], ident[2], ident[3]) in partial_d:
            lab.append(1)
        else:
            lab.append(0)
    return lab, asked


def build(tdir: Path, out: Path) -> None:
    answers = teacher_answers(tdir)
    rows = []
    stats = Counter()
    for line in (tdir / "schemas.jsonl").open(encoding="utf-8"):
        snap = json.loads(line)
        key = snap["key"]
        have = {t: answers[t].get(key) for t in TEACHERS if answers[t].get(key)}
        if not have:
            continue
        schema = load(snap, reads_n=False)
        qs = generate(schema)
        qs = [q for q in qs if rank.veto(schema, q) is None]
        if not qs:
            stats["schemas with no question"] += 1
            continue
        ids = [identity(schema, q) for q in qs]
        feats = [rank.features(schema, q) for q in qs]
        per_teacher = {}
        for t, tq in have.items():
            lab, asked = labels_of(schema, ids, tq)
            parsed = [gid for _, gid in asked]
            stats[f"{t} questions"] += len(parsed)
            stats[f"{t} questions read"] += sum(1 for g in parsed if g is not None)
            stats[f"{t} questions expressible"] += sum(1 for g in parsed if g is not None and g in set(ids))
            per_teacher[t] = {"labels": lab, "asked": parsed}
        rows.append({"key": key, "repo": snap.get("repo", key), "ids": ids, "features": feats, "teachers": per_teacher,
                     "slots": [rank.slots(schema, q) for q in qs],
                     "titles": [q.title(schema) for q in qs], "n": len(qs)})
        stats["schemas"] += 1
        stats["questions"] += len(qs)
    out.mkdir(parents=True, exist_ok=True)
    with (out / "data.pkl").open("wb") as f:
        pickle.dump(rows, f)
    print(json.dumps(stats, indent=1))


SLOTS = ("base", "measure", "time", "by")
SMOOTH = 20.0


def label_of(row) -> np.ndarray:
    return np.max([np.array(t["labels"]) for t in row["teachers"].values()], axis=0)


def word_counts(rows):
    """Per slot and word: how many questions hold it, and how many of them a
    teacher asked."""
    counts = {s: defaultdict(lambda: [0.0, 0.0]) for s in SLOTS}
    for r in rows:
        lab = label_of(r)
        for sl, y in zip(r["slots"], lab):
            for s in SLOTS:
                for w in set(sl[s]):
                    c = counts[s][w]
                    c[0] += 1.0 if y >= 2 else 0.0
                    c[1] += 1.0
    return counts


def word_table(rows) -> dict:
    counts = word_counts(rows)
    total_pos = sum(c[0] for c in counts["base"].values())
    total = sum(c[1] for c in counts["base"].values())
    prior = total_pos / max(total, 1.0)
    return {"prior": prior, "slots": {s: {w: c for w, c in counts[s].items() if c[1] >= 3} for s in SLOTS}}


def word_features(row, table, own=None) -> list[dict]:
    """Each question's words as the rates a teacher asked them, smoothed
    toward the base rate. With `own`, the schema's own counts are taken out
    of the table, so a training row does not see its own labels."""
    prior = table["prior"]
    out = []
    for sl in row["slots"]:
        f = {}
        for s in SLOTS:
            rates = []
            for w in set(sl[s]):
                c = table["slots"][s].get(w)
                if c is None:
                    continue
                pos, n = c
                if own is not None:
                    o = own[s].get(w)
                    if o:
                        pos, n = pos - o[0], n - o[1]
                if n <= 0:
                    continue
                rates.append((pos + SMOOTH * prior) / (n + SMOOTH))
            f[f"w_{s}_max"] = max(rates) if rates else float("nan")
            f[f"w_{s}_min"] = min(rates) if rates else float("nan")
        out.append(f)
    return out


def matrix(rows, names, table=None, leave_out=False):
    X, y, groups = [], [], []
    for r in rows:
        wf = word_features(r, table, word_counts([r]) if leave_out else None) if table else None
        for i, f in enumerate(r["features"]):
            if wf:
                f = {**f, **wf[i]}
            X.append([f.get(n, float("nan")) for n in names])
        lab = np.max([np.array(t["labels"]) for t in r["teachers"].values()], axis=0)
        y.extend(lab.tolist())
        groups.append(r["n"])
    return np.array(X, dtype=np.float32), np.array(y), groups


def five(row, scores, schema_free=True):
    """The five chosen by rank.choose's rules, over identities alone."""
    order = np.argsort(-np.asarray(scores), kind="stable")
    chosen, per_table, per_kind, times, bys, asked = [], Counter(), Counter(), set(), set(), set()
    for i in order:
        base, measure, time, by = row["ids"][i]
        ident = row["ids"][i]
        kind = rank.kind_of(ident)
        if ident in asked or per_table[base] >= 2 or (time and time in times) or (by and by in bys) or per_kind[kind] >= rank.MAX_KIND:
            continue
        chosen.append(int(i))
        asked.add(ident)
        per_table[base] += 1
        per_kind[kind] += 1
        if time:
            times.add(time)
        if by:
            bys.add(by)
        if len(chosen) == 5:
            break
    return chosen


def measure(rows, scorer, label=""):
    """Against each teacher: precision — the share of our five the teacher
    asked anywhere in its ten — and recall — the share of the teacher's first
    five in our five — averaged over schemas; and NDCG@5 of the order."""
    res = defaultdict(list)
    for r in rows:
        s = scorer(r)
        chosen = five(r, s)
        ours = {r["ids"][i] for i in chosen}
        for t, d in r["teachers"].items():
            ten = {g for g in d["asked"] if g is not None}
            first = [g for g in d["asked"][:5] if g is not None]
            res[f"{t} precision"].append(len(ours & ten) / 5)
            res[f"{t} recall"].append(sum(1 for g in d["asked"][:5] if g in ours) / 5)
            res[f"{t} recall of what can be said"].append(
                (sum(1 for g in first if g in ours) / len([g for g in first if g in set(r["ids"])]))
                if any(g in set(r["ids"]) for g in first) else float("nan"))
            lab = np.array(d["labels"])
            order = np.argsort(-np.asarray(s), kind="stable")[:5]
            dcg = sum((2 ** lab[i] - 1) / math.log2(k + 2) for k, i in enumerate(order))
            ideal = sorted(lab, reverse=True)[:5]
            idcg = sum((2 ** v - 1) / math.log2(k + 2) for k, v in enumerate(ideal))
            res[f"{t} ndcg@5"].append(dcg / idcg if idcg else float("nan"))
    return {k: float(np.nanmean(v)) for k, v in sorted(res.items())}


PARAMS = dict(objective="lambdarank", metric="ndcg", ndcg_eval_at=[5], learning_rate=0.05, num_leaves=7,
              min_data_in_leaf=40, feature_fraction=0.8, bagging_fraction=0.8, bagging_freq=1, lambda_l2=1.0,
              verbose=-1, label_gain=[0, 1, 3, 7], lambdarank_truncation_level=20)


def matrix_free(row, names, table):
    """A schema with no labels, as the ranker reads it."""
    wf = word_features(row, table) if table else None
    X = []
    for i, f in enumerate(row["features"]):
        if wf:
            f = {**f, **wf[i]}
        X.append([f.get(n, float("nan")) for n in names])
    return np.array(X, dtype=np.float32), None, None


def all_names(rows, words=False):
    names = list(rows[0]["features"][0])
    if words:
        names += [f"w_{s}_{k}" for s in SLOTS for k in ("max", "min")]
    return names


def trim(rows, cap=4000):
    """A schema of more questions than LightGBM takes in one query keeps
    every labelled one and a sample of the rest, for training only."""
    out = []
    for r in rows:
        if r["n"] <= cap:
            out.append(r)
            continue
        lab = label_of(r)
        keep = [i for i in range(r["n"]) if lab[i] > 0]
        rest = [i for i in range(r["n"]) if lab[i] == 0]
        random.Random(r["key"]).shuffle(rest)
        keep = sorted(keep + rest[: max(0, cap - len(keep))])
        t = {k: v for k, v in r.items() if k not in ("features", "slots", "ids", "teachers", "titles")}
        t["features"] = [r["features"][i] for i in keep]
        t["slots"] = [r["slots"][i] for i in keep]
        t["ids"] = [r["ids"][i] for i in keep]
        t["teachers"] = {k: {"labels": [d["labels"][i] for i in keep], "asked": d["asked"]} for k, d in r["teachers"].items()}
        t["n"] = len(keep)
        out.append(t)
    return out


def fit_one(train_rows, names, rounds=300, table=None):
    import lightgbm as lgb
    train_rows = trim(train_rows)
    X, y, g = matrix(train_rows, names, table, leave_out=True)
    ds = lgb.Dataset(X, y, group=g, feature_name=names, free_raw_data=False)
    return lgb.train(PARAMS, ds, num_boost_round=rounds)


def cv(out: Path, folds: int = 5, rounds: int = 600) -> None:
    rows = pickle.load((out / "data.pkl").open("rb"))
    names = all_names(rows)
    plain = all_names(rows, words=False)
    repos = sorted({r["repo"] for r in rows})
    random.Random(20261028).shuffle(repos)
    fold_of = {repo: i % folds for i, repo in enumerate(repos)}
    learned, prior = [], []
    for k in range(folds):
        train = [r for r in rows if fold_of[r["repo"]] != k]
        test = [r for r in rows if fold_of[r["repo"]] == k]
        table = word_table(train)
        model = fit_one(train, names, rounds, table)
        bare = fit_one(train, plain, rounds)
        for r in test:
            X, _, _ = matrix_free(r, names, table)
            r["_learned"] = model.predict(X)
            Xb, _, _ = matrix_free(r, plain, None)
            r["_bare"] = bare.predict(Xb)
            r["_prior"] = [rank.prior(f) for f in r["features"]]
    result = {
        "schemas": len(rows),
        "prior": measure(rows, lambda r: r["_prior"]),
        "learned without words": measure(rows, lambda r: r["_bare"]),
        "learned": measure(rows, lambda r: r["_learned"]),
    }
    # how far two teachers agree: Gemini's first five as if they were ours
    both = [r for r in rows if len(r["teachers"]) == 2]
    agree = defaultdict(list)
    for r in both:
        h, g = r["teachers"]["haiku"]["asked"], r["teachers"]["gemini"]["asked"]
        hs, gs = {x for x in h if x}, {x for x in g if x}
        agree["gemini's five in haiku's ten"].append(len({x for x in g[:5] if x} & hs) / 5)
        agree["haiku's five in gemini's ten"].append(len({x for x in h[:5] if x} & gs) / 5)
    result["teachers agree"] = {k: float(np.mean(v)) for k, v in agree.items()}
    result["schemas with both teachers"] = len(both)
    (out / "cv.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(json.dumps(result, indent=1))


def holdout(out: Path, first: str, rounds: int = 600) -> None:
    """Fitted on the schemas before `first`, measured once on the rest: the
    second wave of teacher schemas, chosen after the first and never looked
    at."""
    rows = pickle.load((out / "data.pkl").open("rb"))
    names = all_names(rows)
    train = [r for r in rows if r["key"] < first]
    test = [r for r in rows if r["key"] >= first]
    model = fit_one(train, names, rounds)
    for r in test:
        X, _, _ = matrix_free(r, names, None)
        r["_learned"] = model.predict(X)
        r["_prior"] = [rank.prior(f) for f in r["features"]]
    result = {"train schemas": len(train), "test schemas": len(test),
              "prior": measure(test, lambda r: r["_prior"]), "learned": measure(test, lambda r: r["_learned"])}
    (out / "holdout.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(json.dumps(result, indent=1))


def fit(out: Path, rounds: int = 600) -> None:
    rows = pickle.load((out / "data.pkl").open("rb"))
    names = all_names(rows)
    table = word_table(rows)
    model = fit_one(rows, names, rounds, table)
    path = out / "ranker.txt"
    model.save_model(str(path))
    words = {"prior": table["prior"], "slots": {s: {w: [round(c[0]), round(c[1])] for w, c in t.items()} for s, t in table["slots"].items()}}
    (out / "words.json").write_text(json.dumps(words, separators=(",", ":")), encoding="utf-8")
    print(f"words: {sum(len(t) for t in words['slots'].values())} in {(out / 'words.json').stat().st_size} bytes")
    imp = sorted(zip(names, model.feature_importance("gain")), key=lambda x: -x[1])
    print(f"{path.stat().st_size} bytes as text; {model.num_trees()} trees")
    for n, v in imp[:20]:
        print(f"  {n}: {v:.0f}")


def fixtures(out: Path, see: Path, golds: list[Path], show: bool = False) -> dict:
    """Each fixture's gold questions against the five the ranker chooses,
    from the schema alone (A) and with the statistics (B1)."""
    import lightgbm as lgb
    model = lgb.Booster(model_file=str(out / "ranker.txt"))
    names = model.feature_name()
    table = json.loads((out / "words.json").read_text(encoding="utf-8"))
    totals = Counter()
    report = {}
    for gpath in golds:
        g = json.loads(gpath.read_text(encoding="utf-8"))
        for fx, spec in g["fixtures"].items():
            snap = json.loads((see / f"{fx}-{spec['engine']}" / "snapshot.json").read_text(encoding="utf-8"))
            for cfg, reads_n in (("A", False), ("B1", True)):
                schema = load(snap, reads_n)
                qs = [q for q in generate(schema) if rank.veto(schema, q) is None]
                row = {"features": [rank.features(schema, q) for q in qs], "slots": [rank.slots(schema, q) for q in qs]}
                X, _, _ = matrix_free(row, names, table)
                scores = model.predict(X)
                if reads_n:
                    scores = scores + np.array([rank.stats_points(schema, q) for q in qs])
                chosen = rank.choose(schema, qs, list(scores))
                ids = [identity(schema, q) for q in qs]
                five_ids = [ids[i] for i in chosen]
                order = np.argsort(-scores, kind="stable")
                pos = {}
                for r, i in enumerate(order):
                    pos.setdefault(ids[i], r + 1)
                hits, ranks = 0, []
                for q in spec["questions"]:
                    gid, _ = gold.identity(schema, q)
                    hits += gid in five_ids
                    ranks.append(pos.get(gid))
                totals[(gpath.stem, cfg)] += hits
                report[f"{fx} {cfg}"] = {"in five": hits, "ranks": ranks, "five": [qs[i].title(schema) for i in chosen],
                                         "sql": [qs[i].sql(schema) for i in chosen]}
                if show:
                    print(f"{fx} {cfg}: gold in five {hits}; ranks {ranks}")
                    for i in chosen:
                        print("    ", qs[i].title(schema))
    print({f"{k[0]} {k[1]}": v for k, v in totals.items()})
    (out / "fixtures.json").write_text(json.dumps(report, indent=1, ensure_ascii=False), encoding="utf-8")
    return report


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "build":
        build(Path(sys.argv[2]), Path(sys.argv[3]))
    elif cmd == "cv":
        cv(Path(sys.argv[2]))
    elif cmd == "fit":
        fit(Path(sys.argv[2]))
    elif cmd == "holdout":
        holdout(Path(sys.argv[2]), sys.argv[3])
    elif cmd == "fixtures":
        fixtures(Path(sys.argv[2]), Path(sys.argv[3]), [Path(p) for p in sys.argv[4:] if not p.startswith("-")], "-v" in sys.argv)
