"""Round 1e's look: round 1d's, taught by the mean keep of three cheap
judges over more databases, reading more of each result, with the look's
weight and the five's penalties tuned on what the teachers keep of the five.

    python -I look.py pool <round 1c learn> <round 1d dir> <corpus/see> <out> <fixture-engine>...
    python -I look.py pack <out> <fixture-engine>...
    python -I look.py evaluate <round 1d look> <out> <corpus/see> <round 1d dir>

pool and pack are round 1d's, into round 1e's own folder; evaluate reads
round 1d's 28 databases and round 1e's 23 with their answers, and writes
<out>/evaluate.json.
"""

from __future__ import annotations

import importlib.util
import json
import sys
from collections import Counter
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE.parent / "round-1d"))
sys.path.insert(0, str(HERE.parent / "round-1c"))
import choose  # noqa: E402
import see  # noqa: E402

# round 1d's look.py, by its path: this file has its name
_spec = importlib.util.spec_from_file_location("look1d", HERE.parent / "round-1d" / "look.py")
look1d = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(look1d)
from aq import generate, identity, load  # noqa: E402

SEED = 20261030
TEACHERS = ("jev", "deepseek", "glm")
COUNTERS = ("opus", "sonnet", "gemini")
HELD_OUT = ("accidents", "cdeschools", "consumerexpenditures", "fnhk", "hepatitis", "legalacts", "nhldraft", "sap",
            "seznam", "walmart2014")
NAN = float("nan")


def _number(x) -> bool:
    try:
        float(str(x).strip())
        return True
    except ValueError:
        return False


def more(p: dict) -> dict[str, float]:
    """What round 1d's judges named wrong that its numbers missed: a label
    twice, codes for labels, a stray year, a placeholder date, ties at the
    cut."""
    rows = p.get("rows") or []
    f = {k: NAN for k in ("repeated_labels", "numeric_labels", "label_length", "ties_at_cut", "middle_span",
                          "first_share", "largest_gap", "unparsed_time")}
    if not rows:
        return f
    vals = np.array([v for v in (look1d.num(r[-1]) for r in rows) if v is not None], dtype=float)
    if len(vals) >= 2:
        f["ties_at_cut"] = float((vals == vals[-1]).mean())
    if p.get("bucket") is None:
        if len(rows[0]) > 1:
            labels = [r[0] for r in rows]
            named = [str(x).strip() for x in labels if x is not None and str(x).strip()]
            counts = Counter(named)
            f["repeated_labels"] = sum(n for n in counts.values() if n > 1) / len(labels)
            if named:
                f["numeric_labels"] = sum(1 for x in named if _number(x)) / len(named)
                f["label_length"] = float(np.mean([len(x) for x in named]))
        return f
    days = [see.date_days(str(r[0])) for r in rows]
    f["unparsed_time"] = sum(1 for d in days if d is None) / len(days)
    pts = sorted((d, abs(look1d.num(r[-1]) or 0.0)) for d, r in zip(days, rows) if d is not None)
    if len(pts) >= 2:
        span = pts[-1][0] - pts[0][0]
        total = sum(w for _, w in pts) or 1.0
        cum, lo, hi = 0.0, None, None
        for d, w in pts:
            cum += w
            if lo is None and cum >= 0.05 * total:
                lo = d
            if hi is None and cum >= 0.95 * total:
                hi = d
        # every value zero or none: no middle to find
        f["middle_span"] = (hi - lo) / span if span > 0 and lo is not None and hi is not None else NAN
        f["first_share"] = pts[0][1] / total
        f["largest_gap"] = max(b[0] - a[0] for a, b in zip(pts, pts[1:])) / span if span > 0 else NAN
    return f


MORE = ["repeated_labels", "numeric_labels", "label_length", "ties_at_cut", "middle_span", "first_share",
        "largest_gap", "unparsed_time"]


def sibling(schema, q) -> float:
    """Whether a breakdown or a time is reached by one of several keys from
    one table to another: a match's home team, its away team."""
    for d in (q.by, q.time):
        if d is None:
            continue
        for t, c in d.path:
            parent = schema.tables[t].columns[c].fk
            if sum(1 for _, p in schema.tables[t].out if p == parent) > 1:
                return 1.0
    return 0.0


