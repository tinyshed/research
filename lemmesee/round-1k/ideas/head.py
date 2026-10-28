"""The head of a name: in English the last word of a compound says what it
is, the words before it what it is of. A card number is a number naming a
card; the number of cards is a count. Round 1g's lexicon took the largest
of each property over a name's words, so credit lent CreditCardNumber its
money. This reads each place's words the other way, the head's properties
apart from the largest of its modifiers', and asks, fitting nothing,
which of the two tells what is summed better.

    python -I head.py <out>
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
import local  # noqa: E402

CONFIG = "small, round 1e numbers, per panel"


def headed(slots: dict, lex: dict, slots_names, props) -> dict:
    """Each place's head word's nine numbers, and the largest of its
    modifiers'; the head is the last word the lexicon knows."""
    words = lex["words"]
    f = {}
    for s in slots_names:
        ws = [w.split(":", 1)[-1] for w in slots.get(s, []) if not w.startswith("#")]
        known = [w for w in ws if w in words]
        head = words[known[-1]] if known else None
        mods = [words[w] for w in known[:-1]]
        for i, k in enumerate(props):
            f[f"h_{s}_{k}"] = head[i] if head else math.nan
            f[f"m_{s}_{k}"] = max(v[i] for v in mods) if mods else math.nan
    return f


def main(out: Path) -> None:
    from scipy.stats import spearmanr
    sys.argv = [sys.argv[0]]
    dg = local.module("head_diagnose", ROOT / "round-1g" / "diagnose.py")
    corpus = ROOT / "corpus"
    dirs = [corpus / "round-1d" / "look", corpus / "round-1e" / "look", corpus / "round-1f" / "look"]
    D = dg.load_all(dirs, corpus / "see")
    lex = json.loads((corpus / "round-1g" / "lexicon" / "lexicon.json").read_text(encoding="utf-8"))
    scores = json.loads((corpus / "round-1g" / "scores.json").read_text(encoding="utf-8"))[CONFIG]
    rows = []
    for fe in D["fes"]:
        for i, (p, ident) in enumerate(zip(D["mine"][fe], D["ids"][fe])):
            if ident[1][0] not in ("sum", "avg"):
                continue
            mx = dg.lexicon(p["slots"], lex)
            hd = headed(p["slots"], lex, dg.SLOTS, dg.PROPS)
            rows.append({"title": p["title"], "agg": ident[1][0], "max": mx, "head": hd, "teachers": dg.target(p, D["ans"]),
                         "look": float(scores[fe][i]),
                         "judges": [float(bool(D["ans"][j][p["id"]]["keep"])) if p["id"] in D["ans"][j] else math.nan
                                    for j in dg.COUNTERS]})
    result = {}
    for agg in ("sum", "avg"):
        sel = [r for r in rows if r["agg"] == agg]
        res = {"panels": len(sel)}
        for prop in ("sum_ok", "id", "money", "count", "cryptic"):
            for way, key in (("largest", f"x_measure_{prop}"), ("head", f"h_measure_{prop}"), ("modifiers", f"m_measure_{prop}")):
                src = "max" if way == "largest" else "head"
                x = np.array([r[src].get(key, math.nan) for r in sel], dtype=float)
                line = {"known": int(np.isfinite(x).sum())}
                for name, y in [("teachers", [r["teachers"] for r in sel])] + \
                        [(j, [r["judges"][k] for r in sel]) for k, j in enumerate(dg.COUNTERS)] + \
                        [(f"{j} less the look", [r["judges"][k] - r["look"] for r in sel]) for k, j in enumerate(dg.COUNTERS)]:
                    y = np.array([v if v is not None else math.nan for v in y], dtype=float)
                    ok = np.isfinite(x) & np.isfinite(y)
                    line[name] = round(float(spearmanr(x[ok], y[ok])[0]), 3) if ok.sum() > 10 else None
                res[f"{prop}, {way}"] = line
        result[agg] = res
    examples = []
    for key in ("CreditCardNumber", "person.weight", "City.Population", "DEP_DELAY", "tonnage", "Price", "fine"):
        r = next((r for r in rows if key in r["title"]), None)
        if r:
            examples.append({"title": r["title"], "largest sum_ok": r["max"]["x_measure_sum_ok"],
                             "head sum_ok": r["head"]["h_measure_sum_ok"], "largest id": r["max"]["x_measure_id"],
                             "head id": r["head"]["h_measure_id"], "teachers": r["teachers"]})
    result["examples"] = examples
    out.mkdir(parents=True, exist_ok=True)
    (out / "head.json").write_text(json.dumps(result, indent=1, ensure_ascii=False), encoding="utf-8")
    for agg in ("sum",):
        print(f"== {agg}: Spearman of the measure's property with ...")
        for k, v in result[agg].items():
            if isinstance(v, dict):
                print(f"  {k:22} known {v['known']:4}  teachers {v['teachers']}  judges {[v[j] for j in dg.COUNTERS]}  "
                      f"less the look {[v[j + ' less the look'] for j in dg.COUNTERS]}")
    for e in examples:
        print(f"  {e['title'][:62]:62} sum_ok largest {e['largest sum_ok']} head {e['head sum_ok']} | id {e['largest id']} / {e['head id']} | teachers {e['teachers']:.2f}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
