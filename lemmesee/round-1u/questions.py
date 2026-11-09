"""Round 1u: see's questions three ways — through the declared keys, round
1's rules, and round 1t's frozen `seven-restyled` — on Spider, BIRD, CTU and
LiveSQLBench. README.md says what is counted.

    python -I questions.py

Writes corpus/round-1u/questions.json: per set and database, the declared
run's questions that join, how many of them each other run asks, how many
of each run's questions go through a key no declaration names, and how many
of the declared run's five each run keeps; and per set the pooled shares
with 95 % intervals over databases.
"""

import hashlib
import json
import re
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT / "round-1t"))

import cues  # noqa: E402  (first: round 1s, 1p and 1b on the path, the trigrams sorted)
import letters  # noqa: E402

import numpy as np  # noqa: E402

import fit  # noqa: E402
import graph  # noqa: E402
import keys  # noqa: E402
import stack  # noqa: E402
import widen  # noqa: E402

sys.path.append(str(ROOT / "round-1c"))
import aq  # noqa: E402
import learn  # noqa: E402
import rank  # noqa: E402

OUT = ROOT / "corpus" / "round-1u"
SEED = 20261101
SETS = ("spider", "bird", "ctu", "livesqlbench (development)")
MODEL = "seven-restyled"
GUESS = aq._guess_keys


def base_type(t: str) -> str:
    return re.sub(r"\(.*", "", (t or "").lower()).replace("unsigned", "").strip()


def snapshot(s: dict, fks) -> dict:
    tables = []
    for t in s["tables"]:
        cons = []
        if t.get("pk"):
            cons.append({"kind": "p", "columns": list(t["pk"])})
        for c in t["columns"]:
            if c.get("unique") and [c["name"]] != list(t.get("pk") or []):
                cons.append({"kind": "u", "columns": [c["name"]]})
        for tt, cc, rr in fks:
            if tt == t["name"]:
                cons.append({"kind": "f", "columns": [cc], "ref_table": rr})
        tables.append({"name": t["name"], "schema": "public", "kind": "table", "constraints": cons, "indexes": [],
                       "columns": [{"name": c["name"], "type_name": c.get("type") or "",
                                    "base_type": base_type(c.get("type") or ""), "nullable": c.get("nullable", True),
                                    "features": {}} for c in t["columns"]]})
    return {"engine": "postgres", "tables": tables}


def steps(schema, q) -> set:
    """The keys a question goes through, as (table, column) of the base side."""
    out = set()
    for d in (q.time, q.by):
        if d is not None:
            out.update(d.path)
    m = q.measure
    if m.kind in ("parents", "per") and m.cols:
        t, c = m.cols[0]
        if schema.tables[t].columns[c].fk is not None:
            out.add((t, c))
    if m.kind in ("product", "net"):
        for p in {t for t, _ in m.cols} - {q.base}:
            cs = [c.c for c in schema.tables[q.base].columns if c.fk == p]
            if cs:
                out.add((q.base, min(cs)))
    return out


def run(snap, guess: bool, model, names, table):
    aq._guess_keys = GUESS if guess else (lambda schema: None)
    try:
        schema = aq.load(snap, reads_n=False)
    finally:
        aq._guess_keys = GUESS
    qs = [q for q in aq.generate(schema) if rank.veto(schema, q) is None]
    ids = [aq.identity(schema, q) + (q.shape,) for q in qs]
    key_of = lambda st: (st[0], st[1], schema.tables[st[0]].columns[st[1]].fk)  # noqa: E731
    through = [{key_of(st) for st in steps(schema, q)} for q in qs]
    keyset = {(tb.t, c.c, c.fk) for tb in schema.tables for c in tb.columns if c.fk is not None}
    five = []
    if qs:
        row = {"features": [rank.features(schema, q) for q in qs], "slots": [rank.slots(schema, q) for q in qs]}
        x, _, _ = learn.matrix_free(row, names, table)
        five = [ids[i] for i in rank.choose(schema, qs, list(model.predict(x)))]
    return ids, through, five, keyset


def drawn_keys(pool, schemas, split, guesses):
    d = fit.MODELS[MODEL][0]
    frozen = json.load(open(fit.OUT / "frozen-final.json", encoding="utf-8"))["models"][MODEL]
    for k in ("keys-first.ubj", "keys-second.txt", "graph-final.json"):
        assert hashlib.sha256((d / k).read_bytes()).hexdigest() == frozen[k], k
    first, second = widen.load_models(d)
    final = json.load(open(d / "graph-final.json", encoding="utf-8"))
    x, meta = cues.build_all(pool, schemas, split, guesses, frozen["variant"])
    p2 = second.predict(stack.of(final)(meta, first.predict_proba(x)[:, 1]))
    kept = keys.one_target(p2, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
    out = {}
    for i in np.flatnonzero(kept >= final["threshold"]):
        m = meta[i]
        out.setdefault(m[graph.SID], []).append((m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET]))
    return out


