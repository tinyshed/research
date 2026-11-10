"""Round 1v: the frozen models read once on a clean set.

    python -I clean.py freeze-models <choice> [<other> ...]   by hash, before any clean set is parsed
    python -I clean.py freeze-set <set>                       a set's hash, before it is read
    python -I clean.py read <set>                             the frozen models, once

A model of this round is named `<first>--<kind>[-<tag>][@declared]`
(second.py); a blend of two is `<a>+<b>[@<weight of a>]` (blend.py), its
threshold the one blend.py read on the validation split; `1s` and
`nine-mended` are round 1s's and round 1t's as they were frozen there. A set
is a folder of corpus/round-1v/ written by plain.py: `plain-a`, `plain-b`,
`style3`.

`read` refuses a set read before. It writes the per-database counts, the
round's measures for each model, each minus `nine-mended` and minus `1s` on
the same resamples, and beside them every key each model drew, for the judge
and for the reading afterwards.
"""

import hashlib
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import second as second_stage  # noqa: E402  (the earlier rounds on the path, the trigrams sorted)

import lightgbm as lgb  # noqa: E402
import numpy as np  # noqa: E402

import cues  # noqa: E402
import first as first_stage  # noqa: E402
import fit as round_1t  # noqa: E402
import graph  # noqa: E402
import keys  # noqa: E402
import letters  # noqa: E402
import stack  # noqa: E402
import ten  # noqa: E402
import whole  # noqa: E402
import wide  # noqa: E402
import widen  # noqa: E402

