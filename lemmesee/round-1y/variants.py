"""Round 1y: every variant as a second stage's scores and a threshold for
each size of schema; the rule of choice; the freeze.

    python -I variants.py thresholds    every variant's thresholds, from the validation split (variants.json)
    python -I variants.py choose        the development sets, the rule (development.json, choice.json)
    python -I variants.py freeze        the models and thresholds by hash, before plain A or style 3 is scored

A variant is `<first>--<kind>[@<rule>]`; its twin of the other seed is the
same with `eleven-seed2` or `eleven-pu-seed2` for its first stage. The
candidates of the rule are CANDIDATES; `eleven--wide` is the frozen model.
"""

import hashlib
import json
import pickle
import sys
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import fitting as y_fit  # noqa: E402  (round 1v and the earlier rounds on the path)

import numpy as np  # noqa: E402

import whole  # noqa: E402

import pu  # noqa: E402

V, OUT = pu.V, pu.OUT
GITHUB = ("github 2025", "github 2025, rest", "github style", "github fresh")
FROZEN = "eleven--wide"
POSTHOC = ("@size", "@size-teachers", "@size-ipw", "@ipw")
FITTED = ("eleven--wide-pu", "eleven-pu--wide", "eleven-pu--wide-pu")
CANDIDATES = [FROZEN + r for r in POSTHOC] + [f + r for f in FITTED for r in ("@declared", "@ipw")]
# read beside the rule, never its choice: the same fit weighted by a schema's own label frequency (README.md, "Added")
BESIDE = ["eleven-sar--wide-sar@declared", "eleven-sar--wide-sar@ipw"]
TWINS = ("", "-seed2")


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def parts(name: str, twin: str = "") -> tuple:
    """(first stage, kind, rule) of a variant for a seed."""
    body, _, rule = name.partition("@")
    first, kind = body.split("--")
    return first + twin, kind, rule


def twin_name(name: str, twin: str) -> str:
    first, kind, rule = parts(name, twin)
    return f"{first}--{kind}" + (f"@{rule}" if rule else "")


def validation(first: str, kind: str) -> tuple:
    val = pickle.load(open(y_fit.first_dir(first) / "validation.pkl", "rb"))
    sv = np.load(y_fit.second_dir(first, kind) / "validation.npy").astype(np.float64)
    vrec = val["pairs"]
    tables = np.array([len(s["tables"]) for s in val["schemas"]])[vrec["schema"]]
    return vrec, sv, tables


def thresholds_of(name: str, twin: str) -> dict:
    first, kind, rule = parts(name, twin)
    second = json.load(open(y_fit.second_dir(first, kind) / "second.json", encoding="utf-8"))
    t = second["threshold"]
    note = {}
    if rule in ("", "declared"):
        th = [t] * 5
    elif first in y_fit.FROZEN_FIRSTS and rule in ("size", "size-teachers"):
        knots = pu.calibration(twin)
        gt = float(pu.g_of([t], knots)[0])
        e = pu.E_RULES if rule == "size" else pu.E_TEACHERS
        th = [pu.score_at(float(x) * gt, knots) for x in e]
        note = {"g(t)": gt}
    elif first in y_fit.FROZEN_FIRSTS and rule == "size-ipw":
        knots = pu.calibration(twin)
        vrec, sv, tables = validation(first, kind)
        kept = whole.one_target(sv, vrec)
        q = np.where(kept > 0, pu.g_of(kept, knots) / pu.e_of(tables), 0.0)
        tau = pu.best_ipw_threshold(q, vrec["label"], tables)
        th = [pu.score_at(float(x) * tau, knots) for x in pu.E_RULES]
        note = {"tau": tau}
    elif rule == "ipw" and first in y_fit.FROZEN_FIRSTS and kind == "wide":
        vrec, sv, tables = validation(first, kind)
        th = [pu.best_ipw_threshold(whole.one_target(sv, vrec), vrec["label"], tables)] * 5
    elif rule == "ipw":
        th = [second["threshold, weighted F1"]] * 5
    else:
        raise SystemExit(f"no such variant: {name}")
    return {"first": first, "kind": kind, "rule": rule, "frozen threshold": t, "thresholds by size": th, **note}


