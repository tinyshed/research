"""The look taught the product's way: round 1j's baseline look, its folds and
seeds, taught by each set of teachers in turn — Jev, DeepSeek and GLM as
round 1j; Jev (fast) and Luna as round 1k; the same two asked the product's
question (pmjev, pmlunap); and all four together — and each five counted by
the first judges (Sonnet and Gemini) and by the product judges (pmsonnet,
pmgemini), each pair averaged, differences bootstrapped over families.

    python -I product_look.py <out>

A development probe on databases already looked at; the judges that count
never teach.
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

TARGETS = {"first teachers": None, "jev and luna": ("jevfast", "lunap"), "product teachers": ("pmjev", "pmlunap"),
            "all four": ("jevfast", "lunap", "pmjev", "pmlunap")}
JUDGES = {"first judges": ("sonnet", "gemini"), "product judges": ("pmsonnet", "pmgemini")}


def main(out: Path) -> None:
    sys.argv = [sys.argv[0]]
    joint = local.module("pl_joint", ROOT / "round-1j" / "joint.py")
    dg = local.module("pl_diagnose", ROOT / "round-1g" / "diagnose.py")
    sets = local.module("pl_sets", ROOT / "round-1g" / "sets.py")
    frozen = ROOT / "corpus" / "round-1j"
    meta = json.loads((frozen / "data.json").read_text(encoding="utf-8"))
    A = dict(np.load(frozen / "data.npz", allow_pickle=False))
    dirs = [ROOT / "corpus" / f"round-1{x}" / "look" for x in "def"]
    names = {j for t in TARGETS.values() if t for j in t} | {j for js in JUDGES.values() for j in js}
    ans = {j: {} for j in names}
    for d in dirs:
        for j in ans:
            ans[j] |= dg.look1d.labels(d, j)
    ids = [p["id"] for p in meta["panels"]]

    def mean_of(js, field):
        out_ = []
        for pid in ids:
            v = [float(ans[j][pid][field]) if field == "p_keep" and "p_keep" in ans[j].get(pid, {})
                 else float(bool(ans[j][pid]["keep"])) for j in js if pid in ans[j]]
            out_.append(float(np.mean(v)) if v else math.nan)
        return np.asarray(out_)

    ys = {name: (A["y"] if js is None else mean_of(js, "p_keep")) for name, js in TARGETS.items()}
    judged = {name: np.column_stack([mean_of((j,), "keep") for j in js]) for name, js in JUDGES.items()}
    scores = {name: np.full((joint.DRAWS, len(ids)), np.nan) for name in TARGETS}
    for draw, fold, train, test in joint.folds(A["family"].tolist(), joint.DRAWS):
        for name, y in ys.items():
            pred, _ = joint.fit_predict("baseline", A["base"], y, train, test, joint.SEED + 100 * draw + fold)
            scores[name][draw, test] = pred
    dbs = A["db"]
    db_ix = {db: np.flatnonzero(dbs == db) for db in sorted(set(dbs.tolist()))}
    family = {db: str(A["family"][ix[0]]) for db, ix in db_ix.items()}
    kept = {}
    for t in TARGETS:
        for policy, pen in (("plain", dg.NO_PENALTY), ("axes", sets.AXIS_ONCE)):
            for jn, J in judged.items():
                per = {}
                for db, ix in db_ix.items():
                    y = J[ix].mean(axis=1)
                    if not np.isfinite(y).all():
                        continue
                    chooser = dg.look1e.Chooser([meta["panels"][int(i)]["identity"] for i in ix])
                    per[db] = float(np.mean([y[chooser.choose(s, pen)].sum() for s in scores[t][:, ix]]))
                kept[(t, policy, jn)] = per
    result = {}
    for (t, policy, jn), per in kept.items():
        row = {"kept of five": round(float(np.mean(list(per.values()))), 3), "databases": len(per)}
        if t != "first teachers":
            ref = kept[("first teachers", policy, jn)]
            row["minus first teachers"] = joint.paired_interval({db: v - ref[db] for db, v in per.items() if db in ref}, family)
        result[f"{t} / {policy} / {jn}"] = row
    out.mkdir(parents=True, exist_ok=True)
    (out / "product-look.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(f"{'look taught by':18} {'policy':6} {'judges':15} {'kept':>5}   difference [95%]")
    for k, r in result.items():
        t, policy, jn = k.split(" / ")
        d = r.get("minus first teachers")
        print(f"{t:18} {policy:6} {jn:15} {r['kept of five']:>5.2f}   " +
              (f"{d['mean']:+.2f} [{d['95%'][0]:+.2f}, {d['95%'][1]:+.2f}]" if d else ""))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
