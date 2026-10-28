"""What people declared of a column's name, read against what the judges
keep: from semantic.py's fields, how often a column so named was declared
a sum, an average, a count of distinct values, a dimension or a time, by
its whole name and by its head word; then, fitting nothing, whether that
prior orders the sums and averages of rounds 1d-1f as the teachers and the
judges do, and what it says that the look did not.

    python -I semantic_prior.py <semantic dir> <out>
"""

from __future__ import annotations

import json
import math
import sys
from collections import Counter, defaultdict
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE.parent))
import local  # noqa: E402

CONFIG = "small, round 1e numbers, per panel"
ROLES = {("measure", "sum"): "sum", ("measure", "sum_distinct"): "sum", ("measure", "average"): "avg",
         ("measure", "average_distinct"): "avg", ("measure", "avg"): "avg", ("measure", "count_distinct"): "distinct",
         ("measure", "median"): "avg", ("measure", "max"): "max", ("measure", "min"): "max",
         ("dimension", "string"): "dimension", ("dimension", "yesno"): "dimension", ("dimension", "tier"): "dimension",
         ("dimension", "number"): "number", ("dimension", None): "dimension", ("dimension_group", "time"): "time",
         ("dimension_group", "duration"): "number"}


def prior(semantic: Path, tokens):
    """Counts of each role by a column's whole name and by its head word,
    one vote a repository and name, so one project's many views count once."""
    whole, head = defaultdict(Counter), defaultdict(Counter)
    seen = set()
    for line in (semantic / "fields.jsonl").read_text(encoding="utf-8").splitlines():
        r = json.loads(line)
        role = ROLES.get((r["role"], r["type"]))
        if role is None:
            continue
        for col in (r["columns"] or [r["name"]])[:1]:
            tk = tokens(col)
            if not tk:
                continue
            key = (r["repo"], "".join(tk), role)
            if key in seen:
                continue
            seen.add(key)
            whole["".join(tk)][role] += 1
            head[tk[-1]][role] += 1
    return whole, head


def share(counts: Counter, role: str, alpha: float = 1.0) -> float:
    n = sum(counts.values())
    return (counts[role] + alpha * 0.2) / (n + alpha) if n else math.nan


def main(semantic: Path, out: Path) -> None:
    from scipy.stats import spearmanr
    sys.argv = [sys.argv[0]]
    dg = local.module("prior_diagnose", ROOT / "round-1g" / "diagnose.py")
    aq = sys.modules["aq"]
    whole, head = prior(semantic, aq.tokens)
    corpus = ROOT / "corpus"
    D = dg.load_all([corpus / f"round-1{x}" / "look" for x in "def"], corpus / "see")
    scores = json.loads((corpus / "round-1g" / "scores.json").read_text(encoding="utf-8"))[CONFIG]
    rows = []
    for fe in D["fes"]:
        schema = aq.load(json.loads((corpus / "see" / fe / "snapshot.json").read_text(encoding="utf-8")), True)
        for i, (p, ident) in enumerate(zip(D["mine"][fe], D["ids"][fe])):
            agg, cols = ident[1]
            if agg not in ("sum", "avg") or len(cols) != 1:
                continue
            t, c = next(iter(cols))
            tk = schema.tables[t].columns[c].tk
            if not tk:
                continue
            w, h = whole.get("".join(tk)), head.get(tk[-1])
            rows.append({"agg": agg, "title": p["title"], "by whole name": share(w, agg) if w else math.nan,
                         "by head word": share(h, agg) if h else math.nan,
                         "declared, whole name": sum(w.values()) if w else 0, "declared, head word": sum(h.values()) if h else 0,
                         "teachers": dg.target(p, D["ans"]), "look": float(scores[fe][i]),
                         "judges": [float(bool(D["ans"][j][p["id"]]["keep"])) if p["id"] in D["ans"][j] else math.nan
                                    for j in dg.COUNTERS]})
    result = {"names declared": len(whole), "head words declared": len(head)}
    for agg in ("sum", "avg"):
        sel = [r for r in rows if r["agg"] == agg]
        res = {"panels": len(sel)}
        for way in ("by whole name", "by head word"):
            x = np.array([r[way] for r in sel], dtype=float)
            line = {"known": int(np.isfinite(x).sum())}
            for name, y in [("teachers", [r["teachers"] for r in sel]), ("look", [r["look"] for r in sel])] + \
                    [(j, [r["judges"][k] for r in sel]) for k, j in enumerate(dg.COUNTERS)] + \
                    [(f"{j} less the look", [r["judges"][k] - r["look"] for r in sel]) for k, j in enumerate(dg.COUNTERS)]:
                y = np.array([v if v is not None else math.nan for v in y], dtype=float)
                ok = np.isfinite(x) & np.isfinite(y)
                line[name] = round(float(spearmanr(x[ok], y[ok])[0]), 3) if ok.sum() > 10 else None
            res[way] = line
        result[agg] = res
    out.mkdir(parents=True, exist_ok=True)
    (out / "semantic-prior.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    (out / "semantic-prior-panels.json").write_text(json.dumps(rows, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(result, indent=1))
    for key in ("CreditCardNumber", "person.weight", "City.Population", "DEP_DELAY", "tonnage", "PurchasePrice", "fine"):
        r = next((r for r in rows if key in r["title"]), None)
        if r:
            print(f"  {r['title'][:60]:60} whole {r['by whole name']:.2f} ({r['declared, whole name']}) "
                  f"head {r['by head word']:.2f} ({r['declared, head word']}) teachers {r['teachers']:.2f}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]))
