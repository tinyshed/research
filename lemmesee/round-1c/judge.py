"""Round 1c's panels judged blind by models that were not its teachers:
every fixture's pool — round 1's five with statistics, round 1c's five from
the schema and with statistics, each panel once — shuffled, shown as round
1's judging form shows it, without the configuration; then the answers
unblinded and counted per configuration.

    python -I judge.py pack <corpus/round-1c/see> <corpus/see> <out> [<earlier out>]
    python -I judge.py count <out> <answers.json>...

pack writes <out>/packet.txt, what a judge reads, and <out>/key.json, which
panel each configuration chose; given an earlier pack, the packet holds
only the panels it did not, and the key names the others as it did. count
reads the judges' answers, each judge's files merged.
"""

from __future__ import annotations

import json
import random
import re
import sys
from collections import defaultdict
from pathlib import Path

SEED = 20261028
FIXTURES = ["chinook", "airlines", "ergastf1", "northwind", "ccs", "lahman", "classicmodels", "credit"]
CONFIGS = {"O": "round 1, B1", "A": "round 1c, A", "B1": "round 1c, B1"}

FORM = """You are judging panels proposed for a first overview dashboard of eight databases. You did not make them,
and you are not told what made them. Do not read, search or open any file other than this one, and run no
command; when done, write your answers with the Write tool, once, to the path given at the end.

For every panel answer, as round 1's judging form asks:
 1. keep: would you keep this panel on a first overview of this database? true or false
 2. meaning: does the picture mean what it seems to? "yes", "no", or "cannot tell"
 3. wrong: if not, what is wrong, in a few words (wrong population, counted twice through a join, units or
    currencies mixed, a level summed, the wrong time, a code or identifier treated as a number, other); else null
A panel is shown by its title, which says what it computes, and the first rows of its result as they came back
from the database. Judge what a person new to the database would want first, not whether the title is well worded.

After each database, give up to five questions you would want answered first about it that no panel answered.

Write one JSON object: {"panels": {"<panel id>": {"keep": bool, "meaning": "...", "wrong": str|null}, ...},
"missing": {"<database>": ["question", ...], ...}} — every panel id below must be answered.
"""


def text_of(p: dict) -> str:
    rows = p.get("rows") or []
    shown = rows[:12]
    lines = [" | ".join("∅" if v is None else str(v)[:40] for v in r) for r in shown]
    more = f"\n      … {len(rows) - len(shown)} more rows" if len(rows) > len(shown) else ""
    err = f"\n      error: {p['error']}" if p.get("error") else ""
    # the title alone says what is computed: two makers' SQL and punctuation
    # would tell the judge there are two makers
    title = p["title"].replace(", the twelve most", " the twelve most").replace(", by ", " by ")
    return (f"    title: {title}\n    rows ({len(rows)}):\n      "
            + "\n      ".join(lines) + more + err)


def judged_titles(earlier: Path | None) -> dict[str, str]:
    if earlier is None:
        return {}
    packet = (earlier / "packet.txt").read_text(encoding="utf-8")
    return {t: n for n, t in re.findall(r"panel (\w+)\n    title: (.*)", packet)}


def pack(see1c: Path, see: Path, out: Path, earlier: Path | None = None) -> None:
    rng = random.Random(SEED + (1 if earlier else 0))
    out.mkdir(parents=True, exist_ok=True)
    before = judged_titles(earlier)
    body, key = [FORM], {}
    order = FIXTURES[:]
    rng.shuffle(order)
    for fx in order:
        html = (see1c / f"{fx}.html").read_text(encoding="utf-8")
        data = json.loads(re.search(r"const DATA = (\{.*?\});\n", html, re.S).group(1))
        panels = {p["id"]: p for p in data["panels"]}
        # one panel for one title: the same question chosen twice is judged once
        by_title, chosen = {}, defaultdict(list)
        for cfg, ids in data["configs"].items():
            for pid in ids:
                t = panels[pid]["title"]
                by_title.setdefault(t, pid)
                chosen[cfg].append(t)
        shown = lambda t: t.replace(", the twelve most", " the twelve most").replace(", by ", " by ")
        titles = [t for t in by_title if shown(t) not in before]
        rng.shuffle(titles)
        names = {t: f"{fx[:3]}{'n' if before else ''}{i + 1:02d}" for i, t in enumerate(titles)}
        names.update({t: before[shown(t)] for t in by_title if shown(t) in before})
        if not titles:
            key[fx] = {cfg: [names[t] for t in ts] for cfg, ts in chosen.items()}
            continue
        snap = json.loads((see / next(d.name for d in see.iterdir() if d.name.startswith(fx + "-")) / "runs.json").read_text(encoding="utf-8"))
        note = re.sub(r"\s*(A development fixture|A check fixture( of round 1c)?)\.", "", snap.get("note", "")).replace(" Gold questions answered are marked.", "")
        body.append(f"\n=== database: {fx} ===\n{note}\n")
        for t in titles:
            body.append(f"  panel {names[t]}\n{text_of(panels[by_title[t]])}\n")
        key[fx] = {cfg: [names[t] for t in ts] for cfg, ts in chosen.items()}
    (out / "packet.txt").write_text("\n".join(body), encoding="utf-8")
    (out / "key.json").write_text(json.dumps(key, indent=1), encoding="utf-8")
    print(f"{sum(len(set(x for v in k.values() for x in v)) for k in key.values())} panels in {len(key)} databases")


def count(out: Path, answers: list[Path]) -> None:
    key = json.loads((out / "key.json").read_text(encoding="utf-8"))
    judges: dict[str, dict] = {}
    for p in answers:
        # sonnet.json and sonnet-again.json are one judge's
        judges.setdefault(p.stem.split("-")[0], {}).update(json.loads(p.read_text(encoding="utf-8"))["panels"])
    result = {}
    for cfg, name in CONFIGS.items():
        row = {}
        for judge, ans in judges.items():
            kept = harmful = false = n = 0
            for fx, cfgs in key.items():
                for pid in cfgs.get(cfg, []):
                    a = ans.get(pid)
                    if a is None:
                        continue
                    n += 1
                    kept += bool(a["keep"])
                    false += a["meaning"] == "no"
                    harmful += bool(a["keep"]) and a["meaning"] == "no"
            row[judge] = {"panels": n, "kept": kept, "kept share": round(kept / max(n, 1), 3),
                          "false": false, "harmful": harmful}
        result[name] = row
    # agreement between two judges on keep, over every panel
    names = list(judges)
    if len(names) == 2:
        a, b = judges[names[0]], judges[names[1]]
        common = [k for k in a if k in b]
        result["agreement on keep"] = round(sum(a[k]["keep"] == b[k]["keep"] for k in common) / max(len(common), 1), 3)
        result["panels judged by both"] = len(common)
    # per fixture, kept of five, both judges together
    per = {}
    for fx, cfgs in key.items():
        per[fx] = {CONFIGS[c]: {j: sum(bool(judges[j].get(p, {}).get("keep")) for p in ids) for j in judges} for c, ids in cfgs.items()}
    result["kept of five, per database"] = per
    (out / "count.json").write_text(json.dumps(result, indent=1, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(result, indent=1, ensure_ascii=False))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "pack":
        pack(Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]), Path(sys.argv[5]) if len(sys.argv) > 5 else None)
    else:
        count(Path(sys.argv[2]), [Path(p) for p in sys.argv[3:]])
