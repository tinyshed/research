"""A more sensitive count: two fives of one database side by side, and the
judge says which it would rather show. Calibrated here on pairs whose
difference the per-panel count already knows, from large to none, so that
the two ways of counting can be set against each other by how many
databases each needs to see a difference.

    python -I pairwise.py build <out>
    CLIPROXY_KEY=... python -I pairwise.py ask <out> <endpoint, …/v1> <model> <judge> [<at once>]
    python -I pairwise.py count <out> <judge>

Pairs, each a candidate against the reference, the look's out-of-fold five
(round 1g's small look):
  large      round 1's B1 five, which the look's beat by about half a panel in round 1g's sets
  click      the look's five after one click (ideas/second_click.py), +0.24 to +0.31 by the count
  small      round 1g's best look, larger with words and the lexicon, +0.06 to +0.15
  none       the small look taught by LambdaRank, within chance
Each pair is asked twice, the candidate first as A and then as B, the
panels of each five shuffled; a pair whose two fives are one is not asked
and counts zero both ways.
"""

from __future__ import annotations

import json
import math
import os
import random
import re
import sys
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE / "ideas"))
import local  # noqa: E402

SEED = 20261103
PAIRS = ("large", "click", "small", "none")
REFERENCE = "small, round 1e numbers, per panel"

ASK = """You are choosing the first screen of a dashboard for a database a person has never seen. A screen is five
panels; each panel is shown by its title, which says what it computes, and the first rows its query returned.

Below are two candidate first screens, A and B, for the same database. Which would you rather show a person new to
this database? Weigh whether each panel is worth a chart and means what it seems to, and whether the five together
show what matters in this database without repeating each other.

Reply with only one JSON object and no other text:
{{"better": "A" or "B" or "equal", "strength": 1 for slightly, 2 for clearly, 3 for by far, "why": a few words}}

=== database: {db} ===
{note}

--- screen A ---
{a}
--- screen B ---
{b}
"""


def render(p: dict) -> str:
    t = p["title"].replace(", the twelve most", " the twelve most").replace(", by ", " by ")
    rows = p.get("rows") or []
    lines = [" | ".join("∅" if v is None else str(v)[:40] for v in r) for r in rows[:12]]
    more = f"\n      … {len(rows) - 12} more rows" if len(rows) > 12 else ""
    err = f"\n      error: {str(p['error'])[:120]}" if p.get("error") else ""
    return f"  title: {t}\n    rows ({len(rows)}):\n      " + "\n      ".join(lines) + more + err


