"""Round 1y: the report's tables printed as Markdown from the round's output.

    python -I tables.py rule | set <set> | judge <set>
"""

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import numpy as np  # noqa: E402

import pu  # noqa: E402

OUT = pu.OUT
BINS = ["0-4", "5-9", "10-19", "20-39", "40-99", "100+"]


def ci(v) -> str:
    return f"[{v[0]:+.3f}, {v[1]:+.3f}]"


def ci0(v) -> str:
    return f"[{v[0]:.3f}, {v[1]:.3f}]"


def rule() -> None:
    dev = json.load(open(OUT / "development.json", encoding="utf-8"))["variants"]
    choice = json.load(open(OUT / "choice.json", encoding="utf-8"))
    spec = json.load(open(OUT / "variants.json", encoding="utf-8"))
    print("| | thresholds by size | P / R / F1 declared, GitHub pooled | minus the frozen model | keys drawn (40 tables or more) | "
          "weighted P / R / F1 | the other seed, weighted F1 | mean | SchemaPile's complete test |")
    print("|---|---|---|---|---|---|---|---|---|")
    for name, r in sorted(choice["table"].items(), key=lambda kv: -kv[1]["mean"]):
        first, rest = name.split("--", 1)
        e = dev[name]
        g = e["github development sets, pooled"]
        w = g["weighted"]
        th = spec[name]["thresholds by size"]
        ths = f"{th[0]:.3f}" if len(set(th)) == 1 else " ".join(f"{x:.3f}" for x in th)
        print(f"| `{name}`{'' if r.get('a candidate', True) or name == 'eleven--wide' else ' (beside)'} | {ths} | "
              f"{g['precision']:.3f} / {g['recall']:.3f} / {g['f1']:.3f} | "
              + (f"{g['minus base, f1']:+.3f} {ci(g['minus base, f1_95'])}" if "minus base, f1" in g else "")
              + f" | {g['drawn']} ({g['drawn, 40 tables or more']}) | {w['precision']:.3f} / {w['recall']:.3f} / {w['f1']:.4f} | "
              f"{r['weighted F1, seed 20261028']:.4f} | {r['mean']:.4f} | {e['schemapile test, complete, f1']:.3f} |")
    print()
    print(json.dumps({k: v for k, v in choice.items() if k != "table"}, indent=1))


def a_set(which: str, names: list) -> None:
    r = json.load(open(OUT / f"heldout-{which}.json", encoding="utf-8"))
    models = r["models"]
    tables = np.array(r["tables"])
    names = names or list(models)
    print(f"**{which}**")
    print("| | P / R / F1 declared [95 %] | minus the frozen model of its seed | 40 tables or more: P / R / F1 | minus the frozen | keys drawn (40 or more) | weighted P / R / F1 |")
    print("|---|---|---|---|---|---|---|")
    for n in names:
        m = models[n]
        b = m["40 tables or more"]
        w = m["weighted"]
        print(f"| `{n}` | {m['precision']:.3f} / {m['recall']:.3f} / {m['f1']:.3f} {ci0(m['f1_95'])} | "
              + (f"{m['minus base, f1']:+.3f} {ci(m['minus base, f1_95'])}" if "minus base, f1" in m else "")
              + f" | {b['precision']:.3f} / {b['recall']:.3f} / {b['f1']:.3f} | "
              + (f"{b['minus base, f1']:+.3f} {ci(b['minus base, f1_95'])}" if "minus base, f1" in b else "")
              + f" | {m['drawn']} ({m['drawn, 40 tables or more']}) | {w['precision']:.3f} / {w['recall']:.3f} / {w['f1']:.3f} |")
    print()
    print("| tables | schemas | keys | " + " | ".join(f"`{n}`" for n in names) + " |")
    print("|---|---|---|" + "---|" * len(names))
    for what in ("f1", "recall", "precision", "drawn"):
        print(f"| *{what}* | | |" + " |" * len(names))
        for b in BINS + ["40+"]:
            lo, hi = (40, 10 ** 6) if b == "40+" else (int(b.split("-")[0].rstrip("+")), 10 ** 6 if b.endswith("+") else int(b.split("-")[1]) + 1)
            sel = (tables >= lo) & (tables < hi)
            if not sel.any():
                continue
            cells = []
            for n in names:
                per = np.array(models[n]["per database"])[sel]
                tp, fp, fn = per.sum(axis=0)
                p = tp / max(tp + fp, 1)
                rc = tp / max(tp + fn, 1)
                f = 2 * p * rc / max(p + rc, 1e-12)
                cells.append(f"{int(tp + fp)}" if what == "drawn" else f"{dict(f1=f, recall=rc, precision=p)[what]:.3f}")
            per0 = np.array(models[names[0]]["per database"])[sel]
            print(f"| {b} | {int(sel.sum())} | {int(per0[:, 0].sum() + per0[:, 2].sum())} | " + " | ".join(cells) + " |")


def judge(which: str) -> None:
    c = json.load(open(OUT / f"judge-{which}" / "count.json", encoding="utf-8"))
    print(json.dumps(c["answers by kind"], indent=1))
    print(json.dumps(c["strata"], indent=1))
    for scope in ("all", "40 tables or more"):
        e = c[scope]
        print(f"\n**{which}, {scope}**: declared {e['declared keys']:.0f}; real, with the pool's judged-real "
              f"{e['real keys, declared and judged-real of the pool']:.0f} [{e['its 95 % interval'][0]:.0f}, {e['its 95 % interval'][1]:.0f}]")
        print("| | keys drawn | undeclared | sampled | called keys | precision, declared | with the judge [95 %] | recall, declared | "
              "over declared and judged-real [95 %] | F1 with the judge |")
        print("|---|---|---|---|---|---|---|---|---|---|")
        for m, r in e["models"].items():
            print(f"| `{m}` | {r['drawn']:.0f} | {r['keys no declaration names']:.0f} | {r['sampled']} | {r['called keys']}, "
                  f"{100 * r['share called keys']:.1f} % | {r['precision, declarations']:.3f} | {r['precision, with the judge']:.3f} "
                  f"{ci0(r['precision, with the judge, 95'])} | {r['recall, declarations']:.3f} | "
                  f"{r['recall, declared and judged-real']:.3f} {ci0(r['recall, declared and judged-real, 95'])} | {r['f1, with the judge']:.3f} |")
        print()
        print("| | minus | precision with the judge | recall over declared and judged-real |")
        print("|---|---|---|---|")
        for m, r in e["models"].items():
            for k, v in r.items():
                if k.startswith("minus "):
                    print(f"| `{m}` | `{k[6:]}` | {v['precision, with the judge']:+.3f} {ci(v['precision, with the judge, 95'])} | "
                          f"{v['recall, declared and judged-real']:+.3f} {ci(v['recall, declared and judged-real, 95'])} |")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "rule":
        rule()
    elif sys.argv[1] == "set":
        a_set(sys.argv[2], sys.argv[3:])
    else:
        judge(sys.argv[2])
