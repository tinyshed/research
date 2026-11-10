"""Round 1v: what the second stage reads over a whole schema.

Round 1b's graph stage reads nineteen sums of the first stage's scores, a
pair at a time; round 1t added six that read a pair against its schema's
scores (stack.py) and six over the columns sharing a lead word (style.py).
Here a column is read with the others of its schema that are alike, four
ways, the same seven figures each way:

- the columns of the same name (letters and digits, lower case) in other
  tables: one name in 58 tables is one question, not 58;
- the columns named by an actor (`created_by`, `criado_por`, `owner`:
  round 1t's lexicon of roles), whatever their names;
- the columns sharing the lead word, and those sharing the last word.

For a pair from column c to target t, over the other columns of c's group:
how many they are over the schema's tables; the mean and the largest of
their scores into t; the share of them whose best target is t; the group's
summed score into t less its best other target's, over its size; the share
of them whose best score is confident; and the mean of their best scores.
A column alone in its group reads −1 for each.

Beside them: how many tables the schema has and how many targets the column
was scored against (both as logarithms), the column's best over the sum of
its scores, that sum, how many different column names are confident into
the target, version 10's five fields of a pair (ten.py: home, the target
carries the name, the view, the core, the longest table inside), and the
schema's valley: the cut of its columns' best scores that parts those that
look like keys from the rest, wherever the scores of the whole schema lie,
with the pair's and its column's distance from it.
"""

import math
import sys
from collections import defaultdict
from functools import lru_cache
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1t"))

import cues  # noqa: E402,F401  (round 1t's: the earlier rounds on the path)

import numpy as np  # noqa: E402

import graph  # noqa: E402
import roles  # noqa: E402
import stack  # noqa: E402
import style  # noqa: E402
import ten  # noqa: E402
from train import norm  # noqa: E402

GROUPS = ("same name", "actor", "lead word", "last word")
FIGURES = ("others over tables", "others into the target, mean", "others into the target, best",
           "others whose best is the target", "lead over the group's other target", "others confident",
           "others' mean best")
OWN = ["tables, log", "targets of the column, log", "best over the sum", "sum of the column's scores",
       "names confident into the target", "home", "the target carries the name", "view", "core",
       "the longest table inside", "valley: the pair above it", "valley: the column's best above it",
       "valley: how far apart its sides", "valley: columns above it", "valley: its width"]
WIDE = [f"{g}: {f}" for g in GROUPS for f in FIGURES] + OWN
CONFIDENT = graph.CONFIDENT


@lru_cache(maxsize=400_000)
def group_keys(name: str) -> tuple:
    lead, last = style.lead_and_last(name, False)
    return norm(name), ("@" if roles.carries(name, "actor") else ""), lead, last


def logit(p: float) -> float:
    return math.log((p + 1e-4) / (1 - p + 1e-4))


def valley(bests: list) -> tuple:
    """Where a schema's columns part into those that look like keys and the
    rest: the cut of their best scores, as logits, that leaves the two sides
    furthest apart for their sizes (Otsu's). Returns the cut, the share of
    the scores' variance the cut explains, the share of columns above it and
    the gap it sits in; a schema of under four columns has none."""
    n = len(bests)
    if n < 4:
        return 0.0, 0.0, 0.0, 0.0
    v = np.sort(np.array([logit(b) for b in bests]))
    var = float(v.var())
    if var <= 1e-9:
        return float(v[0]), 0.0, 0.0, 0.0
    cs = np.cumsum(v)
    k = np.arange(1, n)
    m0 = cs[:-1] / k
    m1 = (cs[-1] - cs[:-1]) / (n - k)
    between = k * (n - k) * (m0 - m1) ** 2 / (n * n)
    j = int(np.argmax(between))
    return float((v[j] + v[j + 1]) / 2), float(between[j] / var), float((n - j - 1) / n), float(v[j + 1] - v[j])