OUT = whole.OUT
OLD = ("1s", "seven-bounded", "nine-mended")


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def sha(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def parts(name: str) -> tuple:
    """A round 1v model's first stage, the folder of its second, and which threshold."""
    which = ""
    if name.endswith("@declared"):
        name, which = name[:-len("@declared")], "declared"
    first = name.split("--")[0]
    return first, OUT / "second" / name, which


def blend_of(name: str) -> tuple:
    """(a, b, the weight of a) of a blend's name."""
    body, _, w = name.partition("@")
    a, b = body.split("+")
    return a, b, float(w) if w else 0.5


def files(name: str) -> dict:
    if name in OLD:
        d = round_1t.MODELS[name][0]
        return {f: sha(d / f) for f in ("keys-first.ubj", "keys-second.txt", "graph-final.json")}
    if "+" in name:
        a, b, _ = blend_of(name)
        return {"parts": {a: files(a), b: files(b)}, "blend": sha(OUT / "scored" / f"{name}.json")}
    first, d, _ = parts(name)
    return {"keys-first.ubj": sha(OUT / "first" / first / "keys-first.ubj"),
            "keys-second.txt": sha(d / "keys-second.txt"), "second.json": sha(d / "second.json")}


def freeze_models(names: list) -> None:
    path = OUT / "frozen.json"
    if path.exists():
        raise SystemExit("already frozen")
    frozen = {"frozen at": now(), "the round's choice": names[0], "models": {n: files(n) for n in names}}
    path.write_text(json.dumps(frozen, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(frozen, indent=1))


def freeze_set(which: str) -> None:
    path = OUT / "frozen.json"
    frozen = json.load(open(path, encoding="utf-8"))
    if which in frozen:
        raise SystemExit("the set is frozen already")
    frozen[which] = sha(OUT / f"heldout-github-{which}" / "hidden.jsonl")
    frozen[f"{which}, frozen at"] = now()
    path.write_text(json.dumps(frozen, indent=1) + "\n", encoding="utf-8")
    print(which, frozen[which], frozen[f"{which}, frozen at"])


def drawn_old(pool, name: str, schemas: list) -> tuple:
    d, variant = round_1t.MODELS[name]
    first, second = widen.load_models(d)
    final = json.load(open(d / "graph-final.json", encoding="utf-8"))
    x, meta = cues.build_all(pool, schemas, "external", {}, variant)
    raw = second.predict(stack.of(final)(meta, first.predict_proba(x)[:, 1]))
    kept = keys.one_target(raw, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
    where = {s["id"]: i for i, s in enumerate(schemas)}
    out = [dict() for _ in schemas]
    for i in np.flatnonzero(kept >= final["threshold"]):
        m = meta[i]
        out[where[m[graph.SID]]][(m[graph.TABLE], m[graph.COLUMN])] = m[graph.TARGET]
    return out, int(sum(m[graph.LABEL] for m in meta))


def raw_new(pool, name: str, schemas: list, cache: dict) -> tuple:
    """A round 1v model's rows of the set, its raw scores of them and its threshold."""
    import xgboost as xgb  # noqa: E402
    first, d, which = parts(name)
    result = json.load(open(d / "second.json", encoding="utf-8"))
    threshold = result["validation, declarations only"]["threshold"] if which == "declared" else result["threshold"]
    people = bool(first_stage.FITS.get(first, {}).get("people"))
    if ("pairs", people) not in cache:
        cache[("pairs", people)] = ten.build_all(pool, schemas, "external", {}, people)
    x, meta = cache[("pairs", people)]
    if first not in cache:
        model = xgb.XGBClassifier()
        model.load_model(str(OUT / "first" / first / "keys-first.ubj"))
        model.set_params(device="cpu")
        rec = first_stage.compact(meta, schemas)
        rec["first"] = model.predict_proba(x)[:, 1].astype(np.float32)
        cache[first] = rec
    rec = cache[first]
    if d.name not in cache:
        feats = wide.features(pool, result["kind"], schemas, rec, rec["first"])
        cache[d.name] = lgb.Booster(model_file=str(d / "keys-second.txt")).predict(feats)
    return rec, cache[d.name], threshold


def drawn_new(pool, name: str, schemas: list, cache: dict) -> tuple:
    if "+" in name:
        # this round's blend.py by its path: round 1t has a module of the same name earlier on the path
        import importlib.util  # noqa: E402
        spec = importlib.util.spec_from_file_location("blend_1v", HERE / "blend.py")
        blend = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(blend)
        a, b, w = blend_of(name)
        rec, raw_a, _ = raw_new(pool, a, schemas, cache)
        rec_b, raw_b, _ = raw_new(pool, b, schemas, cache)
        ka, kb = blend.row_keys(rec), blend.row_keys(rec_b)
        oa, ob = np.argsort(ka, kind="stable"), np.argsort(kb, kind="stable")
        assert len(ka) == len(kb) and (ka[oa] == kb[ob]).all(), "the two models scored different pairs"
        other = np.empty_like(raw_b)
        other[oa] = raw_b[ob]
        raw = w * raw_a + (1 - w) * other
        threshold = json.load(open(OUT / "scored" / f"{name}.json", encoding="utf-8"))["threshold"]
    else:
        rec, raw, threshold = raw_new(pool, name, schemas, cache)
    kept = whole.one_target(np.asarray(raw, dtype=np.float64), rec)
    return second_stage.predictions(rec, schemas, kept, threshold), int(rec["label"].sum())


def read(which: str) -> None:
    assert letters.is_sorted()
    frozen = json.load(open(OUT / "frozen.json", encoding="utf-8"))
    done = OUT / f"heldout-{which}.json"
    if done.exists():
        raise SystemExit("this set has been read once already")
    hidden = OUT / f"heldout-github-{which}" / "hidden.jsonl"
    assert sha(hidden) == frozen[which], "the set changed since it was frozen"
    for n, f in frozen["models"].items():
        assert files(n) == f, f"{n} changed since it was frozen"
    schemas = [json.loads(line) for line in open(hidden, encoding="utf-8")]
    truth = [{tuple(k) for k in s["fk"]} for s in schemas]
    tables = np.array([len(s["tables"]) for s in schemas])
    result = {"set": which, "read at": now(), "frozen": frozen, "databases": [s["id"] for s in schemas], "models": {}}
    drawn = {}
    cache = {}
    with letters.pool(graph.WORKERS) as pool:
        for n in frozen["models"]:
            if n in OLD:
                cache.clear()  # the earlier rounds' models build their own pairs: this round's are let go first
            preds, held = (drawn_old(pool, n, schemas) if n in OLD else drawn_new(pool, n, schemas, cache))
            per = np.array([[len(t & {(a, c, g) for (a, c), g in p.items()}),
                             len({(a, c, g) for (a, c), g in p.items()} - t),
                             len(t - {(a, c, g) for (a, c), g in p.items()})] for t, p in zip(truth, preds)])
            result["models"][n] = {"per database": per.tolist(), "candidates hold": held}
            drawn[n] = [[[a, c, g] for (a, c), g in sorted(p.items())] for p in preds]
            print(f"{n}: read", flush=True)
    pers = {n: np.array(v["per database"]) for n, v in result["models"].items()}
    big = tables >= 40
    for n, per in pers.items():
        m = whole.measure(per, tables)
        for base in ("nine-mended", "1s"):
            if base in pers and base != n:
                b = whole.measure(per, tables, pers[base])
                for k in ("f1", "f1_95", "macro f1", "macro f1_95"):
                    m[f"minus {base}, {k}"] = b[f"minus base, {k}"]
                if big.any():
                    bb = whole.measure(per[big], tables[big], pers[base][big])
                    m[f"minus {base}, 40 tables or more, f1"] = bb["minus base, f1"]
                    m[f"minus {base}, 40 tables or more, f1_95"] = bb["minus base, f1_95"]
        if big.any():
            m["40 tables or more"] = {k: v for k, v in whole.measure(per[big], tables[big]).items() if k != "by tables"}
        result["models"][n].update(m)
        print(whole.line(n, m))
        for base in ("nine-mended", "1s"):
            if f"minus {base}, f1" in m:
                print(f"    minus {base}: F1 {m[f'minus {base}, f1']:+.3f} {[round(v, 3) for v in m[f'minus {base}, f1_95']]}"
                      f"  macro {m[f'minus {base}, macro f1']:+.3f} {[round(v, 3) for v in m[f'minus {base}, macro f1_95']]}"
                      + (f"  40 tables or more {m[f'minus {base}, 40 tables or more, f1']:+.3f} "
                         f"{[round(v, 3) for v in m[f'minus {base}, 40 tables or more, f1_95']]}"
                         if f"minus {base}, 40 tables or more, f1" in m else ""))
    done.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")
    (OUT / f"heldout-{which}-drawn.json").write_text(json.dumps(drawn) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "freeze-models":
        freeze_models(sys.argv[2:])
    elif cmd == "freeze-set":
        freeze_set(sys.argv[2])
    else:
        read(sys.argv[2])
