"""The two readings of round 1k's ideas as numbers the look can learn
from, in round 1j's panel order: each place's head word and the largest of
its modifiers (head.py), and the spread, the share at zero or below and the
magnitude of a summed or averaged column, apart for sums and averages
(extensive.py). Fits nothing.

    python -I features.py <out>     # after head.py's and extensive.py's runs
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE))
import local  # noqa: E402
from head import headed  # noqa: E402

PLACES = ("measure", "by")


def main(out: Path) -> None:
    sys.argv = [sys.argv[0]]
    dg = local.module("features_diagnose", ROOT / "round-1g" / "diagnose.py")
    corpus = ROOT / "corpus"
    meta = json.loads((corpus / "round-1j" / "data.json").read_text(encoding="utf-8"))
    D = dg.load_all([corpus / f"round-1{x}" / "look" for x in "def"], corpus / "see")
    lex = json.loads((corpus / "round-1g" / "lexicon" / "lexicon.json").read_text(encoding="utf-8"))
    by_id = {p["id"]: (p, ident) for fe in D["fes"] for p, ident in zip(D["mine"][fe], D["ids"][fe])}
    spread = {(r["db"], r["title"]): r for r in
              json.loads((out / "extensive-panels.json").read_text(encoding="utf-8"))}
    head_names = [f"{w}_{s}_{k}" for s in PLACES for w in ("h", "m") for k in dg.PROPS]
    spread_names = [f"{a}_{k}" for a in ("sum", "avg") for k in ("spread", "nonpositive", "magnitude")]
    H, S = [], []
    for row in meta["panels"]:
        p, ident = by_id[row["id"]]
        h = headed(p["slots"], lex, PLACES, dg.PROPS)
        H.append([h[n] for n in head_names])
        r = spread.get((row["db"], p["title"]))
        v = [math.nan] * 6
        if r is not None and ident[1][0] in ("sum", "avg"):
            at = 0 if ident[1][0] == "sum" else 3
            v[at:at + 3] = [r["spread"], r["nonpositive"], r["magnitude"]]
        S.append(v)
    H, S = np.asarray(H, dtype=np.float32), np.asarray(S, dtype=np.float32)
    np.savez_compressed(out / "features.npz", head=H, spread=S)
    (out / "features.json").write_text(json.dumps({"head": head_names, "spread": spread_names,
                                                    "panels": len(H), "with a spread": int(np.isfinite(S).any(1).sum())},
                                                   indent=1), encoding="utf-8")
    print(f"{len(H)} panels; head {H.shape[1]} numbers, spread {S.shape[1]}, {int(np.isfinite(S).any(1).sum())} panels with a spread")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
