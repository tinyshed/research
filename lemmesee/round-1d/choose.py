"""Round 1d's choice of five, learned from the teachers' own sets: round
1c's rules of choice — never two panels over one time column or one
breakdown — replaced by penalties for what two panels share, each weighed
by how the teachers' fives share it. A count and a sum of money over one
time stand side by side in a teacher's five; round 1c's rules let only one.

    python -I choose.py oof <round 1c learn dir> <out>
    python -I choose.py fit <round 1c learn dir> <out>

oof writes <out>/oof.pkl, each teacher schema's scores from a ranker that
never saw it; fit tunes the penalties over folds, measures them against
round 1c's rules, and writes <out>/penalties.json.
"""

from __future__ import annotations

import itertools
import json
import pickle
import random
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parent.parent / "round-1c"))
import learn  # noqa: E402

FOLDS = 5
SEED = 20261028

# What two panels may share, as their identities show it
SHARED = ("base", "measure", "time_same", "time_other", "by_same", "by_other", "shape")
START = {"base": 0.5, "measure": 1.0, "time_same": 9.0, "time_other": 9.0, "by_same": 9.0, "by_other": 9.0, "shape": 0.0}
GRID = (0.0, 0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 9.0)


def shared(a: tuple, b: tuple) -> dict[str, float]:
    """(base, measure, time, breakdown) twice: what they have in common. A
    measure is the same when its aggregate and columns are."""
    same_measure = a[1] == b[1]
    return {
        "base": float(a[0] == b[0]),
        "measure": float(a[0] == b[0] and same_measure),
        "time_same": float(a[2] is not None and a[2] == b[2] and same_measure),
        "time_other": float(a[2] is not None and a[2] == b[2] and not same_measure),
        "by_same": float(a[3] is not None and a[3] == b[3] and same_measure),
        "by_other": float(a[3] is not None and a[3] == b[3] and not same_measure),
        "shape": float((a[2] is None) == (b[2] is None) and (a[3] is None) == (b[3] is None)),
    }


def greedy(ids: list, scores, lam: dict, k: int = 5, pool: int = 60) -> list[int]:
    """The k that score best once what each shares with those chosen before
    it is taken off; a question asked twice in two shapes never."""
    order = list(np.argsort(-np.asarray(scores), kind="stable")[:pool])
    chosen: list[int] = []
    while len(chosen) < k and order:
        best, best_at = None, -1e18
        for i in order:
            if any(ids[i] == ids[j] for j in chosen):
                continue
            at = scores[i] - sum(lam[n] * v for j in chosen for n, v in shared(ids[i], ids[j]).items())
            if at > best_at:
                best, best_at = i, at
        if best is None:
            break
        chosen.append(int(best))
        order.remove(best)
    return chosen


class Pool:
    """A schema's best questions by score, and what each two of them share,
    computed once so a choice under any penalties is a few array sums."""

    def __init__(self, ids: list, scores, size: int = 60):
        self.idx = np.argsort(-np.asarray(scores), kind="stable")[:size]
        self.s = np.asarray(scores)[self.idx]
        n = len(self.idx)
        self.F = np.zeros((n, n, len(SHARED)), dtype=np.float32)
        self.dup = np.zeros((n, n), dtype=bool)
        for a in range(n):
            for b in range(a + 1, n):
                ia, ib = ids[self.idx[a]], ids[self.idx[b]]
                v = shared(ia, ib)
                self.F[a, b] = self.F[b, a] = [v[k] for k in SHARED]
                self.dup[a, b] = self.dup[b, a] = ia == ib

    def choose(self, lam: dict, k: int = 5) -> list[int]:
        w = np.array([lam[n] for n in SHARED], dtype=np.float32)
        P = self.F @ w
        at = self.s.copy()
        free = np.ones(len(self.s), dtype=bool)
        chosen = []
        for _ in range(min(k, len(self.s))):
            cand = np.where(free, at, -np.inf)
            j = int(np.argmax(cand))
            if not np.isfinite(cand[j]):
                break
            chosen.append(j)
            free[j] = False
            free &= ~self.dup[j]
            at = at - P[:, j]
        return [int(self.idx[j]) for j in chosen]


