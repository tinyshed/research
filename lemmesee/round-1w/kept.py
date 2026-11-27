"""Round 1w: the paper's numbers that need no pair built again, from the
scores round 1v kept.

    python -I kept.py <out folder>

`eleven--wide` is read from corpus/round-1v/first/eleven/<set>.pkl (the
pairs and the first stage's scores) and second/eleven--wide/sets/<set>.npy
(the second stage's raw scores), at the threshold of its second.json, its
three files checked against round 1v's frozen.json first. `nine-mended` is
read beside it from round 1v's dump of its scores, through the same
functions: its figures must be round 1t's.

Writes main.json (the 398 external databases and LiveSQLBench against round
1's rules and round 1s's four language models, as round 1t's paper.py lms),
fresh.json (the fresh GitHub set against the language models' replies of
9 November, as paper.py fresh-lms), sizes.json (by the number of tables, as
round 1v's sizes.py), bounds.json (round 1t §3's) and github.json (the four
GitHub development sets pooled). No language model is asked.
"""

import hashlib
import json
import pickle
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1v"))
sys.path.insert(0, str(HERE.parent / "round-1s"))
sys.path.insert(0, str(HERE.parent / "round-1p"))

import numpy as np  # noqa: E402

import lm  # noqa: E402  (round 1s's reading of a reply)
import sizes  # noqa: E402  (round 1v's)
import whole  # noqa: E402

OUT = whole.OUT
ROOT = whole.ROOT
MODEL = "eleven--wide"
BASE = "nine-mended"
SEED = 20261101  # round 1p's resampling of databases
EXTERNAL = ("spider", "bird", "ctu")
LSB = "livesqlbench (development)"
GITHUB = ("github 2025", "github 2025, rest", "github style", "github fresh")


def sha(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def check_frozen() -> dict:
    frozen = json.load(open(OUT / "frozen.json", encoding="utf-8"))
    first = MODEL.split("--")[0]
    d = OUT / "second" / MODEL
    now = {"keys-first.ubj": sha(OUT / "first" / first / "keys-first.ubj"),
           "keys-second.txt": sha(d / "keys-second.txt"), "second.json": sha(d / "second.json")}
    assert now == frozen["models"][MODEL], f"{MODEL} changed since it was frozen"
    return {"frozen at": frozen["frozen at"], "files": now}


def scored(model: str, name: str) -> whole.Scored:
    """A set as a model scored it: round 1v's dump for the earlier models, its kept stages for its own."""
    if model == BASE:
        return whole.Scored(model, name)
    first = model.split("--")[0]
    d = OUT / "second" / model
    path = OUT / "first" / first / f"{whole.slug(name)}.pkl"
    raw = d / "sets" / f"{whole.slug(name)}.npy"
    assert raw.stat().st_mtime > max((d / "keys-second.txt").stat().st_mtime, path.stat().st_mtime), \
        "the kept scores are older than the model"
    rec = pickle.load(open(path, "rb"))["pairs"]
    rec["second"] = np.load(raw).astype(np.float64)
    rec["threshold"] = json.load(open(d / "second.json", encoding="utf-8"))["threshold"]
    dump = OUT / "scores" / model
    dump.mkdir(parents=True, exist_ok=True)
    pickle.dump(rec, open(dump / f"{whole.slug(name)}.pkl", "wb"), protocol=4)
    return whole.Scored(model, name)


def keys_of(sc: whole.Scored) -> dict:
    """{schema id: {(table, column, target)}} a model draws."""
    return {sc.schemas[si]["id"]: {(t, c, g) for (t, c), g in p.items()} for si, p in enumerate(sc.baseline())}


def rules_of(path: Path) -> dict:
    rules = {}
    for line in open(path, encoding="utf-8"):
        g = json.loads(line)
        if g.get("fk"):
            rules.setdefault(g["schema"], set()).add((g["table"], g["column"], g["fk"]))
    return rules


def against(schemas: list, ours: dict, others: dict, subs: tuple) -> dict:
    """round 1t's paper.py lms: each model of `ours` over the databases, and minus each of `others`."""
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["fk"]}
    source = {s["id"]: s.get("source", "") for s in schemas}
    out = {}
    for n, preds in ours.items():
        rng = np.random.default_rng(SEED)
        res = {}
        for which in subs:
            sel = sorted(i for i in truth if which in ("all", source[i]))
            idx = rng.integers(0, len(sel), (1000, len(sel)))
            per = lm.counts(truth, preds, sel)
            boot = whole.resampled(per, idx)
            p, r, f = lm.prf(*per.sum(axis=0))
            e = {"databases": len(sel), "keys": int(per[:, 0].sum() + per[:, 2].sum()),
                 "precision": p, "recall": r, "f1": f, "precision_95": whole.interval(boot[:, 0]),
                 "recall_95": whole.interval(boot[:, 1]), "f1_95": whole.interval(boot[:, 2]),
                 "macro f1 over databases": float(np.mean([lm.prf(*c)[2] for c in per]))}
            for m, mp in {**others, **{k: v for k, v in ours.items() if k != n}}.items():
                pa = lm.counts(truth, mp, sel)
                ba = whole.resampled(pa, idx)
                pt = np.array([p, r, f]) - np.array(lm.prf(*pa.sum(axis=0)))
                e[f"minus {m}"] = {w: {"point": float(pt[j]), "95": whole.interval(boot[:, j] - ba[:, j])}
                                   for j, w in enumerate(("precision", "recall", "f1"))}
                e[f"{m}, P / R / F1"] = list(lm.prf(*pa.sum(axis=0)))
                e[f"{m}, macro f1"] = float(np.mean([lm.prf(*c)[2] for c in pa]))
            res[which] = e
        out[n] = res
    return out


