"""The product judges against the first ones: how their keep differs panel by
panel and by kind of panel, and how every five counts under each, with the
three changes to the choice the owner's databases pointed to.

    python -I product_count.py <out>

Fives: round 1's B1, round 1c's ranker, the look (round 1g's small look,
out of fold) with each axis once, and that look's five changed three ways,
each set before counting: at most two timelines of rows; one measure over
one time; a seat for money, the best panel summing or averaging a money-like
column taking the weakest place when none is in the five. Counted by both
judges of each kind averaged, differences bootstrapped over families. No
model is fitted.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import local  # noqa: E402

CONFIG = "small, round 1e numbers, per panel"
OLD, PM = ("sonnet", "gemini"), ("pmsonnet", "pmgemini")


def kind(ident, slots, money_words) -> dict:
    agg, cols = ident[1]
    words = [w.split(":", 1)[-1] for w in slots.get("measure", []) if not w.startswith("#")]
    return {"timeline of rows": agg == "count" and ident[2] is not None and ident[3] is None,
            "breakdown of rows": agg == "count" and ident[3] is not None,
            "sum or average": agg in ("sum", "avg"),
            "money": agg in ("sum", "avg") and any(w in money_words for w in words),
            "over time": ident[2] is not None}


def choose(chooser, scores, ids, kinds, axis, fixes: set[str]) -> list[int]:
    s = np.asarray(scores, dtype=float).copy()
    five = chooser.choose(s, axis)
    if not fixes:
        return five
    # greedy again, the look's order, with the fixes as hard limits
    order = list(np.argsort(-s, kind="stable"))
    chosen: list[int] = []
    for i in order:
        if len(chosen) == 5:
            break
        trial = chosen + [i]
        # the axis rule's own judgement of the trial, as the chooser makes it
        if any(chooser.dup[i, j] for j in chosen):
            continue
        if any(chooser.F[i, j] @ np.array([axis[n] for n in chooser_shared]) >= 9 for j in chosen):
            continue
        if "two timelines" in fixes and kinds[i]["timeline of rows"] and \
                sum(kinds[j]["timeline of rows"] for j in chosen) >= 2:
            continue
        if "one time a measure" in fixes and ids[i][2] is not None and any(
                ids[j][1] == ids[i][1] and ids[j][2] is not None and ids[j][2] != ids[i][2] for j in chosen):
            continue
        chosen = trial
    if "money seat" in fixes and not any(kinds[j]["money"] for j in chosen):
        money = [i for i in order if kinds[i]["money"] and i not in chosen]
        if money and chosen:
            chosen[-1] = money[0]
    return chosen


def main(out: Path) -> None:
    global chooser_shared
    sys.argv = [sys.argv[0]]
    dg = local.module("pm_diagnose", ROOT / "round-1g" / "diagnose.py")
    sets = local.module("pm_sets", ROOT / "round-1g" / "sets.py")
    joint = local.module("pm_joint", ROOT / "round-1j" / "joint.py")
    import rank
    chooser_shared = dg.look1e.choose.SHARED
    corpus = ROOT / "corpus"
    dirs = [corpus / f"round-1{x}" / "look" for x in "def"]
    D = dg.load_all(dirs, corpus / "see")
    ans = {j: {} for j in OLD + PM}
    for d in dirs:
        for j in ans:
            ans[j] |= dg.look1d.labels(d, j)
    scores = json.loads((corpus / "round-1g" / "scores.json").read_text(encoding="utf-8"))[CONFIG]
    money_words = set(rank.MONEYWORD)
    lex = json.loads((corpus / "round-1g" / "lexicon" / "lexicon.json").read_text(encoding="utf-8"))["words"]
    money_words |= {w for w, v in lex.items() if v[1] >= 0.7}

    # panel by panel
    pairs = {"sonnet": "pmsonnet", "gemini": "pmgemini"}
    panel = {}
    by_kind: dict = {}
    for fe in D["fes"]:
        for p, ident in zip(D["mine"][fe], D["ids"][fe]):
            k = kind(ident, p["slots"], money_words)
            for old, pm in pairs.items():
                if p["id"] in ans[old] and p["id"] in ans[pm]:
                    a, b = bool(ans[old][p["id"]]["keep"]), bool(ans[pm][p["id"]]["keep"])
                    panel.setdefault(old, []).append((a, b))
                    for name, on in k.items():
                        if on:
                            by_kind.setdefault(name, {}).setdefault(old, []).append((a, b))
    result = {"panel by panel": {old: {"panels": len(v), "agree": round(float(np.mean([a == b for a, b in v])), 3),
                                       "kept, first": round(float(np.mean([a for a, _ in v])), 3),
                                       "kept, product": round(float(np.mean([b for _, b in v])), 3)}
                                 for old, v in panel.items()},
              "by kind": {name: {old: {"panels": len(v), "kept, first": round(float(np.mean([a for a, _ in v])), 3),
                                       "kept, product": round(float(np.mean([b for _, b in v])), 3)}
                                 for old, v in d.items()} for name, d in by_kind.items()}}

    # fives
    systems = ["round 1", "round 1c", "look, each axis once", "+ two timelines", "+ one time a measure",
               "+ money seat", "+ all three"]
    fix_sets = {"+ two timelines": {"two timelines"}, "+ one time a measure": {"one time a measure"},
                "+ money seat": {"money seat"}, "+ all three": {"two timelines", "one time a measure", "money seat"}}
    kept = {(s, j): {} for s in systems for j in OLD + PM}
    for fe in D["fes"]:
        mine, ids = D["mine"][fe], D["ids"][fe]
        kinds = [kind(i, p["slots"], money_words) for p, i in zip(mine, ids)]
        chooser = dg.look1e.Chooser(ids)
        r1 = [p for p in D["pools"][fe] if "round 1" in p["in"]]
        fives = {"round 1": r1, "round 1c": [p for p in mine if "round 1c" in p["in"]][:5],
                 "look, each axis once": [mine[i] for i in chooser.choose(scores[fe], sets.AXIS_ONCE)]}
        for name, fx in fix_sets.items():
            fives[name] = [mine[i] for i in choose(chooser, scores[fe], ids, kinds, sets.AXIS_ONCE, fx)]
        for s, five in fives.items():
            for j in OLD + PM:
                if five and all(p["id"] in ans[j] for p in five):
                    kept[(s, j)][fe] = float(sum(bool(ans[j][p["id"]]["keep"]) for p in five))
    family = D["family"]
    table = {}
    for s in systems:
        row = {}
        for label, judges in (("first judges", OLD), ("product judges", PM)):
            both = {fe: np.mean([kept[(s, j)][fe] for j in judges]) for fe in kept[(s, judges[0])]
                    if all(fe in kept[(s, j)] for j in judges)}
            row[label] = {"kept of five": round(float(np.mean(list(both.values()))), 3) if both else None,
                          "databases": len(both)}
            if s != "look, each axis once" and both:
                ref = {fe: np.mean([kept[("look, each axis once", j)][fe] for j in judges])
                       for fe in both if all(fe in kept[("look, each axis once", j)] for j in judges)}
                diff = {fe: both[fe] - ref[fe] for fe in ref}
                row[label]["minus the look"] = joint.paired_interval(diff, family)
        table[s] = row
    result["fives"] = table
    out.mkdir(parents=True, exist_ok=True)
    (out / "product-judges.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(json.dumps(result["panel by panel"], indent=1))
    print(f"{'kind':20} " + " ".join(f"{o + ' first/product':>24}" for o in pairs))
    for name, d in result["by kind"].items():
        print(f"{name:20} " + " ".join(f"{d[o]['kept, first']:>11.2f} / {d[o]['kept, product']:<10.2f}" if o in d else " " * 24 for o in pairs))
    print(f"\n{'five':22} {'first judges':>14} {'diff':>22} {'product judges':>16} {'diff':>22}")
    for s, row in table.items():
        cells = []
        for label in ("first judges", "product judges"):
            r = row[label]
            d = r.get("minus the look")
            cells.append(f"{r['kept of five']:>14.2f} " + (f"{d['mean']:+.2f} [{d['95%'][0]:+.2f}, {d['95%'][1]:+.2f}]" if d else " " * 21))
        print(f"{s:22} " + " ".join(cells))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
