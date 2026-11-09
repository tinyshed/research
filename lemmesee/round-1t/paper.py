"""Round 1t: the paper's other numbers for its final model, `nine-mended`,
through the code that measured them first; added on 9 November 2026.

    python -I paper.py perm      # round 1s's refit.score: SchemaPile-Perm's test and
                                 # validation, the pair task, related or not, the threshold grid
    python -I paper.py nexus     # round 1p's nexus.py: TPC-H, TPC-DS, BIRD's development set
    python -I paper.py bounds    # coverage, and the perfect thresholds of round 1t's §3
    python -I paper.py owner     # the owner's database A, counts only

Each runs the earlier round's function as it is, with version 9's pairs
(cues.build) in place of round 1p's (widen.build): `nine-mended` reads
graph.py's 19 features in its second stage, as those functions do. Writes
under corpus/round-1t/paper/.
"""

import json
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import cues  # noqa: E402  (first: round 1s, 1p and 1b on the path, the trigrams sorted)
import letters  # noqa: E402

import numpy as np  # noqa: E402

import fit  # noqa: E402
import graph  # noqa: E402
import keys  # noqa: E402
import stack  # noqa: E402
import widen  # noqa: E402

MODEL = "nine-mended"
OUT = fit.OUT / "paper"
CORPUS = HERE.parent / "corpus"


def nine(job):
    return cues.build((*job[:3], "nine"))


def use_nine():
    assert json.load(open(fit.MODELS[MODEL][0] / "graph-final.json", encoding="utf-8")).get("graph") is None
    widen.build_all = cues.build_all
    widen.build = lambda job: cues.build((*job[:3], job[3] if len(job) > 3 else "nine"))


def perm():
    sys.path.insert(0, str(HERE.parent / "round-1s"))
    import refit  # noqa: E402
    use_nine()
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "external").mkdir(exist_ok=True)
    for f in ("hidden.jsonl", "guesses.jsonl", "bench.jsonl", "t5-negatives.jsonl"):
        shutil.copy(CORPUS / "round-1s" / "external" / f, OUT / "external" / f)
    refit.scorings = lambda round_1p, out: {MODEL: (fit.MODELS[MODEL][0], "nine", "sorted", None,
                                                    ("validation", "test", "external"))}
    refit.score(CORPUS / "round-1b", CORPUS / "round-1p", OUT, MODEL)


def nexus():
    import nexus as nx  # noqa: E402  (round 1p's)
    use_nine()
    OUT.mkdir(parents=True, exist_ok=True)
    nx.main(fit.MODELS[MODEL][0], CORPUS / "round-1p", CORPUS, "nine")


def bounds():
    """Over every declared key: the keys among the candidates; F1 at the
    model's threshold, at the best one threshold for the set, and with each
    schema at the threshold best for its own F1 (counts then pooled)."""
    from train import prf  # noqa: E402
    d = fit.MODELS[MODEL][0]
    first, second = widen.load_models(d)
    final = json.load(open(d / "graph-final.json", encoding="utf-8"))
    data = fit.sets()
    out = {}
    with letters.pool(graph.WORKERS) as pool:
        for set_name in ("schemapile test", "spider", "bird", "ctu", "livesqlbench (development)"):
            schemas, split, guesses = data[set_name]
            x, meta = cues.build_all(pool, schemas, split, guesses, final["variant"])
            p2 = second.predict(stack.of(final)(meta, first.predict_proba(x)[:, 1]))
            kept = keys.one_target(p2, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
            truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["split"] == split and s["fk"]}
            declared = sum(len(v) for v in truth.values())
            best = {}
            for i in np.flatnonzero(kept > 0):
                m = meta[i]
                best.setdefault(m[graph.SID], []).append(
                    (float(kept[i]), (m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]) in truth.get(m[graph.SID], ())))
            candidates = sum(1 for m in meta if m[graph.LABEL])

            def counts(th_of):
                tp = fp = 0
                for sid, rows in best.items():
                    th = th_of(sid)
                    tp += sum(1 for sc, y in rows if sc >= th and y)
                    fp += sum(1 for sc, y in rows if sc >= th and not y)
                return tp, fp, declared - tp

            grid = np.linspace(0.02, 0.98, 49)
            at = {round(float(t), 2): prf(*counts(lambda sid, t=t: t))[2] for t in grid}
            own = {}
            for sid, rows in best.items():
                n = len(truth.get(sid, ()))
                scores = sorted({sc for sc, _ in rows}, reverse=True)
                top, top_t = -1.0, 2.0
                for t in scores + [2.0]:
                    tp = sum(1 for sc, y in rows if sc >= t and y)
                    fp = sum(1 for sc, y in rows if sc >= t and not y)
                    f = 2 * tp / (2 * tp + fp + (n - tp)) if (tp + fp + n) else 0.0
                    if f > top:
                        top, top_t = f, t
                own[sid] = top_t
            out[set_name] = {
                "declared keys": declared, "keys among the candidates": candidates,
                "coverage": round(candidates / declared, 4),
                "F1 at the model's threshold": round(prf(*counts(lambda sid: final["threshold"]))[2], 4),
                "F1 at the best threshold for the set": round(max(at.values()), 4),
                "the best threshold for the set": max(at, key=at.get),
                "F1 with each schema at its own best threshold": round(prf(*counts(lambda sid: own.get(sid, 2.0)))[2], 4),
                "recall bound: every candidate key found": round(candidates / declared, 4),
            }
            print(set_name, json.dumps(out[set_name]), flush=True)
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "bounds.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")


