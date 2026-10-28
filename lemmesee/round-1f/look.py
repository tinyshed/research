"""Round 1f: round 1e's look taught on twice the databases, and whether the
count of databases is what limits it.

    python -I look.py pool <round 1c learn> <round 1d dir> <corpus/see> <out> <fixture-engine>...
    python -I look.py pack <out> <fixture-engine>...
    python -I look.py evaluate <round 1d look> <round 1e look> <out> <corpus/see> <round 1d dir>

pool and pack are round 1d's, into round 1f's folder; evaluate reads all
three folders with their answers and writes <out>/evaluate.json.
"""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


look1e = _load("look1e", HERE.parent / "round-1e" / "look.py")
look1d = look1e.look1d

SEED = 20261031
HELD_OUT = ("birdaddress", "birdcardgames", "birdcoinmarketcap", "birdcookbook", "birdhumanresources", "birdlegislator",
            "birdmenu", "birdmovie", "birdsoccer2016", "birdsuperstore", "birdsynthea", "birdvideogames")
# what round 1e's tuning chose in 36 of its 38 folds: the look alone orders
FIXED = ({"base": 0.0, "measure": 0.0, "time_same": 0.25, "time_other": 0.0, "by_same": 0.25, "by_other": 0.25,
          "shape": 0.0}, float("inf"))


