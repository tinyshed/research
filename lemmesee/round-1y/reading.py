"""Round 1y: every frozen variant read on plain A or style 3, development
data since round 1v's one reading of each.

    python -I reading.py plain-a | style3

Refuses any other set: plain B is frozen and unread and no code here opens
it. Refuses before the variants are frozen (variants.py freeze), and if a
frozen file changed. The set's pairs are built once and every second
stage's raw scores kept (corpus/round-1y/heldout/<set>/). The first thing
read is the frozen model from the rebuilt pairs: it must give round 1v's
count of every database, or nothing else is read.

Written: heldout-<set>.json, the measures of every variant, its twin and its
controls on the frozen model, by size, declared and weighted, each minus the
frozen model of its seed on the same resamples; heldout-<set>-drawn.json,
every key each draws.
"""

import json
import pickle
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import variants  # noqa: E402  (fitting, round 1v and the earlier rounds on the path)

import lightgbm as lgb  # noqa: E402
import numpy as np  # noqa: E402
import scipy.sparse as sp  # noqa: E402

import first as v_first  # noqa: E402
import graph  # noqa: E402
import letters  # noqa: E402
import ten  # noqa: E402
import whole  # noqa: E402
import wide  # noqa: E402

import fitting  # noqa: E402
import locks  # noqa: E402
import pu  # noqa: E402

V, OUT = pu.V, pu.OUT
SETS = ("plain-a", "style3")
POOL = fitting.POOL   # processes; graph.WORKERS orders the rows and stays the recipe's eight


def scores(which: str) -> tuple:
    """(schemas, rows, {<first>--<kind>: raw scores}), built once and kept."""
    import xgboost as xgb  # noqa: E402
    assert which in SETS, "this round reads plain A and style 3 only"
    frozen = json.load(open(OUT / "frozen.json", encoding="utf-8"))
    hidden = V / f"heldout-github-{which}" / "hidden.jsonl"
    assert variants.sha(hidden) == frozen[which], "the set is not the one round 1v froze"
    assert variants.files() == frozen["models"], "a model changed since it was frozen"
    assert variants.sha(OUT / "variants.json") == frozen["variants.json"], "a threshold changed since it was frozen"
    schemas = [json.loads(line) for line in open(hidden, encoding="utf-8")]
    d = OUT / "heldout" / which
    d.mkdir(parents=True, exist_ok=True)
    spec = variants.load()
    stages = sorted({(sp["first"], sp["kind"]) for sp in spec.values()})
    # every step is kept as it is made, so that a run lost to memory goes on from where it stopped
    if not (d / "rows.pkl").exists():
        with letters.pool(POOL) as pool:
            with locks.held("pairs", f"{which}'s pairs"):
                x, meta = ten.build_all(pool, schemas, "external", {}, True)
                rec = v_first.compact(meta, schemas)
                del meta
        sp.save_npz(d / "pairs.npz", x, compressed=False)
        del x
        pickle.dump(rec, open(d / "rows.pkl", "wb"), protocol=4)
    rec = pickle.load(open(d / "rows.pkl", "rb"))
    x = None
    for f in sorted({s[0] for s in stages}):
        if not (d / f"first-{f}.npy").exists():
            x = sp.load_npz(d / "pairs.npz") if x is None else x
            model = xgb.XGBClassifier()
            model.load_model(str(fitting.first_dir(f) / "keys-first.ubj"))
            model.set_params(device="cpu")
            np.save(d / "part.npy", model.predict_proba(x)[:, 1].astype(np.float32))
            (d / "part.npy").replace(d / f"first-{f}.npy")
            print(f"{which}: first stage {f}", flush=True)
    del x
    for f, kind in stages:
        if not (d / f"{f}--{kind}.npy").exists():
            with letters.pool(POOL) as pool:
                feats = wide.features(pool, "wide", schemas, rec, np.load(d / f"first-{f}.npy"))
            raw = lgb.Booster(model_file=str(fitting.second_dir(f, kind) / "keys-second.txt")).predict(feats)
            del feats
            np.save(d / "part.npy", raw)
            (d / "part.npy").replace(d / f"{f}--{kind}.npy")
            print(f"{which}: second stage {f}--{kind}", flush=True)
    return schemas, rec, {f"{f}--{k}": np.load(d / f"{f}--{k}.npy") for f, k in stages}