def agreement(rows, scores_of, chooser) -> dict[str, float]:
    res = defaultdict(list)
    for r in rows:
        chosen = {r["ids"][i] for i in chooser(r, scores_of(r))}
        for t, d in r["teachers"].items():
            ten = {g for g in d["asked"] if g is not None}
            res[f"{t} precision"].append(len(chosen & ten) / 5)
            res[f"{t} recall"].append(sum(1 for g in d["asked"][:5] if g in chosen) / 5)
    return {k: float(np.mean(v)) for k, v in sorted(res.items())}


def objective(m: dict) -> float:
    return float(np.mean(list(m.values())))


def folds(rows):
    repos = sorted({r["repo"] for r in rows})
    random.Random(SEED).shuffle(repos)
    fold_of = {repo: i % FOLDS for i, repo in enumerate(repos)}
    return [r for r in rows], fold_of


def oof(learn_dir: Path, out: Path) -> None:
    rows = pickle.load((learn_dir / "data.pkl").open("rb"))
    names = learn.all_names(rows)
    _, fold_of = folds(rows)
    scores = {}
    for k in range(FOLDS):
        train = [r for r in rows if fold_of[r["repo"]] != k]
        model = learn.fit_one(train, names, 600)
        for r in rows:
            if fold_of[r["repo"]] == k:
                X, _, _ = learn.matrix_free(r, names, None)
                scores[r["key"]] = model.predict(X)
    out.mkdir(parents=True, exist_ok=True)
    with (out / "oof.pkl").open("wb") as f:
        pickle.dump(scores, f)
    print(f"{len(scores)} schemas scored out of fold")


def tune(rows, scores, pools) -> dict:
    """Coordinate ascent over the grid, from round 1c's rules, on the
    agreement with the teachers."""
    lam = dict(START)
    pick = lambda trial: (lambda r, s: pools[r["key"]].choose(trial))
    best = objective(agreement(rows, lambda r: scores[r["key"]], pick(lam)))
    for _ in range(3):
        moved = False
        for n in SHARED:
            for v in GRID:
                trial = dict(lam, **{n: v})
                o = objective(agreement(rows, lambda r: scores[r["key"]], pick(trial)))
                if o > best + 1e-9:
                    best, lam, moved = o, trial, True
        if not moved:
            break
    return lam


def fit(learn_dir: Path, out: Path) -> None:
    rows = pickle.load((learn_dir / "data.pkl").open("rb"))
    scores = pickle.load((out / "oof.pkl").open("rb"))
    _, fold_of = folds(rows)
    pools = {r["key"]: Pool(r["ids"], scores[r["key"]]) for r in rows}
    rules, learned, chosen_lams = defaultdict(list), defaultdict(list), []
    for k in range(FOLDS):
        train = [r for r in rows if fold_of[r["repo"]] != k]
        test = [r for r in rows if fold_of[r["repo"]] == k]
        lam = tune(train, scores, pools)
        chosen_lams.append(lam)
        for name, m in (("rules", agreement(test, lambda r: scores[r["key"]], lambda r, s: learn.five(r, s))),
                        ("learned", agreement(test, lambda r: scores[r["key"]], lambda r, s: pools[r["key"]].choose(lam)))):
            target = rules if name == "rules" else learned
            for key, v in m.items():
                target[key].append((v, len(test)))
    mean = lambda d: {k: round(sum(v * n for v, n in xs) / sum(n for _, n in xs), 3) for k, xs in d.items()}
    final = tune(rows, scores, pools)
    result = {"round 1c's rules of choice": mean(rules), "learned penalties, out of fold": mean(learned),
              "penalties per fold": chosen_lams, "penalties": final}
    (out / "penalties.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "oof":
        oof(Path(sys.argv[2]), Path(sys.argv[3]))
    elif cmd == "fit":
        fit(Path(sys.argv[2]), Path(sys.argv[3]))
