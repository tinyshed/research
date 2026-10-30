"""Each set of teachers without the look: their own mean chance of keep as the
score, the five it chooses with each axis once, counted by the first judges
(Sonnet and Gemini) and by the product judges (pmsonnet, pmgemini), and its
AUC within a database against each pair over the panels the pair agrees on.
Beside them, the product judges' own answers as the score, the most a five
can keep by them under the same rule. No model is fitted.

    python -I product_teachers.py <out>

Says whether a gap after product_look.py is the teachers' or the look's.
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

TEACHERS = {"jev": ("jev",), "deepseek and glm": ("deepseek", "glm"), "jev and luna": ("jevfast", "lunap"),
            "product teachers": ("pmjev", "pmlunap"), "all four": ("jevfast", "lunap", "pmjev", "pmlunap"),
            "pmjev": ("pmjev",), "pmlunap": ("pmlunap",), "product judges themselves": ("pmsonnet", "pmgemini")}
JUDGES = {"first judges": ("sonnet", "gemini"), "product judges": ("pmsonnet", "pmgemini")}


def auc(s: np.ndarray, y: np.ndarray) -> float:
    pos, neg = s[y > 0.5], s[y < 0.5]
    if not len(pos) or not len(neg):
        return math.nan
    return float(np.mean((pos[:, None] > neg[None, :]) + 0.5 * (pos[:, None] == neg[None, :])))


def main(out: Path) -> None:
    sys.argv = [sys.argv[0]]
    dg = local.module("pt_diagnose", ROOT / "round-1g" / "diagnose.py")
    sets = local.module("pt_sets", ROOT / "round-1g" / "sets.py")
    frozen = ROOT / "corpus" / "round-1j"
    meta = json.loads((frozen / "data.json").read_text(encoding="utf-8"))
    A = dict(np.load(frozen / "data.npz", allow_pickle=False))
    dirs = [ROOT / "corpus" / f"round-1{x}" / "look" for x in "def"]
    names = {j for t in TEACHERS.values() for j in t} | {j for js in JUDGES.values() for j in js}
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

    dbs = A["db"]
    db_ix = {db: np.flatnonzero(dbs == db) for db in sorted(set(dbs.tolist()))}
    judged = {jn: np.column_stack([mean_of((j,), "keep") for j in js]).mean(axis=1) for jn, js in JUDGES.items()}
    result = {}
    for tn, ts in TEACHERS.items():
        s = mean_of(ts, "p_keep")
        row = {"keep": round(float(np.nanmean(s > 0.5)), 3)}
        for jn, y in judged.items():
            fives, aucs = [], []
            for db, ix in db_ix.items():
                if not (np.isfinite(y[ix]).all() and np.isfinite(s[ix]).all()):
                    continue
                chooser = dg.look1e.Chooser([meta["panels"][int(i)]["identity"] for i in ix])
                fives.append(float(y[ix][chooser.choose(s[ix], sets.AXIS_ONCE)].sum()))
                aucs.append(auc(s[ix], y[ix]))
            row[jn] = {"kept of five": round(float(np.mean(fives)), 3), "auc": round(float(np.nanmean(aucs)), 3),
                       "databases": len(fives)}
        result[tn] = row
    out.mkdir(parents=True, exist_ok=True)
    (out / "product-teachers.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(f"{'score':26} {'keep':>5}  " + "  ".join(f"{jn:>14} {'auc':>5}" for jn in JUDGES) + "  databases")
    for tn, r in result.items():
        print(f"{tn:26} {r['keep']:5.2f}  " + "  ".join(f"{r[jn]['kept of five']:14.2f} {r[jn]['auc']:5.3f}"
                                                     for jn in JUDGES) + f"  {r['product judges']['databases']}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