def questions(see_dir: Path, fe: str) -> tuple[dict, dict]:
    """Each title's identity, and the ranker's features with the sibling key."""
    snap = json.loads((see_dir / fe / "snapshot.json").read_text(encoding="utf-8"))
    schema = load(snap, True)
    import rank
    ids, feats = {}, {}
    for q in generate(schema):
        t = q.title(schema)
        if t not in ids:
            ids[t] = identity(schema, q)
            feats[t] = dict(rank.features(schema, q), sibling_key=sibling(schema, q))
    return ids, feats


def pool(learn_dir: Path, d1d: Path, see_dir: Path, out: Path, fixtures: list[str]) -> None:
    look1d.pool(learn_dir, d1d, see_dir, out, fixtures, 30)


def pack(out: Path, fixtures: list[str]) -> None:
    look1d.pack(out, fixtures)


def vector(p: dict, extra: bool = True) -> list[float]:
    """Round 1d's numbers and, unless measured without them, round 1e's."""
    v = look1d.vector(p)
    if not extra:
        return [x for k, x in zip(look1d.RESULT + ["score", "rank"] + look1d.QUESTION, v) if k != "sibling_key"]
    return v + [p["more"].get(k, NAN) for k in MORE]


def target(p: dict, ans: dict[str, dict]) -> float | None:
    """The teachers' mean keep: Jev's probability, the others' verdicts."""
    got = []
    for j in TEACHERS:
        a = ans[j].get(p["id"])
        if a is not None:
            got.append(float(a["p_keep"]) if "p_keep" in a else float(bool(a["keep"])))
    return float(np.mean(got)) if got else None


def fit_look(panels: list[dict], ans: dict[str, dict], extra: bool = True):
    import lightgbm as lgb
    rows = [(vector(p, extra), t) for p in panels if (t := target(p, ans)) is not None]
    X = np.array([r[0] for r in rows], dtype=float)
    y = np.array([r[1] for r in rows], dtype=float)
    params = dict(objective="cross_entropy", learning_rate=0.05, num_leaves=4, min_data_in_leaf=10,
                  feature_fraction=0.5, bagging_fraction=0.9, bagging_freq=1, lambda_l2=1.0, verbose=-1, seed=SEED)
    return lgb.train(params, lgb.Dataset(X, y), num_boost_round=150)


def logit(p) -> np.ndarray:
    p = np.clip(np.asarray(p, dtype=float), 1e-3, 1 - 1e-3)
    return np.log(p / (1 - p))


class Chooser:
    """A database's candidates and what each two share, once; a five under
    any scores and penalties is then a few array sums, as round 1d's Pool."""

    def __init__(self, ids: list):
        n = len(ids)
        self.F = np.zeros((n, n, len(choose.SHARED)), dtype=np.float32)
        self.dup = np.zeros((n, n), dtype=bool)
        for a in range(n):
            for b in range(a + 1, n):
                v = choose.shared(ids[a], ids[b])
                self.F[a, b] = self.F[b, a] = [v[k] for k in choose.SHARED]
                self.dup[a, b] = self.dup[b, a] = ids[a] == ids[b]

    def choose(self, scores, lam: dict, k: int = 5) -> list[int]:
        P = self.F @ np.array([lam[n] for n in choose.SHARED], dtype=np.float32)
        at = np.asarray(scores, dtype=float).copy()
        free = np.ones(len(at), dtype=bool)
        chosen = []
        for _ in range(min(k, len(at))):
            cand = np.where(free, at, -np.inf)
            j = int(np.argmax(cand))
            if not np.isfinite(cand[j]):
                break
            chosen.append(j)
            free[j] = False
            free &= ~self.dup[j]
            at = at - P[:, j]
        return chosen


WEIGHTS = (0.0, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0)


def combined(score, lg, w: float):
    # past the last weight the look alone orders, the score only breaking ties
    return lg + 1e-3 * score if w == float("inf") else score + w * lg