def owner():
    """Database A as round 1q's `owner.py draw` read it, counts only: the
    keys `nine-mended` draws, those declared, those no declaration names,
    and of these how many were among the 36 round 1q's reading of the app's
    code judged (its verdicts, key by key, were not kept)."""
    import roles  # noqa: E402,F401  (round 1t's, before round 1q's own roles.py is on the path)
    sys.path.append(str(HERE.parent / "round-1q"))
    import external  # noqa: E402
    import prod  # noqa: E402
    d = fit.MODELS[MODEL][0]
    snap = json.load(open(CORPUS / "prod" / "aimee" / "snapshot.json", encoding="utf-8"))
    s = prod.hidden(snap, "aimee")
    run, g = external.scorer(d, nine)
    thr = json.load(open(d / "graph-final.json", encoding="utf-8"))["threshold"]
    sc, meta = run([s], {})
    kept = keys.one_target(sc, [(m[g.SID], m[g.TABLE], m[g.COLUMN]) for m in meta])
    drawn = {(m[g.TABLE], m[g.COLUMN], m[g.TARGET]) for m, k in zip(meta, kept) if k >= thr}
    declared = {tuple(k) for k in s["fk"]}
    read = {tuple(k) for k in json.load(open(CORPUS / "round-1q" / "owner-aimee-drawn.json", encoding="utf-8"))["undeclared"]}
    undeclared = drawn - declared
    tp = len(drawn & declared)
    result = {"database": "A", "declared one-column keys": len(declared), "drawn": len(drawn),
              "drawn and declared": tp, "drawn, no declaration": len(undeclared),
              "of them among the 36 round 1q's reading judged": len(undeclared & read),
              "of them not read": len(undeclared - read),
              "F1 over the declared keys": round(2 * tp / (len(drawn) + len(declared)), 4) if drawn else 0.0,
              "precision, declared only": round(tp / len(drawn), 4) if drawn else 0.0}
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "owner-A.json").write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


def lms():
    """The paper's main table: `nine-mended` over every declared key of
    round 1p's 398 external databases (its open predictions from `perm`) and
    of LiveSQLBench, with intervals and macro F1, and `nine-mended` minus each
    of round 1s's four language models, paired over databases, from their
    saved replies (round 1s's lm.py reading them; no new call)."""
    sys.path.insert(0, str(HERE.parent / "round-1s"))
    import lm  # noqa: E402
    from refit import EXTERNAL_SEED, interval, resampled  # noqa: E402
    sets = {}
    ext = [json.loads(line) for line in open(OUT / "external" / "hidden.jsonl", encoding="utf-8")]
    ours = {}
    for line in open(OUT / "external" / f"open-predictions-{MODEL}.jsonl", encoding="utf-8"):
        sid, t, c, r, _ = json.loads(line)
        ours.setdefault(sid, set()).add((t, c, r))
    sets["external"] = (ext, ours, "db", ("spider", "bird", "ctu", "all"))
    lsb = [json.loads(line) for line in open(CORPUS / "round-1s" / "heldout" / "hidden.jsonl", encoding="utf-8")]
    d = fit.MODELS[MODEL][0]
    first, second = widen.load_models(d)
    final = json.load(open(d / "graph-final.json", encoding="utf-8"))
    _, lsb_guesses = keys.load(CORPUS / "round-1s" / "heldout")
    with letters.pool(graph.WORKERS) as pool:
        x, meta = cues.build_all(pool, lsb, "external", lsb_guesses, final["variant"])
    p2 = second.predict(stack.of(final)(meta, first.predict_proba(x)[:, 1]))
    kept = keys.one_target(p2, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
    ours_l = {}
    for i in np.flatnonzero(kept >= final["threshold"]):
        m = meta[i]
        ours_l.setdefault(m[graph.SID], set()).add((m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]))
    sets["livesqlbench"] = (lsb, ours_l, "heldout-db", ("all",))
    out = {"model": MODEL}
    for name, (schemas, preds, folder, subs) in sets.items():
        truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["fk"]}
        source = {s["id"]: s.get("source", "livesqlbench") for s in schemas}
        rng = np.random.default_rng(EXTERNAL_SEED)
        res = {}
        lmp = {}
        for mname in lm.MODELS:
            base = CORPUS / "round-1s" / "llm" / mname / folder
            if base.exists():
                lmp[mname], _ = lm.whole_schema(schemas, base)
        # round 1's rules on the same footing: the harness's guess for each
        # column (guesses.jsonl), declarations hidden, as a set of keys
        rules = {}
        gpath = OUT / "external" / "guesses.jsonl" if name == "external" else \
            CORPUS / "round-1s" / "heldout" / "guesses.jsonl"
        for line in open(gpath, encoding="utf-8"):
            g = json.loads(line)
            if g.get("fk"):
                rules.setdefault(g["schema"], set()).add((g["table"], g["column"], g["fk"]))
        lmp["round 1's rules"] = rules
        for which in subs:
            sel = sorted(i for i in truth if which in ("all", source[i]))
            idx = rng.integers(0, len(sel), (1000, len(sel)))
            per = lm.counts(truth, preds, sel)
            boot = resampled(per, idx)
            p, r, f = lm.prf(*per.sum(axis=0))
            e = {"databases": len(sel), "keys": int(per[:, 0].sum() + per[:, 2].sum()),
                 "precision": p, "recall": r, "f1": f, "precision_95": interval(boot[:, 0]),
                 "recall_95": interval(boot[:, 1]), "f1_95": interval(boot[:, 2]),
                 "macro f1 over databases": float(np.mean([lm.prf(*c)[2] for c in per]))}
            for mname, mp in lmp.items():
                pa = lm.counts(truth, mp, sel)
                ba = resampled(pa, idx)
                pt = np.array([p, r, f]) - np.array(lm.prf(*pa.sum(axis=0)))
                e[f"minus {mname}"] = {w: {"point": float(pt[j]), "95": interval(boot[:, j] - ba[:, j])}
                                       for j, w in enumerate(("precision", "recall", "f1"))}
                e[f"{mname}, P / R / F1"] = list(lm.prf(*pa.sum(axis=0)))
            res[which] = e
        out[name] = res
        print(name, json.dumps({k: {kk: (round(vv, 3) if isinstance(vv, float) else vv) for kk, vv in v.items()
                                    if not kk.startswith("minus")} for k, v in res.items()})[:1500], flush=True)
    (OUT / "lms.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")