def evaluate(d1d_look: Path, d1e_look: Path, out: Path, see_dir: Path, d1d: Path) -> None:
    lam1d = json.loads((d1d / "penalties.json").read_text(encoding="utf-8"))["penalties"]
    base = lambda fe: fe.rsplit("-", 1)[0]
    folders = {d: sorted(p.parent.name for p in d.glob("*/pool.json")) for d in (d1d_look, d1e_look, out)}
    old, mid, new = folders[d1d_look], folders[d1e_look], folders[out]
    held = [fe for fe in new if base(fe) in HELD_OUT]
    taught_1e = old + [fe for fe in mid if base(fe) not in look1e.HELD_OUT]
    taught = old + mid + [fe for fe in new if fe not in held]
    home = {fe: d for d, fes in folders.items() for fe in fes}
    pools = {fe: json.loads((home[fe] / fe / "pool.json").read_text(encoding="utf-8"))["panels"] for fe in home}
    ans = {}
    for j in look1e.TEACHERS + look1e.COUNTERS:
        ans[j] = {}
        for d in folders:
            ans[j] |= look1d.labels(d, j)
    ident, qfeat = {}, {}
    for fe in home:
        ident[fe], qfeat[fe] = look1e.questions(see_dir, fe)
        for p in pools[fe]:
            p["question"] = qfeat[fe].get(p["title"], {})
            p["more"] = look1e.more(p)
    look1d.QUESTION[:] = sorted({k for fe in home for f in qfeat[fe].values() for k in f})
    mine = {fe: [p for p in pools[fe] if "round 1" not in p["in"]] for fe in home}
    choosers = {fe: look1e.Chooser([ident[fe].get(p["title"], ("?", p["title"])) for p in mine[fe]]) for fe in old + held}
    X = {fe: np.array([look1e.vector(p) for p in mine[fe]], dtype=float) for fe in home}

    def one(fe, model) -> dict:
        y = np.array([np.nan if (t := look1e.target(p, ans)) is None else t for p in mine[fe]])
        if fe not in choosers:
            choosers[fe] = look1e.Chooser([ident[fe].get(p["title"], ("?", p["title"])) for p in mine[fe]])
        return {"oof": look1e.logit(model.predict(X[fe])), "score": np.array([p["score"] for p in mine[fe]]),
                "chooser": choosers[fe], "y": y}

    def scored(taught: list[str], left_out: list[str]) -> dict:
        """The taught each scored by a look that never saw it; those left
        out, the twelve and the 28 in turn, by one that saw none of them."""
        data = {fe: one(fe, look1e.fit_look([p for f in taught if f != fe for p in pools[f]], ans)) for fe in taught}
        everyone = look1e.fit_look([p for f in taught for p in pools[f]], ans)
        for fe in left_out:
            if fe not in data:
                data[fe] = one(fe, everyone)
        return data

    def fives(data: dict, taught: list[str], where: list[str]) -> tuple[dict, dict]:
        per, params = {}, {}
        for fe in where:
            lam, w = look1e.tune([d for d in taught if d != fe], data, lam1d, 2.0)
            params[base(fe)] = {"penalties": lam, "weight": w}
            x = data[fe]
            per[fe] = [mine[fe][i]["id"] for i in x["chooser"].choose(look1e.combined(x["score"], x["oof"], w), lam)]
        return per, params

    def fixed(data: dict, where: list[str]) -> dict:
        lam, w = FIXED
        return {fe: [mine[fe][i]["id"] for i in data[fe]["chooser"].choose(
            look1e.combined(data[fe]["score"], data[fe]["oof"], w), lam)] for fe in where}

    # round 1d's look, taught by Sonnet on the 28
    q_now = list(look1d.QUESTION)
    look1d.QUESTION[:] = sorted({k for fe in old for f in qfeat[fe].values() for k in f if k != "sibling_key"})
    sonnet_1d = {}
    for fe in old + held:
        model = look1d.fit_look([p for f in old if f != fe for p in pools[f]], ans["sonnet"])
        lg = look1e.logit(model.predict(np.array([look1d.vector(p) for p in mine[fe]], dtype=float)))
        sonnet_1d[fe] = [mine[fe][i]["id"] for i in choosers[fe].choose(np.array([p["score"] for p in mine[fe]]) + 2.0 * lg, lam1d)]
    look1d.QUESTION[:] = q_now

    data_1e = scored(taught_1e, held)
    data_1f = scored(taught, held)
    systems = {
        "round 1, B1": {fe: [p["id"] for p in pools[fe] if "round 1" in p["in"]] for fe in old + held},
        "round 1c": {fe: [p["id"] for p in pools[fe] if "round 1c" in p["in"]] for fe in old + held},
        "round 1d, the look taught by Sonnet": sonnet_1d,
    }
    systems["round 1e, taught on its 41"], _ = fives(data_1e, taught_1e, old + held)
    systems["round 1f, taught on 81"], params = fives(data_1f, taught, old + held)

    def kept(judge, five):
        return sum(1 for pid in five if ans[judge].get(pid, {}).get("keep"))

    result: dict = {"taught on": len(taught), "held out": len(held)}
    rng = np.random.default_rng(SEED)
    for part, dbs in (("round 1d's 28, each left out", old), ("the twelve held out", held)):
        covered = {j: [fe for fe in dbs if all(pid in ans[j] for s in systems.values() for pid in s.get(fe, []))]
                   for j in look1e.COUNTERS}
        block: dict = {"databases counted": {j: len(c) for j, c in covered.items()}}
        for name, per in systems.items():
            block[name] = {j: round(float(np.mean([kept(j, per[fe]) for fe in covered[j]])), 2)
                           for j in look1e.COUNTERS if covered[j]}
        block["ceiling"] = {j: round(float(np.mean([min(5, len({ident[fe].get(p["title"]) for p in mine[fe]
                                                                   if ans[j].get(p["id"], {}).get("keep")}))
                                                     for fe in covered[j]])), 2) for j in look1e.COUNTERS if covered[j]}
        for other in ("round 1, B1", "round 1c", "round 1d, the look taught by Sonnet", "round 1e, taught on its 41"):
            for j in look1e.COUNTERS:
                if not covered[j] or (j == "sonnet" and other.endswith("Sonnet")):
                    continue
                d = np.array([kept(j, systems["round 1f, taught on 81"][fe]) - kept(j, systems[other][fe])
                              for fe in covered[j]], float)
                boots = [rng.choice(d, len(d)).mean() for _ in range(10000)]
                block[f"round 1f − {other}, {j}"] = {"mean": round(float(d.mean()), 2),
                                                      "95%": [round(float(np.percentile(boots, q)), 2) for q in (2.5, 97.5)]}
        for a in look1e.TEACHERS + look1e.COUNTERS:
            for b in look1e.COUNTERS:
                if a < b or a in look1e.TEACHERS:
                    both = [p["id"] for fe in dbs for p in mine[fe] if p["id"] in ans[a] and p["id"] in ans[b]]
                    if both and a != b:
                        block[f"{a} and {b} agree on keep"] = round(float(np.mean(
                            [bool(ans[a][i]["keep"]) == bool(ans[b][i]["keep"]) for i in both])), 3)
        result[part] = block

    # the learning curve on the twelve: the look taught on fewer databases,
    # the choice fixed as round 1e tuned it
    curve = {}
    pick = np.random.default_rng(SEED)
    for size in (10, 20, 40, 60, len(taught)):
        runs = []
        for _ in range(1 if size == len(taught) else 5):
            some = list(pick.choice(taught, size, replace=False)) if size < len(taught) else taught
            model = look1e.fit_look([p for f in some for p in pools[f]], ans)
            per = fixed({fe: one(fe, model) for fe in held}, held)
            runs.append({j: float(np.mean([kept(j, per[fe]) for fe in held])) for j in ("sonnet", "gemini")})
        curve[size] = {j: round(float(np.mean([r[j] for r in runs])), 2) for j in ("sonnet", "gemini")}
        curve[size]["draws"] = len(runs)
    result["learning curve on the twelve, kept of five"] = curve
    result["penalties and weight, per database"] = params
    result["fives"] = {name: per for name, per in systems.items()}
    (out / "evaluate.json").write_text(json.dumps(result, indent=1, ensure_ascii=False), encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k not in ("fives", "penalties and weight, per database")},
                     indent=1, ensure_ascii=False))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "pool":
        look1d.pool(Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]), Path(sys.argv[5]), sys.argv[6:], 30)
    elif cmd == "pack":
        look1d.pack(Path(sys.argv[2]), sys.argv[3:])
    elif cmd == "evaluate":
        evaluate(*(Path(a) for a in sys.argv[2:7]))