def build(out: Path) -> None:
    sys.argv = [sys.argv[0]]
    dg = local.module("pair_diagnose", ROOT / "round-1g" / "diagnose.py")
    click = local.module("pair_click", HERE / "ideas" / "second_click.py")
    click.NO_PENALTY = dg.NO_PENALTY
    corpus = ROOT / "corpus"
    D = dg.load_all([corpus / f"round-1{x}" / "look" for x in "def"], corpus / "see")
    g = corpus / "round-1g"
    ref = json.loads((g / "scores.json").read_text(encoding="utf-8"))[REFERENCE]
    best = json.loads((g / "scores-lexicon.json").read_text(encoding="utf-8"))["large, with words and the lexicon, per panel"]
    rank = json.loads((g / "scores.json").read_text(encoding="utf-8"))["small, round 1e numbers, LambdaRank"]
    rng = random.Random(SEED)
    items, counted = [], {}
    for fe in D["fes"]:
        mine, ids = D["mine"][fe], D["ids"][fe]
        pool = D["pools"][fe]
        chooser = dg.look1e.Chooser(ids)
        human = [dg.target(p, D["ans"]) for p in mine]
        five = {"reference": [mine[i] for i in chooser.choose(ref[fe], dg.NO_PENALTY)],
                "large": [p for p in pool if "round 1" in p["in"]],
                "click": [mine[i] for i in click.simulate(chooser, dg.look1e.choose.shared, ids, ref[fe], human, 1, False)[0]],
                "small": [mine[i] for i in chooser.choose(best[fe], dg.NO_PENALTY)],
                "none": [mine[i] for i in chooser.choose(rank[fe], dg.NO_PENALTY)]}
        note = re.sub(r"\s*(A (development|check|look) fixture( of round [0-9a-z]+)?)\.", "",
                      json.loads((dg.look1d.HERE.parent / "corpus" / "see" / fe / "runs.json").read_text(encoding="utf-8")).get("note", "")
                      if (corpus / "see" / fe / "runs.json").exists() else "")
        for pair in PAIRS:
            cand, refr = five[pair], five["reference"]
            if pair == "large":
                cand, refr = five["reference"], five["large"]  # the look is the candidate against round 1
            counted[f"{pair}/{fe}"] = {j: (sum(bool(D["ans"][j][p["id"]]["keep"]) for p in cand)
                                           - sum(bool(D["ans"][j][p["id"]]["keep"]) for p in refr))
                                       if all(p["id"] in D["ans"][j] for p in cand + refr) else None
                                       for j in dg.COUNTERS}
            if {p["title"] for p in cand} == {p["title"] for p in refr}:
                continue
            for order in ("candidate first", "candidate second"):
                a, b = (cand, refr) if order == "candidate first" else (refr, cand)
                a, b = rng.sample(a, len(a)), rng.sample(b, len(b))
                items.append({"pair": pair, "db": fe, "order": order,
                              "prompt": ASK.format(db=fe.rsplit("-", 1)[0], note=note,
                                                   a="\n".join(render(p) for p in a), b="\n".join(render(p) for p in b))})
    out.mkdir(parents=True, exist_ok=True)
    (out / "prompts.jsonl").write_text("\n".join(json.dumps(x, ensure_ascii=False) for x in items) + "\n", encoding="utf-8")
    (out / "counted.json").write_text(json.dumps(counted, indent=0), encoding="utf-8")
    (out / "family.json").write_text(json.dumps(D["family"]), encoding="utf-8")
    print(f"{len(items)} prompts over {len(D['fes'])} databases;",
          {p: sum(1 for x in items if x["pair"] == p) // 2 for p in PAIRS}, "pairs that differ")


def ask(out: Path, url: str, model: str, judge: str, workers: int = 8) -> None:
    key = os.environ["CLIPROXY_KEY"]
    items = [json.loads(l) for l in (out / "prompts.jsonl").read_text(encoding="utf-8").splitlines()]
    path = out / f"{judge}.jsonl"
    done = {(r["pair"], r["db"], r["order"]) for r in map(json.loads, path.read_text(encoding="utf-8").splitlines())} \
        if path.exists() else set()
    todo = [x for x in items if (x["pair"], x["db"], x["order"]) not in done]

    def one(x):
        body = json.dumps({"model": model, "messages": [{"role": "user", "content": x["prompt"]}], "temperature": 0}).encode()
        for _ in range(3):
            req = urllib.request.Request(f"{url}/chat/completions", data=body, method="POST",
                                         headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            try:
                with urllib.request.urlopen(req, timeout=300) as r:
                    text = json.loads(r.read())["choices"][0]["message"]["content"] or ""
            except Exception:  # noqa: BLE001 — asked again, then left out
                continue
            m = None
            for m in re.finditer(r"\{[^{}]*\"better\"[^{}]*\}", text):
                pass
            if m:
                try:
                    return {**{k: x[k] for k in ("pair", "db", "order")}, **json.loads(m.group())}
                except json.JSONDecodeError:
                    continue
        return None

    with path.open("a", encoding="utf-8") as f, ThreadPoolExecutor(workers) as pool:
        for r in pool.map(one, todo):
            if r is not None:
                f.write(json.dumps(r, ensure_ascii=False) + "\n")
                f.flush()
    print(f"{len(todo)} asked")


def count(out: Path, judge: str) -> None:
    joint = local.module("pair_joint", ROOT / "round-1j" / "joint.py")
    counted = json.loads((out / "counted.json").read_text(encoding="utf-8"))
    family = json.loads((out / "family.json").read_text(encoding="utf-8"))
    answers = [json.loads(l) for l in (out / f"{judge}.jsonl").read_text(encoding="utf-8").splitlines()]
    pref: dict = {}
    for r in answers:
        better = str(r.get("better", "")).strip().upper()
        s = {"A": 1, "B": -1}.get(better, 0) * (1 if r["order"] == "candidate first" else -1)
        strength = min(3, max(1, int(r.get("strength") or 1))) if s else 0
        pref.setdefault((r["pair"], r["db"]), []).append((s, s * strength))
    result = {}
    for pair in PAIRS:
        keys = [k for k in counted if k.startswith(pair + "/") and counted[k].get(judge) is not None]
        dbs = [k.split("/", 1)[1] for k in keys]
        c = np.array([counted[k][judge] for k in keys], dtype=float)
        votes = [pref.get((pair, db)) for db in dbs]
        both = [v for v in votes if v is None or len(v) == 2]
        sign = np.array([0.0 if v is None else np.mean([x[0] for x in v]) for v in votes])
        graded = np.array([0.0 if v is None else np.mean([x[1] for x in v]) for v in votes])
        consistent = [v for v in votes if v and len(v) == 2]
        row = {"databases": len(dbs), "pairs asked": sum(v is not None for v in votes),
               "answered both orders": len(consistent),
               "orders agree": round(float(np.mean([v[0][0] == v[1][0] for v in consistent])), 3) if consistent else None}
        for name, x in (("counted, kept of five", c), ("pairwise, sign", sign), ("pairwise, graded", graded)):
            sd = float(np.std(x, ddof=1)) if len(x) > 1 else math.nan
            mean = float(np.mean(x))
            z = mean / (sd / math.sqrt(len(x))) if sd > 0 else math.nan
            # databases for 80 % power at this effect, as the mean over its spread
            need = ((1.96 + 0.84) * sd / abs(mean)) ** 2 if mean and sd > 0 else math.inf
            row[name] = {"mean": round(mean, 3), "sd": round(sd, 3), "z": round(z, 2),
                         "databases for 80% power": round(need) if math.isfinite(need) else None,
                         "95%": joint.paired_interval(dict(zip(dbs, x.tolist())), family)["95%"]}
        result[pair] = row
    (out / f"count-{judge}.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(f"{'pair':6} {'dbs':>4} {'asked':>5} {'agree':>6} | {'counted mean':>12} {'z':>6} {'need':>6} | "
          f"{'sign mean':>9} {'z':>6} {'need':>6} | {'graded mean':>11} {'z':>6} {'need':>6}")
    for pair, r in result.items():
        c, s, g = r["counted, kept of five"], r["pairwise, sign"], r["pairwise, graded"]
        print(f"{pair:6} {r['databases']:>4} {r['pairs asked']:>5} {str(r['orders agree']):>6} | {c['mean']:>12.3f} {c['z']:>6.2f} "
              f"{c['databases for 80% power']:>6} | {s['mean']:>9.3f} {s['z']:>6.2f} {s['databases for 80% power']:>6} | "
              f"{g['mean']:>11.3f} {g['z']:>6.2f} {g['databases for 80% power']:>6}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, out, *rest = sys.argv[1:]
    out = Path(out)
    if cmd == "build":
        build(out)
    elif cmd == "ask":
        ask(out, rest[0], rest[1], rest[2], int(rest[3]) if len(rest) > 3 else 8)
    elif cmd == "count":
        count(out, rest[0])
