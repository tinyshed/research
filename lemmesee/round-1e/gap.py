"""Round 1e's first question: where round 1d's five loses what its pool
holds. On round 1d's 28 databases, each judge's keep is set beside the
ranker's score, the look's probability out of fold, and what the five's
penalties allow.

    python -I gap.py <round 1d look dir> <corpus/see> <round 1d dir> <fixture-engine>...

Writes <look dir>/gap.json and prints it.
"""

from __future__ import annotations

import json
import sys
from collections import Counter, defaultdict
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE.parent / "round-1d"))
sys.path.insert(0, str(HERE.parent / "round-1c"))
import choose  # noqa: E402
import look  # noqa: E402

JUDGES = ("opus", "sonnet", "gemini")
W = 2.0


def auc(y, s) -> float | None:
    y, s = np.asarray(y, dtype=int), np.asarray(s, dtype=float)
    pos, neg = s[y == 1], s[y == 0]
    if not len(pos) or not len(neg):
        return None
    return float(((pos[:, None] > neg[None, :]).sum() + 0.5 * (pos[:, None] == neg[None, :]).sum()) / (len(pos) * len(neg)))


def kind(ident: tuple) -> str:
    base, (agg, cols), time, by = ident[0], ident[1], ident[2], ident[3]
    shape = "series" if time is not None and by is None else "split" if time is not None else "breakdown" if by is not None else "single"
    return f"{shape}/{agg}"


def main(out: Path, see_dir: Path, d1d: Path, fixtures: list[str]) -> None:
    lam = json.loads((d1d / "penalties.json").read_text(encoding="utf-8"))["penalties"]
    pools = {fe: json.loads((out / fe / "pool.json").read_text(encoding="utf-8"))["panels"] for fe in fixtures}
    ans = {j: look.labels(out, j) for j in JUDGES}
    teacher = ans["sonnet"]
    ident, qfeat = {}, {}
    for fe in fixtures:
        ident[fe], qfeat[fe] = look.identities(see_dir, fe)
        for p in pools[fe]:
            p["question"] = qfeat[fe].get(p["title"], {})
    look.QUESTION[:] = sorted({k for fe in fixtures for f in qfeat[fe].values() for k in f})

    res: dict = {"databases": len(fixtures), "penalties": lam}
    per_db = {}
    aucs = defaultdict(list)
    kinds = {j: Counter() for j in JUDGES}
    kinds_kept = {j: Counter() for j in JUDGES}
    chosen_kinds, chosen_kinds_kept = Counter(), Counter()
    missed_rank, dropped = [], []
    ceil = defaultdict(list)
    for fe in fixtures:
        train = [p for f in fixtures if f != fe for p in pools[f]]
        model = look.fit_look(train, teacher)
        mine = [p for p in pools[fe] if "round 1" not in p["in"]]
        pk = np.clip(model.predict(np.array([look.vector(p) for p in mine], dtype=float)), 1e-3, 1 - 1e-3)
        logit = np.log(pk / (1 - pk))
        score = np.array([p["score"] for p in mine])
        combined = score + W * logit
        ids = [ident[fe].get(p["title"], ("?", p["title"])) for p in mine]
        five = choose.Pool(ids, combined, size=len(mine)).choose(lam)
        order = list(np.argsort(-combined, kind="stable"))
        row = {}
        for j in JUDGES:
            have = [k for k, p in enumerate(mine) if p["id"] in ans[j]]
            if len(have) < len(mine):
                continue
            y = np.array([bool(ans[j][p["id"]]["keep"]) for p in mine], dtype=int)
            for name, s in (("score", score), ("look", logit), ("combined", combined)):
                a = auc(y, s)
                if a is not None:
                    aucs[f"{j}, {name}"].append(a)
            distinct = {}
            for k, i in enumerate(ids):
                distinct.setdefault(i, k)
            kept_distinct = sum(1 for i, k in distinct.items() if y[k])
            oracle = choose.Pool(ids, y * 10.0 + 1e-3 * combined, size=len(mine)).choose(lam)
            row[j] = {"pool": len(distinct), "kept in pool": kept_distinct, "ceiling": min(5, kept_distinct),
                      "ceiling under the penalties": int(sum(y[oracle])), "five": int(sum(y[five]))}
            ceil[f"{j}, ceiling"].append(min(5, kept_distinct))
            ceil[f"{j}, ceiling under the penalties"].append(int(sum(y[oracle])))
            ceil[f"{j}, five"].append(int(sum(y[five])))
            for k, p in enumerate(mine):
                kinds[j][kind(ids[k])] += 1
                kinds_kept[j][kind(ids[k])] += int(y[k])
            # a second panel of one base and one measure in the five, over
            # another time or breakdown, against the rest
            for at, k in enumerate(five):
                again = any(ids[k][0] == ids[m][0] and ids[k][1] == ids[m][1] for m in five[:at])
                ceil[f"{j}, kept: a base and measure again"].append(int(y[k])) if again else ceil[f"{j}, kept: the rest"].append(int(y[k]))
                ceil[f"{j}, share of the five: a base and measure again"].append(int(again))
            if j == "opus":
                for k in five:
                    chosen_kinds[kind(ids[k])] += 1
                    chosen_kinds_kept[kind(ids[k])] += int(y[k])
                    if not y[k]:
                        a = ans[j][mine[k]["id"]]
                        r = mine[k]["result"]
                        dropped.append({"db": fe, "title": mine[k]["title"], "meaning": a.get("meaning"), "wrong": a.get("wrong"),
                                        "rows": r["rows"], "top_share": r.get("top_share"), "blank_labels": r.get("blank_labels"),
                                        "coverage": r.get("coverage"), "p": round(float(pk[k]), 2), "rank": order.index(k) + 1})
                # what Opus keeps and the five left: where it stood
                for k, p in enumerate(mine):
                    if y[k] and k not in five:
                        missed_rank.append(order.index(k) + 1)
        per_db[fe] = row
    res["mean"] = {k: round(float(np.mean(v)), 2) for k, v in sorted(ceil.items())}
    res["auc, mean over databases"] = {k: round(float(np.mean(v)), 3) for k, v in sorted(aucs.items())}
    res["keep rate by kind"] = {kd: {j: f"{kinds_kept[j][kd]}/{kinds[j][kd]}" for j in JUDGES}
                                for kd in sorted(kinds["opus"], key=lambda x: -kinds["opus"][x])}
    res["the five's kinds, kept by Opus"] = {kd: f"{chosen_kinds_kept[kd]}/{n}" for kd, n in chosen_kinds.most_common()}
    res["kept by Opus but not chosen: rank by the combined score"] = {
        "n": len(missed_rank), "median": float(np.median(missed_rank)) if missed_rank else None,
        "within 10": sum(1 for r in missed_rank if r <= 10)}
    res["chosen and dropped by Opus"] = dropped
    res["per database"] = per_db
    (out / "gap.json").write_text(json.dumps(res, indent=1, ensure_ascii=False), encoding="utf-8")
    print(json.dumps({k: v for k, v in res.items() if k not in ("chosen and dropped by Opus", "per database")}, indent=1, ensure_ascii=False))
    for d in dropped:
        print(f"  {d['db'][:14]:14} r{d['rank']:>2} p{d['p']:.2f} rows {d['rows']:>4.0f} {d['meaning'] or '':11} {d['title'][:70]} — {d['wrong']}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4:])
