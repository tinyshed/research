"""Round 1q: a database's own dialect. Real databases often declare some of
their keys and not others; the declared ones say how this database names a
key. Each database's declared keys are split in half at random: one half is
shown, the other is to be found. The model guesses as round 1p does, every
declaration hidden; the rule then draws, for a column not shown, the target
of a shown key whose column has the same name, replacing the model's guess.

    .venv/Scripts/python -I dialect.py <round 1p out> <cascade out> <out>

writes <out>/dialect.json: over the hidden half of each source's keys,
precision, recall and F1 of the model alone and with the rule, on databases
declaring at least four keys, with three seeds of the split.
"""

import json
import random
import sys
from collections import defaultdict
from pathlib import Path

ROUND_1P = Path(__file__).resolve().parent.parent / "round-1p"
sys.path.insert(0, str(ROUND_1P))
sys.path.insert(0, str(ROUND_1P.parent / "round-1b"))

import gemini  # noqa: E402
from train import norm  # noqa: E402


def main(data: Path, cascade: Path, out: Path) -> None:
    schemas = {s["id"]: s for s in map(json.loads, open(data / "hidden.jsonl", encoding="utf-8"))}
    sure = defaultdict(set)
    for line in open(cascade / "sure.jsonl", encoding="utf-8"):
        sid, t, c, r, _ = json.loads(line)
        sure[sid].add((t, c, r))
    result = {}
    for seed in (1, 2, 3):
        rng = random.Random(seed)
        alone, ruled, truth = {}, {}, {}
        for sid, s in schemas.items():
            fk = sorted(tuple(k) for k in s["fk"])
            if len(fk) < 4:
                continue
            rng.shuffle(fk)
            shown, hidden = set(fk[: len(fk) // 2]), set(fk[len(fk) // 2:])
            shown_cols = {(t, c) for t, c, _ in shown}
            by_name = defaultdict(set)
            for t, c, r in shown:
                by_name[norm(c)].add(r)
            guess = {k for k in sure[sid] if (k[0], k[1]) not in shown_cols}
            alone[sid], truth[sid] = guess, hidden
            mine = {(t, c): r for t, c, r in guess}
            for t in s["tables"]:
                for col in t["columns"]:
                    targets = by_name.get(norm(col["name"]), set())
                    # a table's own key, and the target's, are not the dialect's to draw
                    own_key = t["pk"] == [col["name"]]
                    if ((t["name"], col["name"]) not in shown_cols and len(targets) == 1 and not own_key
                            and next(iter(targets)) != t["name"]):
                        mine[(t["name"], col["name"])] = next(iter(targets))
            ruled[sid] = {(t, c, r) for (t, c), r in mine.items()}
        for src in ("spider", "bird", "ctu", "all"):
            ids = [i for i in truth if src in ("all", schemas[i]["source"])]
            f = lambda preds, sel: gemini.prf(*gemini.open_counts(truth, preds, sel))[2]  # noqa: E731
            boot = sorted(f(ruled, pick) - f(alone, pick)
                          for pick in ([rng.choice(ids) for _ in ids] for _ in range(1000)))
            result.setdefault(f"{src}, the dialect minus the model, 95", []).append(
                {"seed": seed, "difference": f(ruled, ids) - f(alone, ids), "95": [boot[24], boot[974]]})
            for how, preds in (("model alone", alone), ("model and the dialect", ruled)):
                p, r, f = gemini.prf(*gemini.open_counts(truth, preds, ids))
                result.setdefault(f"{src}, {how}", []).append({"seed": seed, "databases": len(ids),
                                                               "hidden keys": sum(len(truth[i]) for i in ids),
                                                               "precision": p, "recall": r, "f1": f})
    out.mkdir(parents=True, exist_ok=True)
    (out / "dialect.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    for k, runs in result.items():
        if k.endswith(", 95"):
            print(k, [f"{r['difference']:+.3f} [{r['95'][0]:+.3f}, {r['95'][1]:+.3f}]" for r in runs])
            continue
        mean = lambda x: sum(r[x] for r in runs) / len(runs)  # noqa: E731
        print(f"{k:32} databases {runs[0]['databases']:3} hidden keys {runs[0]['hidden keys']:4} "
              f"P {mean('precision'):.3f} R {mean('recall'):.3f} F1 {mean('f1'):.3f} "
              f"(F1 by seed {', '.join('%.3f' % r['f1'] for r in runs)})")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]))
