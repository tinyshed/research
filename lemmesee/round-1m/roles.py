"""The five as a screen of roles rather than the five best: the person who
runs a product opens a screen with its pulse, its money, its health, its
speed and its people, and a score for each panel alone fills the five with
the most common good kind instead — on a living database, three timelines of
one activity. Each question is given a role by what it reads; the five takes
the best of each role there is, in that order, then the best of the rest,
with one pulse at most and round 1g's rule of each axis once throughout.

    python -I roles.py count <out>

counts on the 93 public databases, by the first judges and the product's,
the look's five (round 1g's out-of-fold scores, each axis once), round 1k's
seat for money, and the five by roles, differences from the look's five
bootstrapped over families. The chooser is used by stories.py's bench and
on the owner's databases too.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT / "round-1c"))
sys.path.insert(0, str(ROOT / "round-1k"))
import local  # noqa: E402
import rank  # noqa: E402

ORDER = ("pulse", "money", "health", "speed", "people")
HEALTH = {"status", "state", "stage", "result", "outcome", "resolution", "approved", "approval", "cancel",
          "cancelled", "canceled", "fail", "failed", "failure", "error", "success", "succeeded", "rejected",
          "refund", "refunded", "returned", "paid", "complete", "completed", "active"}


def words(slot: list[str]) -> set[str]:
    return {w.split(":", 1)[-1].lstrip("@") for w in slot if not w.startswith("#")}


def role(ident, slots: dict, money_words: set[str]) -> str:
    """What a question is for on the screen."""
    agg, cols = ident[1]
    time, by = ident[2], ident[3]
    if agg == "late" or agg == "avg" and len(cols) == 2:
        return "speed"
    if agg in ("sum", "avg") and words(slots.get("measure", [])) & money_words:
        return "money"
    if agg == "count_if" or by is not None and words(slots.get("by", [])) & HEALTH:
        return "health"
    if agg == "distinct":
        return "people"
    if agg == "count" and time is not None and by is None:
        return "pulse"
    return "other"


def timed_role(ident, slots: dict, money_words: set[str]) -> str | None:
    """The role a question plays on a weekly screen when it can show a
    change: drawn over a time. Health is a status split over a time, not
    rows by status since the start, where a fortnight's failures vanish."""
    r = role(ident, slots, money_words)
    if ident[2] is None:
        return None
    if r == "health" or r in ("pulse", "speed"):
        return r
    if r in ("money", "people") and ident[3] is None:
        return r
    return None


def screen(schema, qs, ids, scores, slots, money_words: set[str], k: int = 5) -> list[int]:
    """A weekly screen from all of a schema's questions by a ranker's
    scores: the best over a time of each role there is, however far down the
    ranker placed it, then the best of the rest under rank.choose's limits,
    with one pulse. Two roles may share a table's time: a pulse and its
    split by status are different panels."""
    s = np.asarray(scores, dtype=float)
    order = [int(i) for i in np.argsort(-s, kind="stable")]
    timed = [timed_role(i, sl, money_words) for i, sl in zip(ids, slots)]
    plain = [role(i, sl, money_words) for i, sl in zip(ids, slots)]
    # people are parents counted through a key, not a row's own identifier counted again
    for i, q in enumerate(qs):
        if timed[i] == "people" and not (q.measure.cols and schema.col(q.measure.cols[0]).fk is not None):
            timed[i] = None
    chosen, tables, bys, asked, times = [], {}, set(), set(), set()

    def take(i):
        q = qs[i]
        chosen.append(i)
        asked.add(ids[i])
        tables[q.base] = tables.get(q.base, 0) + 1
        if ids[i][3]:
            bys.add(ids[i][3])
        if q.time:
            times.add((q.time.t, q.time.c))

    def fits(i, strict_time: bool) -> bool:
        q = qs[i]
        if ids[i] in asked or tables.get(q.base, 0) >= 2 or ids[i][3] and ids[i][3] in bys:
            return False
        return not (strict_time and q.time and (q.time.t, q.time.c) in times)

    for r in ORDER:
        i = next((i for i in order if timed[i] == r and fits(i, False)), None)
        if i is not None:
            take(i)
    for i in order:
        if len(chosen) >= k:
            break
        if fits(i, True) and not (plain[i] == "pulse" and any(plain[j] == "pulse" for j in chosen)):
            take(i)
    return sorted(chosen[:k], key=lambda i: -s[i])


def conflicts(chooser, weights: np.ndarray, chosen: list[int], i: int) -> bool:
    """round 1g's chooser's own test of two panels sharing too much."""
    return any(chooser.dup[i, j] or chooser.F[i, j] @ weights >= 9 for j in chosen)