def pooled(per: np.ndarray, num: int, den: int, idx: np.ndarray) -> dict:
    share = per[:, num].sum() / max(per[:, den].sum(), 1)
    b = per[idx]
    boots = b[:, :, num].sum(axis=1) / np.maximum(b[:, :, den].sum(axis=1), 1)
    return {"share": round(float(share), 4), "95": [round(float(np.percentile(boots, 2.5)), 4),
                                                     round(float(np.percentile(boots, 97.5)), 4)]}


def macro(per: np.ndarray, num: int, den: int, idx: np.ndarray) -> dict:
    """The mean over databases of each one's share, those with a denominator
    only: a database of thousands of questions weighs as one of ten."""
    has = per[:, den] > 0
    each = np.where(has, per[:, num] / np.maximum(per[:, den], 1), np.nan)
    boots = np.nanmean(each[idx], axis=1)
    return {"mean": round(float(np.nanmean(each)), 4), "databases": int(has.sum()),
            "95": [round(float(np.nanpercentile(boots, 2.5)), 4), round(float(np.nanpercentile(boots, 97.5)), 4)]}


def main() -> None:
    import lightgbm as lgb
    assert letters.is_sorted()
    ranker = lgb.Booster(model_file=str(ROOT / "corpus" / "round-1c" / "learn" / "ranker.txt"))
    names = ranker.feature_name()
    table = json.loads((ROOT / "corpus" / "round-1c" / "learn" / "words.json").read_text(encoding="utf-8"))
    ext, ext_guesses = keys.load(fit.ROOT / "round-1s" / "external")
    lsb, lsb_guesses = keys.load(fit.ROOT / "round-1s" / "heldout")
    data = {src: ([s for s in ext if s["source"] == src], "external", ext_guesses) for src in ("spider", "bird", "ctu")}
    data["livesqlbench (development)"] = (lsb, "external", lsb_guesses)
    result = {"model": MODEL, "sets": {}}
    OUT.mkdir(parents=True, exist_ok=True)
    t0 = time.time()
    with letters.pool(graph.WORKERS) as pool:
        for set_name in (sys.argv[1:] or SETS):
            schemas, split, guesses = data[set_name]
            schemas = [s for s in schemas if s["split"] == split and s["fk"]]
            drawn = drawn_keys(pool, schemas, split, guesses)
            rows, per_db = [], {}
            for s in schemas:
                runs = {"declared": run(snapshot(s, s["fk"]), False, ranker, names, table),
                        "rules": run(snapshot(s, []), True, ranker, names, table),
                        "model": run(snapshot(s, drawn.get(s["id"], [])), False, ranker, names, table)}
                d_ids, d_through, d_five, declared = runs["declared"]
                joins = {i for i, th in zip(d_ids, d_through) if th}
                rec = {"questions, declared": len(d_ids), "joining, declared": len(joins)}
                for r in ("rules", "model"):
                    ids, through, five, _ = runs[r]
                    asked = set(ids)
                    rec[f"questions, {r}"] = len(asked)
                    rec[f"joining still asked, {r}"] = len(joins & asked)
                    rec[f"through an undeclared key, {r}"] = len({i for i, th in zip(ids, through) if th - declared})
                    rec[f"the declared five kept, {r}"] = len(set(d_five) & set(five))
                rec["the declared five"] = len(d_five)
                per_db[s["id"]] = rec
                rows.append(rec)
            cols = ["joining, declared", "joining still asked, rules", "joining still asked, model",
                    "through an undeclared key, rules", "through an undeclared key, model",
                    "questions, rules", "questions, model", "the declared five", "the declared five kept, rules",
                    "the declared five kept, model"]
            per = np.array([[r[c] for c in cols] for r in rows], dtype=float)
            idx = np.random.default_rng(SEED).integers(0, len(per), (1000, len(per)))
            summary = {"databases": len(per), "joining questions, declared": int(per[:, 0].sum())}
            for j, r in ((1, "rules"), (2, "model")):
                summary[f"joining still asked, {r}"] = pooled(per, j, 0, idx)
                summary[f"joining still asked, {r}, mean over databases"] = macro(per, j, 0, idx)
            for j, k, r in ((3, 5, "rules"), (4, 6, "model")):
                summary[f"questions through an undeclared key, {r}"] = int(per[:, j].sum())
                summary[f"share of the run's questions through an undeclared key, {r}"] = pooled(per, j, k, idx)
            for j, r in ((8, "rules"), (9, "model")):
                summary[f"the declared five kept, {r}"] = pooled(per, j, 7, idx)
            summary["seconds"] = round(time.time() - t0)
            result["sets"][set_name] = {"summary": summary, "per database": per_db}
            print(set_name, json.dumps(summary), flush=True)
    if sys.argv[1:]:
        return
    (OUT / "questions.json").write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main()