def tune(dbs: list[str], data: dict, start: dict, start_w: float) -> tuple[dict, float]:
    """The penalties and the look's weight by coordinate ascent, from round
    1d's, on the teachers' mean keep of the five over these databases."""
    def objective(lam, w):
        tot = 0.0
        for d in dbs:
            x = data[d]
            five = x["chooser"].choose(combined(x["score"], x["oof"], w), lam)
            tot += float(np.nansum(x["y"][five]))
        return tot / len(dbs)
    lam, w = dict(start), start_w
    best = objective(lam, w)
    for _ in range(3):
        moved = False
        for v in WEIGHTS + (float("inf"),):
            o = objective(lam, v)
            if o > best + 1e-9:
                best, w, moved = o, v, True
        for n in choose.SHARED:
            for v in choose.GRID:
                trial = dict(lam, **{n: v})
                o = objective(trial, w)
                if o > best + 1e-9:
                    best, lam, moved = o, trial, True
        if not moved:
            break
    return lam, w


def evaluate(d1d_look: Path, out: Path, see_dir: Path, d1d: Path) -> None:
    lam1d = json.loads((d1d / "penalties.json").read_text(encoding="utf-8"))["penalties"]
    old = sorted(p.parent.name for p in d1d_look.glob("*/pool.json"))
    new = sorted(p.parent.name for p in out.glob("*/pool.json"))
    held = [fe for fe in new if fe.rsplit("-", 1)[0] in HELD_OUT]
    taught = old + [fe for fe in new if fe not in held]
    home = {fe: d1d_look for fe in old} | {fe: out for fe in new}
    pools = {fe: json.loads((home[fe] / fe / "pool.json").read_text(encoding="utf-8"))["panels"] for fe in home}
    ans = {j: look1d.labels(d1d_look, j) | look1d.labels(out, j) for j in TEACHERS + COUNTERS}
    ident, qfeat = {}, {}
    for fe in home:
        ident[fe], qfeat[fe] = questions(see_dir, fe)
        for p in pools[fe]:
            p["question"] = qfeat[fe].get(p["title"], {})
            p["more"] = more(p)
    look1d.QUESTION[:] = sorted({k for fe in home for f in qfeat[fe].values() for k in f})
    mine = {fe: [p for p in pools[fe] if "round 1" not in p["in"]] for fe in home}
    choosers = {fe: Chooser([ident[fe].get(p["title"], ("?", p["title"])) for p in mine[fe]]) for fe in old + new}

    def scored(taught: list[str], extra: bool = True) -> dict:
        """Every taught database scored by a look that never saw it, and the
        ten by one fitted on all the taught."""
        data = {}
        for fe in taught + held:
            train = [p for f in taught if f != fe for p in pools[f]]
            model = fit_look(train, ans, extra)
            y = np.array([np.nan if (t := target(p, ans)) is None else t for p in mine[fe]])
            data[fe] = {"oof": logit(model.predict(np.array([vector(p, extra) for p in mine[fe]], dtype=float))),
                        "score": np.array([p["score"] for p in mine[fe]]), "chooser": choosers[fe], "y": y}
        return data

    def fives(data: dict, taught: list[str]) -> tuple[dict, dict]:
        """Each database's five, the penalties and weight tuned on the taught
        others."""
        per, params = {}, {}
        for fe in old + held:
            lam, w = tune([d for d in taught if d != fe], data, lam1d, 2.0)
            params[fe] = {"penalties": lam, "weight": w}
            x = data[fe]
            per[fe] = [mine[fe][i]["id"] for i in x["chooser"].choose(combined(x["score"], x["oof"], w), lam)]
        return per, params

    data = scored(taught)

    # round 1d's look as it was taught, by Sonnet over round 1d's 28 with its
    # own features, out of fold there and fitted on all for the ten
    q_now = list(look1d.QUESTION)
    look1d.QUESTION[:] = sorted({k for fe in old for f in qfeat[fe].values() for k in f if k != "sibling_key"})
    sonnet_1d = {}
    for fe in old + held:
        train = [p for f in old if f != fe for p in pools[f]]
        model = look1d.fit_look(train, ans["sonnet"])
        lg = logit(model.predict(np.array([look1d.vector(p) for p in mine[fe]], dtype=float)))
        five = data[fe]["chooser"].choose(data[fe]["score"] + 2.0 * lg, lam1d)
        sonnet_1d[fe] = [mine[fe][i]["id"] for i in five]
    look1d.QUESTION[:] = q_now

    systems: dict[str, dict[str, list]] = {
        "round 1, B1": {fe: [p["id"] for p in pools[fe] if "round 1" in p["in"]] for fe in home},
        "round 1c": {fe: [p["id"] for p in pools[fe] if "round 1c" in p["in"]] for fe in home},
        "round 1d, learned choice": {fe: [p["id"] for p in pools[fe] if "round 1d, choice" in p["in"]] for fe in home},
        "round 1d, the look taught by Sonnet": sonnet_1d,
        "round 1e, round 1d's penalties and weight": {
            fe: [mine[fe][i]["id"] for i in data[fe]["chooser"].choose(data[fe]["score"] + 2.0 * data[fe]["oof"], lam1d)]
            for fe in old + held},
    }
    systems["round 1e"], chosen = fives(data, taught)
    # what each change brings: the same, taught on round 1d's 28 only, and
    # without the result's new numbers and the sibling key
    systems["round 1e, taught on round 1d's 28 only"], _ = fives(scored(old), old)
    systems["round 1e, without the new numbers"], _ = fives(scored(taught, extra=False), taught)

    def kept(judge, five):
        return sum(1 for pid in five if ans[judge].get(pid, {}).get("keep"))

    result: dict = {"taught on": len(taught), "held out": len(held)}
    rng = np.random.default_rng(SEED)
    for part, dbs in (("round 1d's 28, each left out", old), ("the ten held out", held)):
        covered = {j: [fe for fe in dbs if all(pid in ans[j] for s in systems.values() for pid in s.get(fe, []))]
                   for j in COUNTERS}
        block = {"databases counted": {j: len(c) for j, c in covered.items()}}
        for name, per in systems.items():
            block[name] = {j: round(float(np.mean([kept(j, per[fe]) for fe in covered[j]])), 2)
                           for j in COUNTERS if covered[j]}
        # the most a five from the same candidates could keep
        def ceiling(j, fe):
            got = {ident[fe].get(p["title"]) for p in mine[fe] if ans[j].get(p["id"], {}).get("keep")}
            return min(5, len(got))
        block["ceiling"] = {j: round(float(np.mean([ceiling(j, fe) for fe in covered[j]])), 2) for j in COUNTERS if covered[j]}
        pairs = [(a, b) for a in TEACHERS for b in COUNTERS] + [(a, b) for a in COUNTERS for b in COUNTERS if a < b]
        for a, b in pairs:
            both = [p["id"] for fe in dbs for p in mine[fe] if p["id"] in ans[a] and p["id"] in ans[b]]
            if both:
                block[f"{a} and {b} agree on keep"] = round(float(np.mean(
                    [bool(ans[a][i]["keep"]) == bool(ans[b][i]["keep"]) for i in both])), 3)
        for other in ("round 1, B1", "round 1c", "round 1d, the look taught by Sonnet"):
            for j in COUNTERS:
                if not covered[j] or (j == "sonnet" and other.endswith("Sonnet")):
                    continue
                d = np.array([kept(j, systems["round 1e"][fe]) - kept(j, systems[other][fe]) for fe in covered[j]], float)
                boots = [rng.choice(d, len(d)).mean() for _ in range(10000)]
                block[f"round 1e − {other}, {j}"] = {"mean": round(float(d.mean()), 2),
                                                      "95%": [round(float(np.percentile(boots, q)), 2) for q in (2.5, 97.5)]}
        result[part] = block
    result["penalties and weight, per database left out"] = {fe.rsplit("-", 1)[0]: c for fe, c in chosen.items()}
    result["fives"] = {name: {fe: per[fe] for fe in old + held if fe in per} for name, per in systems.items()}
    (out / "evaluate.json").write_text(json.dumps(result, indent=1, ensure_ascii=False), encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k not in ("fives", "penalties and weight, per database left out")},
                     indent=1, ensure_ascii=False))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "pool":
        pool(Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]), Path(sys.argv[5]), sys.argv[6:])
    elif cmd == "pack":
        pack(Path(sys.argv[2]), sys.argv[3:])
    elif cmd == "evaluate":
        evaluate(Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]), Path(sys.argv[5]))
