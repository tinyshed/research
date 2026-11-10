"""Round 1v: the scorings side by side, F1 over every declared key by set.

    python -I table.py [<model> ...]     corpus/round-1v/scored/<model>.json, and the frozen models read beside them
"""
import json
import sys
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "corpus" / "round-1v" / "scored"
SHORT = {"schemapile validation": "sp-val", "schemapile validation, renamed": "sp-val-rn", "schemapile test": "sp-test",
         "schemapile test, renamed": "sp-test-rn", "spider": "spider", "bird": "bird", "ctu": "ctu",
         "livesqlbench (development)": "lsb", "github 2025": "gh-2025", "github 2025, rest": "gh-rest",
         "github style": "gh-style", "github fresh": "gh-fresh"}


def main(names):
    scored = {n: json.load(open(OUT / f"{n}.json", encoding="utf-8")) for n in names}
    sets = [s for s in SHORT if any(s in r["sets"] for r in scored.values())]
    rows = {}
    first = scored[names[0]]["sets"]
    for fm in ("1s", "seven-bounded", "nine-mended"):
        if any(fm in first.get(s, {}) for s in sets):
            rows[fm] = {s: first[s][fm] for s in sets if fm in first.get(s, {})}
    for n, r in scored.items():
        rows[n] = {s: r["sets"][s]["measures"] for s in sets if s in r["sets"]}
    for what, pick in (("F1 over every declared key", lambda m: m["f1"]), ("macro F1", lambda m: m["macro f1"]),
                       ("F1, schemas of 40 tables or more", None), ("recall", lambda m: m["recall"]),
                       ("precision", lambda m: m["precision"])):
        print(what)
        print(f"{'':34}" + "".join(f"{SHORT[s]:>11}" for s in sets))
        for n, by in rows.items():
            line = f"{n:34}"
            for s in sets:
                m = by.get(s)
                if m is None:
                    line += f"{'':>11}"
                elif pick is None:
                    tp = fp = fn = 0
                    big = [v for k, v in m["by tables"].items() if k in ("40-99", "100+")]
                    if not big:
                        line += f"{'':>11}"
                        continue
                    # pooled over the two bins from their precision, recall and keys
                    for v in big:
                        t = v["recall"] * v["keys"]
                        tp += t
                        fn += v["keys"] - t
                        fp += (t / v["precision"] - t) if v["precision"] else 0
                    p, r = tp / max(tp + fp, 1e-9), tp / max(tp + fn, 1e-9)
                    line += f"{2 * p * r / max(p + r, 1e-9):>11.3f}"
                else:
                    line += f"{pick(m):>11.3f}"
            print(line)
        print()


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(sys.argv[1:])