def show(title: str, res: dict) -> None:
    print(f"== {title}")
    for n, by in res.items():
        for which, e in by.items():
            print(f"{n:14} {which:8} {e['databases']:4d} dbs {e['keys']:6d} keys  P {e['precision']:.3f} R {e['recall']:.3f} "
                  f"F1 {e['f1']:.3f} [{e['f1_95'][0]:.3f}, {e['f1_95'][1]:.3f}] macro {e['macro f1 over databases']:.3f}")
            for k, v in e.items():
                if k.startswith("minus "):
                    f = v["f1"]
                    print(f"    {k:34} F1 {f['point']:+.3f} [{f['95'][0]:+.3f}, {f['95'][1]:+.3f}]  "
                          f"P {v['precision']['point']:+.3f}  R {v['recall']['point']:+.3f} "
                          f"[{v['recall']['95'][0]:+.3f}, {v['recall']['95'][1]:+.3f}]")
    sys.stdout.flush()


def main_table(sc: dict) -> dict:
    out = {}
    ext = [s for name in EXTERNAL for s in sc[MODEL][name].schemas]
    ours = {m: {k: v for name in EXTERNAL for k, v in keys_of(sc[m][name]).items()} for m in (MODEL, BASE)}
    others = {"round 1's rules": rules_of(ROOT / "round-1s" / "external" / "guesses.jsonl")}
    for m in lm.MODELS:
        others[m], _ = lm.whole_schema(ext, ROOT / "round-1s" / "llm" / m / "db")
    out["external"] = against(ext, ours, others, (*EXTERNAL, "all"))
    show("the 398 external databases", out["external"])
    lsb = sc[MODEL][LSB].schemas
    ours = {m: keys_of(sc[m][LSB]) for m in (MODEL, BASE)}
    others = {"round 1's rules": rules_of(ROOT / "round-1s" / "heldout" / "guesses.jsonl")}
    for m in lm.MODELS:
        others[m], _ = lm.whole_schema(lsb, ROOT / "round-1s" / "llm" / m / "heldout-db")
    out["livesqlbench"] = against(lsb, ours, others, ("all",))
    show("LiveSQLBench", out["livesqlbench"])
    return out


def fresh(sc: dict) -> dict:
    schemas = sc[MODEL]["github fresh"].schemas
    ours = {m: keys_of(sc[m]["github fresh"]) for m in (MODEL, BASE)}
    others = {}
    for m in lm.MODELS:
        d = ROOT / "round-1t" / "lm" / "llm" / m / "fresh-db"
        if d.exists():
            others[m], _ = lm.whole_schema(schemas, d)
    out = against(schemas, ours, others, ("all",))
    show("the fresh GitHub set (development data for eleven--wide)", out)
    return out


