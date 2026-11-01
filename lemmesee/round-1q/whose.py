"""Round 1q: whose values — the planner's sample of each column tested for
inclusion in a key's, applied to the widened variant's keys as the
specification's rule fixes it.

    .venv/Scripts/python -I whose.py <round 1p out> <cascade out> <stats> <out> <sources>

reads the cascade's `sure.jsonl` and `questions.jsonl` (the model's keys at
or above its threshold, and its candidates in [0.2, threshold)), the
statistics `stats.py` wrote, and round 1p's hidden schemas; writes whose.json
into <out>: over every declared one-column key of the databases with
statistics, the model alone, the model with the rule, and as a bound what
the rule would do over every candidate the model scored at all. Nothing is
trained.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

ROUND_1P = Path(__file__).resolve().parent.parent / "round-1p"
sys.path.insert(0, str(ROUND_1P))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import gemini  # noqa: E402
import widen  # noqa: E402

from stats import order  # noqa: E402


def sample(col: dict) -> list:
    return [v for v in (col.get("mcv") or []) + (col.get("hist") or []) if v is not None]


def comparable(a, b) -> bool:
    return isinstance(a, (int, float)) == isinstance(b, (int, float))


def evidence(stats: dict, t: str, c: str, r: str, key: str) -> dict | None:
    """How much of the column's sample lies within the key's range, and
    whether its distinct count fits the key's table."""
    try:
        col, k = stats[t]["columns"][c], stats[r]["columns"][key]
    except KeyError:
        return None
    vs, ks = sample(col), sample(k)
    if not vs or not ks:
        return None
    lo, hi = min(ks, key=order), max(ks, key=order)
    inside = sum(1 for v in vs if comparable(v, lo) and order(lo) <= order(v) <= order(hi)) / len(vs)
    nd = col["n_distinct"]
    distinct = -nd * stats[t]["rows"] if nd < 0 else nd
    return {"inside": inside, "fits": distinct <= stats[r]["rows"]}


def main(data: Path, cascade: Path, stats_dir: Path, out: Path, sources: set) -> None:
    schemas = {s["id"]: s for s in map(json.loads, open(data / "hidden.jsonl", encoding="utf-8"))}
    stats = {}
    for f in stats_dir.glob("*/*.json"):
        if f.parent.name in sources:
            stats[f"{f.parent.name}/{f.stem}"] = json.load(open(f, encoding="utf-8"))
    ids = [i for i in stats if i in schemas and schemas[i]["fk"]]
    tables = {i: {t["name"]: t for t in schemas[i]["tables"]} for i in ids}

    def key_of(sid, r):
        k, _ = widen.key_of(tables[sid][r], True)
        return k["name"] if k else None

    sure, band = defaultdict(set), defaultdict(list)
    for line in open(cascade / "sure.jsonl", encoding="utf-8"):
        sid, t, c, r, s = json.loads(line)
        if sid in stats:
            sure[sid].add((t, c, r))
    for line in open(cascade / "questions.jsonl", encoding="utf-8"):
        _, sid, t, c, r, s = json.loads(line)
        if sid in stats:
            band[sid].append((t, c, r))
    ruled, drawn, dropped, unknown = defaultdict(set), 0, 0, 0
    for sid in ids:
        for t, c, r in sure[sid]:
            e = evidence(stats[sid], t, c, r, key_of(sid, r) or "")
            if e is not None and e["inside"] < 0.5:
                dropped += 1
                continue
            ruled[sid].add((t, c, r))
        for t, c, r in band[sid]:
            e = evidence(stats[sid], t, c, r, key_of(sid, r) or "")
            if e is None:
                unknown += 1
            elif e["inside"] == 1.0 and e["fits"]:
                ruled[sid].add((t, c, r))
                drawn += 1
    truth = {i: {tuple(k) for k in schemas[i]["fk"]} for i in ids}
    # the band's candidates with a sample, and how inclusion sorts them
    band_rows = []
    for sid in ids:
        for t, c, r in band[sid]:
            k = key_of(sid, r)
            e = evidence(stats[sid], t, c, r, k or "") if k else None
            if e is None:
                continue
            ks = sample(stats[sid][r]["columns"][k])
            band_rows.append({"key": (t, c, r) in truth[sid], "included": e["inside"] == 1.0 and e["fits"],
                              "text key": isinstance(ks[0], str)})
    share = lambda rows: (sum(x["key"] for x in rows) / len(rows)) if rows else 0.0  # noqa: E731
    inc = [x for x in band_rows if x["included"]]
    band_summary = {"candidates with a sample": len(band_rows), "keys among them": sum(x["key"] for x in band_rows),
                    "included": len(inc), "keys among the included": share(inc),
                    "keys among the included, text keys": share([x for x in inc if x["text key"]]),
                    "keys among the included, other keys": share([x for x in inc if not x["text key"]])}
    import random
    rng = random.Random(20261101)
    f = lambda preds, sel: gemini.prf(*gemini.open_counts(truth, preds, sel))[2]  # noqa: E731
    boot = sorted(f(ruled, pick) - f(sure, pick) for pick in ([rng.choice(ids) for _ in ids] for _ in range(1000)))
    result = {"band": band_summary,
              "the rule minus the model alone": {"difference": f(ruled, ids) - f(sure, ids), "95": [boot[24], boot[974]]},
              "databases": len(ids), "keys declared": sum(len(v) for v in truth.values()),
              "band candidates drawn": drawn, "sure keys dropped": dropped, "candidates without statistics": unknown}
    for name in ("spider", "bird", "all"):
        sel = [i for i in ids if name in ("all", schemas[i]["source"])]
        for how, preds in (("model alone", sure), ("model and its values", ruled)):
            result[f"{name}, {how}"] = dict(zip(("precision", "recall", "f1"),
                                               gemini.prf(*gemini.open_counts(truth, preds, sel))))
    out.mkdir(parents=True, exist_ok=True)
    (out / "whose.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]), set(sys.argv[5].split(",")))
