"""The second click: what a person's first few answers are worth, and
whether carrying each across the schema is worth more than obeying it.

    python -I second_click.py <out>

On every database of rounds 1d-1f, the look's out-of-fold five (round 1g's
small look, its scores saved) is shown to a person, here the teachers' mean
keep: a panel it would not keep (below one half) is dropped, the worst
first, up to three clicks, and the five chosen again. Dropped alone, the
panel is replaced by the next. Carried, every candidate sharing the dropped
panel's measure, breakdown or time loses as much as CARRY says, set before
anything was counted. The judges that never teach count every five. No
model is fitted.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE.parent))
import local  # noqa: E402

CONFIG = "small, round 1e numbers, per panel"
CLICKS = (1, 2, 3)
# what a dropped panel takes from a candidate sharing a part of it, in the
# look's probability; fixed before counting
CARRY = {"measure": 0.30, "by_same": 0.30, "by_other": 0.15, "time_same": 0.15, "time_other": 0.05,
         "base": 0.0, "shape": 0.0}


def simulate(chooser, shared, ids, s0, human, clicks: int, carry: bool) -> tuple[list[int], int]:
    s = np.asarray(s0, dtype=float).copy()
    used = 0
    five = chooser.choose(s, NO_PENALTY)
    for _ in range(clicks):
        bad = [i for i in five if human[i] is not None and human[i] < 0.5]
        if not bad:
            break
        worst = min(bad, key=lambda i: human[i])
        used += 1
        s[worst] = -np.inf
        if carry:
            for c in range(len(s)):
                if np.isfinite(s[c]):
                    sh = shared(ids[worst], ids[c])
                    s[c] -= sum(CARRY[k] * v for k, v in sh.items())
        five = chooser.choose(s, NO_PENALTY)
    return five, used


def main(out: Path) -> None:
    global NO_PENALTY
    sys.argv = [sys.argv[0]]
    dg = local.module("click_diagnose", ROOT / "round-1g" / "diagnose.py")
    NO_PENALTY = dg.NO_PENALTY
    corpus = ROOT / "corpus"
    dirs = [corpus / "round-1d" / "look", corpus / "round-1e" / "look", corpus / "round-1f" / "look"]
    D = dg.load_all(dirs, corpus / "see")
    scores = json.loads((corpus / "round-1g" / "scores.json").read_text(encoding="utf-8"))[CONFIG]
    shared = dg.look1e.choose.shared
    variants = [("no click", 0, False)] + [(f"{k} {'carried' if c else 'dropped'}", k, c) for k in CLICKS for c in (False, True)]
    kept = {v[0]: {j: {} for j in dg.COUNTERS} for v in variants}
    used = {v[0]: [] for v in variants}
    for fe in D["fes"]:
        mine, ids = D["mine"][fe], D["ids"][fe]
        human = [dg.target(p, D["ans"]) for p in mine]
        chooser = dg.look1e.Chooser(ids)
        for name, k, c in variants:
            five, n = simulate(chooser, shared, ids, scores[fe], human, k, c)
            used[name].append(n)
            for j in dg.COUNTERS:
                if fe in D["covered"][j]:
                    kept[name][j][fe] = float(sum(bool(D["ans"][j][mine[i]["id"]]["keep"]) for i in five))
    joint = local.module("click_joint", ROOT / "round-1j" / "joint.py")
    family = D["family"]
    result = {"config": CONFIG, "carry": CARRY, "databases": len(D["fes"])}
    for name, _, _ in variants:
        row = {"clicks used, mean": round(float(np.mean(used[name])), 2)}
        for j in dg.COUNTERS:
            row[f"{j}, kept of five"] = round(float(np.mean(list(kept[name][j].values()))), 3)
            if name != "no click":
                diff = {fe: v - kept["no click"][j][fe] for fe, v in kept[name][j].items()}
                row[f"{j}, minus no click"] = joint.paired_interval(diff, family)
                if name.endswith("carried"):
                    other = name.replace("carried", "dropped")
                    diff = {fe: v - kept[other][j][fe] for fe, v in kept[name][j].items()}
                    row[f"{j}, carried minus dropped"] = joint.paired_interval(diff, family)
        result[name] = row
    out.mkdir(parents=True, exist_ok=True)
    (out / "second-click.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(f"{'':16} {'clicks':>6} " + " ".join(f"{j:>8}" for j in dg.COUNTERS))
    for name, _, _ in variants:
        r = result[name]
        print(f"{name:16} {r['clicks used, mean']:>6} " + " ".join(f"{r[f'{j}, kept of five']:>8.2f}" for j in dg.COUNTERS))
    for name, _, _ in variants[1:]:
        r = result[name]
        line = "  ".join(f"{j} {r[f'{j}, minus no click']['mean']:+.2f} [{r[f'{j}, minus no click']['95%'][0]:+.2f}, "
                         f"{r[f'{j}, minus no click']['95%'][1]:+.2f}]" for j in dg.COUNTERS)
        print(f"{name:16} vs none: {line}")
        if name.endswith("carried"):
            line = "  ".join(f"{j} {r[f'{j}, carried minus dropped']['mean']:+.2f} [{r[f'{j}, carried minus dropped']['95%'][0]:+.2f}, "
                             f"{r[f'{j}, carried minus dropped']['95%'][1]:+.2f}]" for j in dg.COUNTERS)
            print(f"{'':16} carried vs dropped: {line}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