def by_size(sc: dict) -> dict:
    out = {}
    pooled = {}
    for name in whole.SETS:
        entry = {}
        for m in (MODEL, BASE):
            s = sc[m][name]
            per = s.counts(s.baseline())
            entry[m] = whole.measure(per, s.tables)
            entry[m]["what becomes of a declared key"] = sizes.fates(s)
            pooled.setdefault(m, []).append((name, per, s.tables))
        if name in sizes.REPLIES:
            root, folder = sizes.REPLIES[name]
            for model in sizes.LMS:
                raws = ROOT / root / "llm" / model / folder
                if raws.exists():
                    per = sizes.lm_counts(sc[MODEL][name].schemas, raws)
                    entry[model] = whole.measure(per, sc[MODEL][name].tables)
                    pooled.setdefault(model, []).append((name, per, sc[MODEL][name].tables))
        out[name] = entry
        print(f"== {name}")
        for n, m in entry.items():
            print(whole.line(n, m))
    names = [n for n in sizes.REPLIES if all(model in out[n] for model in sizes.LMS)]
    print("== " + ", ".join(names) + ", pooled")
    out["pooled where the language models answered"] = {"sets": names}
    for n, parts in pooled.items():
        chosen = [(per, tables) for name, per, tables in parts if name in names]
        if len(chosen) != len(names):
            continue
        per, tables = np.vstack([p for p, _ in chosen]), np.concatenate([t for _, t in chosen])
        base = None
        if n != MODEL:
            mine = [(p, t) for name, p, t in pooled[MODEL] if name in names]
            base = np.vstack([p for p, _ in mine])
        # a model minus eleven--wide on the same resamples, pooled and by size
        m = whole.measure(per, tables, base)
        if base is not None:
            for b in whole.BINS:
                k = (tables >= b[0]) & (tables < b[1])
                if k.any():
                    d = whole.measure(per[k], tables[k], base[k])
                    m["by tables"][whole.bin_name(b)]["minus eleven--wide, f1"] = d["minus base, f1"]
                    m["by tables"][whole.bin_name(b)]["minus eleven--wide, f1_95"] = d["minus base, f1_95"]
        out["pooled where the language models answered"][n] = m
        print(whole.line(n, m))
    sys.stdout.flush()
    return out


def bounds(sc: dict) -> dict:
    """round 1t §3's: the keys among the candidates; F1 at the model's
    threshold, at the best one threshold for the set, and with each schema
    at the threshold best for its own F1 (counts then pooled)."""
    out = {}
    for m in (MODEL, BASE):
        out[m] = {}
        for name in ("schemapile test", *EXTERNAL, LSB):
            s = sc[m][name]
            kept = whole.one_target(np.asarray(s.second, dtype=np.float64),
                                    {"schema": s.schema, "table": s.table, "column": s.column})
            declared = sum(len(t) for t in s.truth)
            candidates = int(s.label.sum())
            rows = {}
            for i in np.flatnonzero(kept > 0):
                rows.setdefault(int(s.schema[i]), []).append((float(kept[i]), bool(s.label[i])))

            def counts(th_of):
                tp = fp = 0
                for si, r in rows.items():
                    th = th_of(si)
                    tp += sum(1 for v, y in r if v >= th and y)
                    fp += sum(1 for v, y in r if v >= th and not y)
                return tp, fp, declared - tp

            grid = np.linspace(0.02, 0.98, 49)
            at = {round(float(t), 2): whole.prf(*counts(lambda si, t=t: t))[2] for t in grid}
            own = {}
            for si, r in rows.items():
                n = len(s.truth[si])
                top, top_t = -1.0, 2.0
                for t in sorted({v for v, _ in r}, reverse=True) + [2.0]:
                    tp = sum(1 for v, y in r if v >= t and y)
                    fp = sum(1 for v, y in r if v >= t and not y)
                    f = 2 * tp / (2 * tp + fp + (n - tp)) if (tp + fp + n) else 0.0
                    if f > top:
                        top, top_t = f, t
                own[si] = top_t
            out[m][name] = {
                "declared keys": declared, "keys among the candidates": candidates,
                "coverage": round(candidates / declared, 4),
                "F1 at the model's threshold": round(whole.prf(*counts(lambda si: s.threshold))[2], 4),
                "F1 at the best threshold for the set": round(max(at.values()), 4),
                "the best threshold for the set": max(at, key=at.get),
                "F1 with each schema at its own best threshold": round(whole.prf(*counts(lambda si: own.get(si, 2.0)))[2], 4)}
            print(f"{m:14} {name:28} {json.dumps(out[m][name])}", flush=True)
    return out


def github(sc: dict) -> dict:
    out = {}
    pers = {m: np.vstack([sc[m][n].counts(sc[m][n].baseline()) for n in GITHUB]) for m in (MODEL, BASE)}
    tables = np.concatenate([sc[MODEL][n].tables for n in GITHUB])
    for m, per in pers.items():
        out[m] = whole.measure(per, tables, pers[BASE] if m != BASE else None)
        print(whole.line(m, out[m]))
    return out


def run(out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    frozen = check_frozen()
    sc = {m: {name: scored(m, name) for name in whole.SETS} for m in (MODEL, BASE)}
    threshold = sc[MODEL]["spider"].threshold
    head = {"model": MODEL, "threshold": threshold, "frozen": frozen, "beside it": BASE}
    for name, fn in (("main", main_table), ("fresh", fresh), ("sizes", by_size), ("bounds", bounds),
                     ("github", github)):
        (out / f"{name}.json").write_text(json.dumps({**head, **fn(sc)}, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    run(Path(sys.argv[1]))