def wide_features(meta, scores, sizes: dict) -> np.ndarray:
    out = np.zeros((len(meta), len(WIDE)), dtype=np.float32)
    start = 0
    SID, TABLE, COLUMN, TARGET = graph.SID, graph.TABLE, graph.COLUMN, graph.TARGET
    nf = len(FIGURES)
    while start < len(meta):
        end = start
        sid = meta[start][SID]
        while end < len(meta) and meta[end][SID] == sid:
            end += 1
        tables = max(sizes.get(sid, 1), 1)
        rows_of = defaultdict(list)
        for i in range(start, end):
            rows_of[(meta[i][TABLE], meta[i][COLUMN])].append(i)
        best, best_target, total = {}, {}, {}
        for k, rows in rows_of.items():
            b, bt, t = -1.0, None, 0.0
            for i in rows:
                sc = float(scores[i])
                t += sc
                if sc > b:
                    b, bt = sc, meta[i][TARGET]
            best[k], best_target[k], total[k] = b, bt, t
        gk = {k: group_keys(k[1]) for k in rows_of}
        # a group's members, their summed and two best scores into each target, and whose best it is
        stats = []
        for gi in range(len(GROUPS)):
            members = defaultdict(int)
            confident = defaultdict(int)
            best_sum = defaultdict(float)
            best_is = defaultdict(int)
            for k in rows_of:
                g = gk[k][gi]
                if not g:
                    continue
                members[g] += 1
                confident[g] += best[k] >= CONFIDENT
                best_sum[g] += best[k]
                best_is[(g, best_target[k])] += 1
            into = defaultdict(float)
            top = defaultdict(lambda: [0.0, 0.0])
            for i in range(start, end):
                g = gk[(meta[i][TABLE], meta[i][COLUMN])][gi]
                if not g or members[g] < 2:
                    continue
                sc = float(scores[i])
                key = (g, meta[i][TARGET])
                into[key] += sc
                t2 = top[key]
                if sc > t2[0]:
                    t2[1], t2[0] = t2[0], sc
                elif sc > t2[1]:
                    t2[1] = sc
            # a group's two best targets by summed score
            lead = defaultdict(lambda: [(0.0, None), (0.0, None)])
            for (g, t), v in into.items():
                l2 = lead[g]
                if v > l2[0][0]:
                    l2[1], l2[0] = l2[0], (v, t)
                elif v > l2[1][0]:
                    l2[1] = (v, t)
            stats.append((members, confident, best_sum, best_is, into, top, lead))
        cut, apart, above, width = valley(list(best.values()))
        names_into = defaultdict(set)
        for k, b in best.items():
            if b >= CONFIDENT:
                names_into[best_target[k]].add(gk[k][0])
        for i in range(start, end):
            m = meta[i]
            k = (m[TABLE], m[COLUMN])
            t = m[TARGET]
            sc = float(scores[i])
            row = out[i]
            for gi, (members, confident, best_sum, best_is, into, top, lead) in enumerate(stats):
                g = gk[k][gi]
                n = members[g] - 1 if g else 0
                if n <= 0:
                    row[gi * nf:(gi + 1) * nf] = -1.0
                    continue
                key = (g, t)
                is_best = best_target[k] == t
                t2 = top[key]
                other = lead[g][1][0] if lead[g][0][1] == t else lead[g][0][0]
                row[gi * nf:(gi + 1) * nf] = (
                    n / tables, (into[key] - sc) / n, t2[1] if sc >= t2[0] else t2[0],
                    (best_is[key] - is_best) / n, (into[key] - other) / (n + 1),
                    (confident[g] - (best[k] >= CONFIDENT)) / n, (best_sum[g] - best[k]) / n)
            b = best[k]
            o = len(GROUPS) * nf
            row[o:] = (math.log2(tables) / 8, math.log2(len(rows_of[k])) / 8, b / total[k] if total[k] > 0 else 0.0,
                       min(total[k], 5.0), len(names_into[t] - {gk[k][0]}) / tables,
                       m[ten.HOME], m[ten.CARRIED], m[ten.VIEW], m[ten.CORE], m[ten.INSIDE],
                       logit(sc) - cut, logit(b) - cut, apart, above, width)
        start = end
    return out


KINDS = {
    "graph": lambda meta, scores, sizes: graph.graph_features(meta, scores),
    "relative": lambda meta, scores, sizes: stack.graph_features(meta, scores),
    "style": lambda meta, scores, sizes: style.graph_features(meta, scores),
    "wide": lambda meta, scores, sizes: np.hstack([stack.graph_features(meta, scores),
                                                   wide_features(meta, scores, sizes)]),
}
NAMES = {
    "graph": list(graph.GRAPH_FEATURES),
    "relative": list(graph.GRAPH_FEATURES) + list(stack.EXTRA),
    "style": list(graph.GRAPH_FEATURES) + list(stack.EXTRA) + list(style.STYLE),
    "wide": list(graph.GRAPH_FEATURES) + list(stack.EXTRA) + WIDE,
}


def job(args):
    """One chunk of a set: its pairs' features of a kind, from compact rows."""
    import first  # noqa: E402  (here: first imports this module's neighbours)
    kind, schemas, rec, scores = args
    meta = first.expand(rec, schemas)
    sizes = {s["id"]: len(s["tables"]) for s in schemas}
    return KINDS[kind](meta, scores, sizes)


def features(pool, kind: str, schemas: list, rec: dict, scores: np.ndarray, chunks: int = 32) -> np.ndarray:
    """A set's features, its schemas shared out over the pool; rows in the set's order."""
    n = len(rec["schema"])
    if n == 0:
        return np.zeros((0, len(NAMES[kind])), dtype=np.float32)
    # a schema's pairs lie together: cut where the schema changes
    change = np.flatnonzero(np.diff(rec["schema"])) + 1
    bounds = np.concatenate([[0], change, [n]])
    target = n / chunks
    cuts, last = [0], 0
    for b in bounds[1:]:
        if b - last >= target or b == n:
            cuts.append(int(b))
            last = b
    jobs = []
    for a, b in zip(cuts, cuts[1:]):
        ids = np.unique(rec["schema"][a:b])
        remap = {int(si): j for j, si in enumerate(ids)}
        part = {k: v[a:b] for k, v in rec.items() if isinstance(v, np.ndarray) and len(v) == n}
        part["schema"] = np.array([remap[int(si)] for si in part["schema"]], dtype=np.int32)
        jobs.append((kind, [schemas[int(si)] for si in ids], part, scores[a:b]))
    return np.vstack(list(pool.map(job, jobs)))