def read(which: str) -> None:
    assert letters.is_sorted()
    schemas, rec, raws = scores(which)
    spec = variants.load()
    tables = np.array([len(s["tables"]) for s in schemas])
    bins = pu.size_bin(tables)
    before = json.load(open(V / f"heldout-{which}.json", encoding="utf-8"))
    assert before["databases"] == [s["id"] for s in schemas]
    drawn, pers = {}, {}

    def take(name, raw, th):
        preds = variants.draw(schemas, rec, raw, th)
        drawn[name] = [[[a, c, g] for (a, c), g in sorted(p.items())] for p in preds]
        pers[name] = variants.counts(schemas, preds)

    for v in (variants.FROZEN, "eleven-seed2--wide"):
        take(v, raws[v], spec[v]["thresholds by size"])
        was = np.array(before["models"][v]["per database"])
        if not (pers[v] == was).all():
            raise SystemExit(f"{v} does not read as round 1v read it: {pers[v].sum(axis=0)} against {was.sum(axis=0)}")
        print(f"{v}: {int(pers[v][:, :2].sum())} keys drawn, every database as round 1v read it", flush=True)
    controls = {}
    for v, sp in spec.items():
        if v in pers:
            continue
        take(v, raws[f"{sp['first']}--{sp['kind']}"], sp["thresholds by size"])
        twin = "-seed2" if "seed2" in sp["first"] else ""
        base = "eleven" + twin + "--wide"
        n = int(pers[v][:, :2].sum())
        t = variants.matched_threshold(rec, raws[base], n)
        take(v + " | matched", raws[base], [t] * 5)
        by_size = []
        for b in range(5):
            m = bins[rec["schema"]] == b
            nb = int(pers[v][bins == b][:, :2].sum())
            sub = {k: a[m] for k, a in rec.items() if isinstance(a, np.ndarray) and len(a) == len(m)}
            by_size.append(variants.matched_threshold(sub, raws[base][m], nb) if nb and m.any() else 2.0)
        take(v + " | matched-by-size", raws[base], by_size)
        controls[v] = {"matched": t, "matched-by-size": by_size}
    result = {"set": which, "read at": variants.now(), "databases": [s["id"] for s in schemas],
              "tables": tables.tolist(), "controls' thresholds": controls, "models": {}}
    for name, per in pers.items():
        twin = "-seed2" if "seed2" in name.split(" | ")[0].split("--")[0] else ""
        base = "eleven" + twin + "--wide"
        m = variants.measures(per, tables, None if name == base else pers[base])
        big = tables >= 40
        if name != base and big.any():
            b = whole.measure(per[big], tables[big], pers[base][big])
            m["40 tables or more"]["minus base, f1"] = b["minus base, f1"]
            m["40 tables or more"]["minus base, f1_95"] = b["minus base, f1_95"]
        m["per database"] = per.tolist()
        result["models"][name] = m
        w = m["weighted"]
        print(f"{name:52} P {m['precision']:.3f} R {m['recall']:.3f} F1 {m['f1']:.3f}"
              + (f" ({m['minus base, f1']:+.3f} [{m['minus base, f1_95'][0]:+.3f},{m['minus base, f1_95'][1]:+.3f}])" if "minus base, f1" in m else "")
              + f" | 40+: P {m['40 tables or more']['precision']:.3f} R {m['40 tables or more']['recall']:.3f} F1 {m['40 tables or more']['f1']:.3f}"
              f" | weighted F1 {w['f1']:.3f} | drawn {m['drawn']} (40+: {m['drawn, 40 tables or more']})", flush=True)
    (OUT / f"heldout-{which}.json").write_text(json.dumps(result) + "\n", encoding="utf-8")
    (OUT / f"heldout-{which}-drawn.json").write_text(json.dumps(drawn) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    read(sys.argv[1])