def fresh_lms():
    """Round 1s's four language models asked on the fresh GitHub set, under
    its protocol (ask.py, round 1p's prompt, one reply a schema, read by
    lm.py): P / R / F1 with intervals, `nine-mended` and `1s` minus each,
    paired over databases, from those two models' one reading of the set;
    tokens, price and latency as lm.py counts them."""
    sys.path.insert(0, str(HERE.parent / "round-1s"))
    import lm  # noqa: E402
    from refit import EXTERNAL_SEED, interval, resampled  # noqa: E402
    schemas = [json.loads(line) for line in open(fit.OUT / "heldout-github-style-fresh" / "hidden.jsonl",
                                                 encoding="utf-8")]
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["fk"]}
    read = json.load(open(fit.OUT / "heldout-github-fresh.json", encoding="utf-8"))["sets"]["github fresh"]
    ids = read["1s"]["databases"]
    assert sorted(truth) == sorted(ids)
    ours = {n: np.array(read[n]["per database"]) for n in ("nine-mended", "1s")}
    idx = np.random.default_rng(EXTERNAL_SEED).integers(0, len(ids), (1000, len(ids)))
    boots = {n: resampled(p, idx) for n, p in ours.items()}
    out = {"databases": len(ids), "keys": int(ours["1s"][:, 0].sum() + ours["1s"][:, 2].sum())}
    for n, p in ours.items():
        out[n] = dict(zip(("precision", "recall", "f1"), lm.prf(*p.sum(axis=0))),
                      f1_95=interval(boots[n][:, 2]), **{"macro f1": float(np.mean([lm.prf(*c)[2] for c in p]))})
    base = fit.OUT / "lm" / "llm"
    for m in lm.MODELS:
        d = base / m / "fresh-db"
        if not d.exists():
            out[m] = "not asked"
            continue
        preds, info = lm.whole_schema(schemas, d)
        per = lm.counts(truth, preds, ids)
        b = resampled(per, idx)
        p, r, f = lm.prf(*per.sum(axis=0))
        e = {"replies": info, "precision": p, "recall": r, "f1": f, "precision_95": interval(b[:, 0]),
             "recall_95": interval(b[:, 1]), "f1_95": interval(b[:, 2]),
             "macro f1": float(np.mean([lm.prf(*c)[2] for c in per])), "usage": lm.usage(m, d)}
        for n in ours:
            pt = np.array([out[n]["precision"], out[n]["recall"], out[n]["f1"]]) - np.array([p, r, f])
            e[f"{n} minus this"] = {w: {"point": float(pt[j]), "95": interval(boots[n][:, j] - b[:, j])}
                                    for j, w in enumerate(("precision", "recall", "f1"))}
        out[m] = e
    (OUT / "fresh-lms.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(json.dumps({k: (v if not isinstance(v, dict) else {kk: vv for kk, vv in v.items() if kk not in ("usage",)})
                      for k, v in out.items()}, indent=1)[:6000])


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    assert letters.is_sorted()
    {"perm": perm, "nexus": nexus, "bounds": bounds, "owner": owner, "lms": lms,
     "fresh-lms": fresh_lms}[sys.argv[1]]()