def choose(chooser, scores, roles: list[str], axis, shared, k: int = 5, depth: int = 30,
           one_pulse: bool = True) -> list[int]:
    """shared: the names of round 1g's shared axes, in the order chooser.F holds them."""
    weights = np.array([axis[n] for n in shared])
    s = np.asarray(scores, dtype=float)
    order = [int(i) for i in np.argsort(-s, kind="stable")]
    reach = order[:depth]
    chosen: list[int] = []
    for r in ORDER:
        for i in reach:
            if roles[i] == r and i not in chosen and not conflicts(chooser, weights, chosen, i):
                chosen.append(i)
                break
        if len(chosen) == k:
            break
    for i in order:
        if len(chosen) == k:
            break
        if i in chosen or conflicts(chooser, weights, chosen, i):
            continue
        if one_pulse and roles[i] == "pulse" and any(roles[j] == "pulse" for j in chosen):
            continue
        chosen.append(i)
    return sorted(chosen, key=lambda i: -s[i])


def money_words() -> set[str]:
    lex = json.loads((ROOT / "corpus" / "round-1g" / "lexicon" / "lexicon.json").read_text(encoding="utf-8"))["words"]
    return set(rank.MONEYWORD) | {w for w, v in lex.items() if v[1] >= 0.7}


def count(out: Path) -> None:
    sys.argv = [sys.argv[0]]
    dg = local.module("ro_diagnose", ROOT / "round-1g" / "diagnose.py")
    sets = local.module("ro_sets", ROOT / "round-1g" / "sets.py")
    joint = local.module("ro_joint", ROOT / "round-1j" / "joint.py")
    pc = local.module("ro_product_count", ROOT / "round-1k" / "product_count.py")
    shared = pc.chooser_shared = dg.look1e.choose.SHARED
    corpus = ROOT / "corpus"
    dirs = [corpus / f"round-1{x}" / "look" for x in "def"]
    D = dg.load_all(dirs, corpus / "see")
    judges = {"first judges": ("sonnet", "gemini"), "product judges": ("pmsonnet", "pmgemini")}
    ans = {j: {} for js in judges.values() for j in js}
    for d in dirs:
        for j in ans:
            ans[j] |= dg.look1d.labels(d, j)
    scores = json.loads((corpus / "round-1g" / "scores.json").read_text(encoding="utf-8"))[pc.CONFIG]
    mw = money_words()
    systems = ("look, each axis once", "+ money seat", "roles", "roles, pulses free", "roles, from the ten")
    kept = {(s, j): {} for s in systems for j in ans}
    role_mix = {s: {} for s in systems}
    for fe in D["fes"]:
        mine, ids = D["mine"][fe], D["ids"][fe]
        roles = [role(i, p["slots"], mw) for p, i in zip(mine, ids)]
        kinds = [pc.kind(i, p["slots"], mw) for p, i in zip(mine, ids)]
        chooser = dg.look1e.Chooser(ids)
        s = scores[fe]
        fives = {"look, each axis once": chooser.choose(s, sets.AXIS_ONCE),
                 "+ money seat": pc.choose(chooser, s, ids, kinds, sets.AXIS_ONCE, {"money seat"}),
                 "roles": choose(chooser, s, roles, sets.AXIS_ONCE, shared),
                 "roles, pulses free": choose(chooser, s, roles, sets.AXIS_ONCE, shared, one_pulse=False),
                 "roles, from the ten": choose(chooser, s, roles, sets.AXIS_ONCE, shared, depth=10)}
        for name, five in fives.items():
            for i in five:
                role_mix[name][roles[i]] = role_mix[name].get(roles[i], 0) + 1
            for j in ans:
                if five and all(mine[i]["id"] in ans[j] for i in five):
                    kept[(name, j)][fe] = float(sum(bool(ans[j][mine[i]["id"]]["keep"]) for i in five))
    result = {}
    for name in systems:
        row = {}
        for label, js in judges.items():
            both = {fe: np.mean([kept[(name, j)][fe] for j in js]) for fe in kept[(name, js[0])]
                    if all(fe in kept[(name, j)] for j in js)}
            row[label] = {"kept of five": round(float(np.mean(list(both.values()))), 3), "databases": len(both)}
            if name != systems[0]:
                ref = {fe: np.mean([kept[(systems[0], j)][fe] for j in js]) for fe in both
                       if all(fe in kept[(systems[0], j)] for j in js)}
                row[label]["minus the look"] = joint.paired_interval({fe: both[fe] - ref[fe] for fe in ref}, D["family"])
        n = sum(role_mix[name].values())
        row["roles of its panels"] = {r: round(v / n, 3) for r, v in sorted(role_mix[name].items(), key=lambda x: -x[1])}
        result[name] = row
    out.mkdir(parents=True, exist_ok=True)
    (out / "roles.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(f"{'five':22} {'first judges':>14} {'minus the look':>24} {'product judges':>16} {'minus the look':>24}")
    for name, row in result.items():
        cells = []
        for label in judges:
            r = row[label]
            d = r.get("minus the look")
            cells.append(f"{r['kept of five']:>14.2f} " + (f"{d['mean']:+.2f} [{d['95%'][0]:+.2f}, {d['95%'][1]:+.2f}]".rjust(24) if d else " " * 24))
        print(f"{name:22} " + " ".join(cells))
        print(" " * 23 + ", ".join(f"{r} {v:.0%}" for r, v in row["roles of its panels"].items()))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, *a = sys.argv[1:]
    if cmd == "count":
        count(Path(a[0]))