def all_thresholds() -> dict:
    out = {}
    for name in [FROZEN, *CANDIDATES, *BESIDE]:
        for twin in TWINS:
            out[twin_name(name, twin)] = thresholds_of(name, twin)
            print(f"{twin_name(name, twin):36} " + " ".join(f"{x:.4f}" for x in out[twin_name(name, twin)]["thresholds by size"]))
    (OUT / "variants.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    return out


def load() -> dict:
    return json.load(open(OUT / "variants.json", encoding="utf-8"))


def draw(schemas: list, rec: dict, raw: np.ndarray, th) -> list[dict]:
    """A schema's keys: one target a column, drawn at its size's threshold."""
    kept = whole.one_target(np.asarray(raw, dtype=np.float64), rec)
    tables = np.array([len(s["tables"]) for s in schemas])
    row_t = np.asarray(th, dtype=np.float64)[pu.size_bin(tables)][rec["schema"]]
    out = [dict() for _ in schemas]
    for i in np.flatnonzero((kept >= row_t) & (kept > 0)):
        s = schemas[rec["schema"][i]]
        t = s["tables"][rec["table"][i]]
        out[rec["schema"][i]][(t["name"], t["columns"][rec["column"][i]]["name"])] = s["tables"][rec["target"][i]]["name"]
    return out


def counts(schemas: list, preds: list[dict]) -> np.ndarray:
    per = np.zeros((len(schemas), 3), dtype=np.int64)
    for i, (s, p) in enumerate(zip(schemas, preds)):
        truth = {tuple(k) for k in s["fk"]}
        drawn = {(a, c, g) for (a, c), g in p.items()}
        per[i] = [len(truth & drawn), len(drawn - truth), len(truth - drawn)]
    return per


def matched_threshold(rec: dict, raw: np.ndarray, n: int) -> float:
    """The one threshold at which a model draws n keys (or as near as its ties let it)."""
    kept = np.sort(whole.one_target(np.asarray(raw, dtype=np.float64), rec))[::-1]
    return float(kept[min(n, len(kept)) - 1])


def set_scores(first: str, kind: str, name: str) -> np.ndarray:
    return np.load(y_fit.second_dir(first, kind) / "sets" / f"{whole.slug(name)}.npy").astype(np.float64)


def measures(per: np.ndarray, tables: np.ndarray, base: np.ndarray | None = None) -> dict:
    m = whole.measure(per, tables, base)
    big = tables >= 40
    if big.any():
        m["40 tables or more"] = {k: v for k, v in whole.measure(per[big], tables[big]).items() if k != "by tables"}
    m["drawn"] = int(per[:, :2].sum())
    m["drawn, 40 tables or more"] = int(per[big][:, :2].sum())
    m["weighted"] = pu.ipw(per, tables, base)
    return m


def development() -> dict:
    """Every variant and twin on every development set of round 1v; the GitHub four pooled; SchemaPile's complete test."""
    spec = load()
    data = {}
    for name in whole.SETS:
        schemas, rec = y_fit.set_pairs(name)
        data[name] = (schemas, rec, np.array([len(s["tables"]) for s in schemas]))
    rules = pickle.load(open(V / "sets" / "schemapile-test.pkl", "rb"))["rules"]
    by = {}
    for (sid, t, c), g in rules.items():
        by.setdefault(sid, []).append((t, c, g))
    sp_schemas = data["schemapile test"][0]
    complete = np.array([all(k in {tuple(x) for x in s["fk"]} for k in by.get(s["id"], ())) for s in sp_schemas])
    pers = {}
    for v, sp in spec.items():
        pers[v] = {}
        for name, (schemas, rec, tables) in data.items():
            raw = set_scores(sp["first"], sp["kind"], name)
            pers[v][name] = counts(schemas, draw(schemas, rec, raw, sp["thresholds by size"]))
    gh_tables = np.concatenate([data[g][2] for g in GITHUB])
    out = {"variants": {}, "sets": {n: {"databases": len(d[0])} for n, d in data.items()}}
    base_gh = np.vstack([pers[FROZEN][g] for g in GITHUB])
    for v in spec:
        gh = np.vstack([pers[v][g] for g in GITHUB])
        e = {"github development sets, pooled": measures(gh, gh_tables, None if v == FROZEN else base_gh),
             "schemapile test, complete, f1": whole.prf(*pers[v]["schemapile test"][complete].sum(axis=0))[2],
             "sets": {}}
        for name, (schemas, rec, tables) in data.items():
            m = measures(pers[v][name], tables, None if v == FROZEN else pers[FROZEN][name])
            e["sets"][name] = m
        out["variants"][v] = e
        g = e["github development sets, pooled"]
        print(f"{v:40} github pooled P {g['precision']:.3f} R {g['recall']:.3f} F1 {g['f1']:.4f} | weighted P "
              f"{g['weighted']['precision']:.3f} R {g['weighted']['recall']:.3f} F1 {g['weighted']['f1']:.4f} | drawn {g['drawn']}"
              f" (40+: {g['drawn, 40 tables or more']}) | complete test {e['schemapile test, complete, f1']:.3f}", flush=True)
    out["per database"] = {v: {n: p.tolist() for n, p in d.items()} for v, d in pers.items()}
    return out


def choose() -> None:
    dev = development()
    key = lambda v: dev["variants"][v]["github development sets, pooled"]["weighted"]["f1"]  # noqa: E731
    table = {}
    for name in [FROZEN, *CANDIDATES, *BESIDE]:
        a, b = key(twin_name(name, "")), key(twin_name(name, "-seed2"))
        table[name] = {"weighted F1, seed 20261027": a, "weighted F1, seed 20261028": b, "mean": (a + b) / 2,
                       "a candidate": name in CANDIDATES}
    best = max(CANDIDATES, key=lambda n: table[n]["mean"])
    above = table[best]["mean"] > table[FROZEN]["mean"]
    fitted = best.split("@")[0] in FITTED
    other = max([c for c in CANDIDATES if (c.split("@")[0] in FITTED) != fitted], key=lambda n: table[n]["mean"])
    choice = {"chosen at": now(), "rule": "the highest weighted F1 over the four GitHub development sets pooled, the mean of two seeds",
              "table": table, "the highest candidate": best, "above the frozen model": bool(above),
              "the round's result": best if above else "no correction is chosen over the frozen model",
              "the best candidate of the other family": other,
              "read beside, a schema's own label frequency": max(BESIDE, key=lambda n: table[n]["mean"])}
    for name, r in sorted(table.items(), key=lambda kv: -kv[1]["mean"]):
        print(f"{name:34} {r['weighted F1, seed 20261027']:.4f} {r['weighted F1, seed 20261028']:.4f} mean {r['mean']:.4f}")
    print(json.dumps({k: v for k, v in choice.items() if k != "table"}, indent=1))
    (OUT / "development.json").write_text(json.dumps(dev) + "\n", encoding="utf-8")
    (OUT / "choice.json").write_text(json.dumps(choice, indent=1) + "\n", encoding="utf-8")


def sha(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def files() -> dict:
    out = {}
    for v, sp in load().items():
        d = y_fit.second_dir(sp["first"], sp["kind"])
        out[v] = {"keys-first.ubj": sha(y_fit.first_dir(sp["first"]) / "keys-first.ubj"),
                  "keys-second.txt": sha(d / "keys-second.txt"), "second.json": sha(d / "second.json")}
    return out


def freeze() -> None:
    path = OUT / "frozen.json"
    if path.exists():
        raise SystemExit("already frozen")
    choice = json.load(open(OUT / "choice.json", encoding="utf-8"))
    round_1v = json.load(open(V / "frozen.json", encoding="utf-8"))
    frozen = {"frozen at": now(), "the highest candidate": choice["the highest candidate"],
              "the round's result": choice["the round's result"], "models": files(),
              "variants.json": sha(OUT / "variants.json"), "choice.json": sha(OUT / "choice.json"),
              "calibration": {t or "seed 20261027": sha(OUT / f"calibration{t}.npz") for t in TWINS},
              "plain-a": round_1v["plain-a"], "style3": round_1v["style3"],
              "plain-b": "frozen by round 1v, unread; not read here"}
    for v in (FROZEN, "eleven-seed2--wide"):
        assert frozen["models"][v] == round_1v["models"][v], f"{v} is not the model round 1v froze"
    path.write_text(json.dumps(frozen, indent=1) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in frozen.items() if k != "models"}, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    {"thresholds": all_thresholds, "choose": choose, "freeze": freeze}[sys.argv[1]]()
