"""Round 1w: the language models' batch replies read against the paper's
model, a set at a time.

    python -I lms.py <out.json>

A model's replies are those batch.py and batch_anthropic.py kept in
corpus/round-1w/lm/llm/<model>/<set>/, read as round 1s reads a reply
(lm.py's `whole_schema`: names matched without case, a key naming nothing
of the schema a false one, one key a column). A set is reported for a model
only when every one of its databases was answered.

`eleven--wide` and `nine-mended` stand beside them: on plain A and style 3
from round 1v's one reading of each set (heldout-<set>.json, the counts a
database it wrote then; nothing is read again), on the other sets from
their kept scores (kept.py). Each language model is given P, R and F1 over
every declared key with a 95 % interval and macro F1, by the number of
tables, and `eleven--wide` minus it on the same resamples of databases.
"""

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import kept  # noqa: E402  (round 1v's whole and round 1s's lm on the path)

import numpy as np  # noqa: E402

import lm  # noqa: E402
import whole  # noqa: E402

LLM = whole.ROOT / "round-1w" / "lm" / "llm"
CLEAN = {"plain-a-db": "plain-a", "style3-db": "style3"}
KEPT = {"db": kept.EXTERNAL, "heldout-db": (kept.LSB,), "fresh-db": ("github fresh",)}
OURS = (kept.MODEL, kept.BASE)


def usage(d: Path) -> dict:
    recs = [json.load(open(p, encoding="utf-8")) for p in sorted(d.glob("*.json"))]
    secs = {r["batch"] for r in recs if r.get("batch")}
    return {"replies": len(recs), "input tokens": sum(r.get("input tokens") or 0 for r in recs),
            "output tokens": sum(r.get("output tokens") or 0 for r in recs),
            "reasoning tokens": sum(r.get("reasoning tokens") or 0 for r in recs),
            "finished otherwise than at the end": sum(1 for r in recs if r.get("finish") not in ("stop", "end_turn")),
            "answered by": sorted({str(r.get("answered by")) for r in recs}), "batches": len(secs),
            "asked a request at a time": sum(1 for r in recs if not r.get("batch"))}


def a_set(folder: str) -> tuple:
    """(schemas, {our model: counts a database}) in one order of databases."""
    if folder in CLEAN:
        which = CLEAN[folder]
        read = json.load(open(whole.OUT / f"heldout-{which}.json", encoding="utf-8"))
        by_id = {json.loads(line)["id"]: json.loads(line)
                 for line in open(whole.OUT / f"heldout-github-{which}" / "hidden.jsonl", encoding="utf-8")}
        schemas = [by_id[i] for i in read["databases"]]
        return schemas, {m: np.array(read["models"][m]["per database"]) for m in OURS}
    schemas, pers = None, {}
    for m in OURS:
        scs = [kept.scored(m, name) for name in KEPT[folder]]
        schemas = [s for sc in scs for s in sc.schemas]
        pers[m] = np.vstack([sc.counts(sc.baseline()) for sc in scs])
    return schemas, pers


def run(out: Path) -> None:
    kept.check_frozen()
    result = {}
    for folder in (*CLEAN, *KEPT):
        models = sorted(d.parent.name for d in LLM.glob(f"*/{folder}") if any(d.glob("*.raw")))
        if not models:
            continue
        schemas, pers = a_set(folder)
        ids = [s["id"] for s in schemas]
        truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas}
        tables = np.array([len(s["tables"]) for s in schemas])
        entry = {"databases": len(ids), "keys": int(sum(len(t) for t in truth.values()))}
        print(f"== {folder}: {entry['databases']} databases, {entry['keys']} keys")
        for m in OURS:
            entry[m] = whole.measure(pers[m], tables, pers[kept.BASE] if m != kept.BASE else None)
            print(whole.line(m, entry[m]))
        for model in models:
            preds, info = lm.whole_schema(schemas, LLM / model / folder)
            if info["unanswered"]:
                print(f"{model}: {info['unanswered']} of {len(ids)} unanswered, not reported")
                continue
            per = lm.counts(truth, preds, ids)
            m = whole.measure(per, tables)
            d = whole.measure(pers[kept.MODEL], tables, per)
            m["eleven--wide minus this, f1"] = d["minus base, f1"]
            m["eleven--wide minus this, f1_95"] = d["minus base, f1_95"]
            m["eleven--wide minus this, macro f1"] = d["minus base, macro f1"]
            m["eleven--wide minus this, macro f1_95"] = d["minus base, macro f1_95"]
            idx = np.random.default_rng(whole.SEED).integers(0, len(per), (1000, len(per)))
            ours, theirs = whole.resampled(pers[kept.MODEL], idx), whole.resampled(per, idx)
            for j, w in enumerate(("precision", "recall")):
                m[f"eleven--wide minus this, {w}"] = float(whole.prf(*pers[kept.MODEL].sum(axis=0))[j] - whole.prf(*per.sum(axis=0))[j])
                m[f"eleven--wide minus this, {w}_95"] = whole.interval(ours[:, j] - theirs[:, j])
            big = tables >= 40
            if big.any():
                b = whole.measure(pers[kept.MODEL][big], tables[big], per[big])
                m["40 tables or more"] = {k: v for k, v in whole.measure(per[big], tables[big]).items() if k != "by tables"}
                m["eleven--wide minus this, 40 tables or more, f1"] = b["minus base, f1"]
                m["eleven--wide minus this, 40 tables or more, f1_95"] = b["minus base, f1_95"]
            m["replies"] = info
            m["usage"] = usage(LLM / model / folder)
            entry[model] = m
            print(whole.line(model, m))
            print(f"    eleven--wide minus this: F1 {m['eleven--wide minus this, f1']:+.3f} "
                  f"{[round(v, 3) for v in m['eleven--wide minus this, f1_95']]}  "
                  f"P {m['eleven--wide minus this, precision']:+.3f}  R {m['eleven--wide minus this, recall']:+.3f} "
                  f"{[round(v, 3) for v in m['eleven--wide minus this, recall_95']]}  {info}")
        result[folder] = entry
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    run(Path(sys.argv[1]))
