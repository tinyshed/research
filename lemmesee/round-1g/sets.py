"""Round 1g's fives judged as sets: for each database a judge never taught
on, round 1's five and two looks' fives out of fold, unnamed, shuffled and
shown side by side, each panel once. The judge says how many of each five
are worth a place on a first screen together, which repeat another of the
same five, and which five it would keep.

    python -I sets.py pack <round 1d look> <round 1e look> <round 1f look> <corpus/see> <out> "<config>" "the teachers' mean"
    JUDGE_KEY=... python -I sets.py ask <out> <endpoint, …/v1> <model> <judge> [<at once>]
    python -I sets.py gather <out> <judge>
    python -I sets.py count <out>

Each takes --into=<folder> for a second set of fives beside the first; a
configuration ending " + diverse" is chosen never holding one base and
measure twice, nor one breakdown twice. pack writes <out>/sets/packet-<n>.txt for agents, one prompt a database in
<out>/sets/gemini/ for an endpoint, and <out>/sets/key.json; "the
teachers' mean" stands for the five the teachers' own answers would choose.
count reads <out>/sets/<judge>-<n>.json.
"""

from __future__ import annotations

import json
import random
import re
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))
import diagnose  # noqa: E402

SEED = 20261102
PER = 5
TEACHERS = "the teachers' mean"
# a five that never holds one base and measure twice, nor one breakdown
# twice: set before any judge saw a five, from round 1e's gap and round
# 1d's teachers' fives, not from the judges here
DIVERSE = " + diverse"
DIVERSITY = {"base": 0.0, "measure": 9.0, "time_same": 0.0, "time_other": 0.0, "by_same": 0.0, "by_other": 9.0,
             "shape": 0.0}
# what Sonnet's repeats in the second set point to: one time or breakdown
# column once, whatever the measure, and a push away from a second panel
# of the same shape; set after Sonnet's answers, so Gemini alone counts it
AXES = " + axes"
AXIS_ONCE = {"base": 0.0, "measure": 0.0, "time_same": 9.0, "time_other": 9.0, "by_same": 9.0, "by_other": 9.0,
             "shape": 1.0}
INTO = next((a.split("=")[1] for a in sys.argv if a.startswith("--into=")), "sets")

FORM = """You are judging first-overview dashboards for several databases. For each database you see its
panels — each a title saying what it computes and the first rows its database returned — and two or three
candidate fives, each a list of five of those panels. You did not make them and are not told what made them.
Do not read, search or open any file other than this one, and run no command; when done, write your answers
with the Write tool, once, to the path given to you.

For every database, and every five of it:
 1. worth: how many of its five panels are worth a place on a first screen together — a panel that repeats
    what another panel of the same five already shows does not count, nor a single bar, an empty result, a
    column of blanks, or a picture that means less than it seems to.
 2. repeats: the panel ids in that five that repeat another panel of the same five.
Then for the database:
 3. best: the letter of the five you would keep as this database's first screen.

Write one JSON object: {"databases": {"<database>": {"fives": {"<letter>": {"worth": int, "repeats": [ids]}, ...},
"best": "<letter>"}, ...}} — every database and every five below must be answered.
"""

ASK = ("Reply with only one JSON object and no other text: "
       '{"databases": {"<database>": {"fives": {"<letter>": {"worth": int, "repeats": [panel ids]}, ...}, "best": "<letter>"}}} '
       "— every five below must be answered. Do not read, search or open any file, and run no command.")


def pack(dirs: list[Path], see_dir: Path, out: Path, configs: list[str]) -> None:
    D = diagnose.load_all(dirs, see_dir)
    scores = json.loads((out / "scores.json").read_text(encoding="utf-8"))
    counted = sorted({fe for j in ("sonnet", "gemini") for fe in D["covered"][j]})
    rng = random.Random(SEED)
    order = counted[:]
    rng.shuffle(order)
    sets_dir = out / INTO
    (sets_dir / "gemini").mkdir(parents=True, exist_ok=True)
    key: dict = {}
    for n in range(0, len(order), PER):
        body = [FORM]
        for fe in order[n:n + PER]:
            db = fe.rsplit("-", 1)[0]
            note = re.sub(r"\s*(A (development|check|look) fixture|A fixture)( of round \w+)?\.", "",
                          json.loads((dirs[0] / fe / "pool.json").read_text(encoding="utf-8"))["note"]
                          if (dirs[0] / fe).exists() else
                          next(json.loads((d / fe / "pool.json").read_text(encoding="utf-8"))["note"] for d in dirs if (d / fe).exists()))
            fives = {"round 1, B1": [p for p in D["pools"][fe] if "round 1" in p["in"]][:5]}
            for c in configs:
                base = c.removesuffix(DIVERSE).removesuffix(AXES)
                if base == TEACHERS:
                    # what a perfect student of the teachers would choose
                    s = np.array([diagnose.target(p, D["ans"]) for p in D["mine"][fe]]) + 1e-6 * np.array(
                        [p["score"] for p in D["mine"][fe]])
                else:
                    s = np.array(scores[base][fe])
                rule = DIVERSITY if c.endswith(DIVERSE) else AXIS_ONCE if c.endswith(AXES) else None
                chosen = (diagnose.look1e.Chooser(D["ids"][fe]).choose(s, rule) if rule
                          else diagnose.five(D, fe, s))
                fives[c] = [D["mine"][fe][i] for i in chosen]
            # a five chosen twice is shown once, under one letter
            letters, shown = {}, []
            for sys_name in rng.sample(list(fives), len(fives)):
                titles = tuple(sorted(p["title"] for p in fives[sys_name]))
                hit = next((l for l, t in shown if t == titles), None)
                if hit is None:
                    hit = "ABCD"[len(shown)]
                    shown.append((hit, titles))
                letters[sys_name] = hit
            # panels numbered after shuffling, so no five's numbers run together
            unique = {}
            for sys_name in fives:
                for p in fives[sys_name]:
                    unique.setdefault(p["title"], p)
            titles_shuffled = list(unique)
            rng.shuffle(titles_shuffled)
            pid = {t: f"{db}-s{k + 1:02d}" for k, t in enumerate(titles_shuffled)}
            panels = {pid[t]: unique[t] for t in titles_shuffled}
            names = list(panels)
            text = [f"\n=== database: {db} ===\n{note.strip()}\n"]
            for name in names:
                p = panels[name]
                rows = p.get("rows") or []
                lines = [" | ".join("∅" if v is None else str(v)[:40] for v in r) for r in rows[:12]]
                more = f"\n      … {len(rows) - 12} more rows" if len(rows) > 12 else ""
                err = f"\n      error: {str(p['error'])[:120]}" if p.get("error") else ""
                title = p["title"].replace(", the twelve most", " the twelve most").replace(", by ", " by ")
                text.append(f"  panel {name}\n    title: {title}\n    rows ({len(rows)}):\n      " + "\n      ".join(lines) + more + err + "\n")
            for letter, titles in sorted(shown):
                text.append(f"  five {letter}: " + ", ".join(sorted(pid[t] for t in titles)) + "\n")
            key[db] = {"fixture": fe, "letters": letters}
            body += text
            (sets_dir / "gemini" / f"{db}.txt").write_text(f"{FORM.split(chr(10) + 'Write one JSON')[0].replace('Do not read, search or open any file other than this one, and run no' + chr(10) + 'command; when done, write your answers with the Write tool, once, to the path given to you.', '')}\n\n{ASK}\n\n" + "".join(text), encoding="utf-8")
        (sets_dir / f"packet-{n // PER + 1}.txt").write_text("\n".join(body), encoding="utf-8")
    (sets_dir / "key.json").write_text(json.dumps(key, indent=1), encoding="utf-8")
    print(f"{len(key)} databases in {(len(order) + PER - 1) // PER} packets")


def ask(out: Path, url: str, model: str, judge: str = "gemini", workers: int = 8) -> None:
    """Every database's prompt this judge has not answered, through an
    OpenAI-style endpoint; the key from JUDGE_KEY, never written."""
    import os
    import urllib.request
    from concurrent.futures import ThreadPoolExecutor
    key = os.environ["JUDGE_KEY"]
    todo = [p for p in sorted((out / INTO / "gemini").glob("*.txt"))
            if not (p.with_suffix(f".{judge}.raw").exists() and '"databases"' in p.with_suffix(f".{judge}.raw").read_text(encoding="utf-8"))]

    def one(p: Path) -> str:
        body = json.dumps({"model": model, "messages": [{"role": "user", "content": p.read_text(encoding="utf-8")}],
                           "temperature": 0}).encode()
        err = ""
        for _ in range(3):
            req = urllib.request.Request(f"{url}/chat/completions", data=body, method="POST",
                                         headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            try:
                with urllib.request.urlopen(req, timeout=300) as r:
                    text = json.loads(r.read())["choices"][0]["message"]["content"]
            except Exception as e:  # noqa: BLE001 — asked again, then reported
                err = str(e)[:120]
                continue
            if text and '"databases"' in text:
                p.with_suffix(f".{judge}.raw").write_text(text, encoding="utf-8")
                return f"{p.name} ok"
            err = "no answer in the reply"
        return f"{p.name} failed: {err}"

    with ThreadPoolExecutor(workers) as pool:
        for line in pool.map(one, todo):
            print(line, flush=True)


def gather(out: Path, judge: str = "gemini") -> None:
    """The replies into one file, as the agents' answers are; the last whole
    answer of each."""
    got, bad = {}, []
    decoder = json.JSONDecoder()
    for raw in sorted((out / INTO / "gemini").glob(f"*.{judge}.raw")):
        text = raw.read_text(encoding="utf-8", errors="replace")
        answer = None
        for m in re.finditer(r"\{\s*\"databases\"", text):
            try:
                answer = decoder.raw_decode(text, m.start())[0]["databases"]
            except (json.JSONDecodeError, KeyError):
                continue
        if answer is None:
            bad.append(raw.name)
        else:
            got |= answer
    (out / INTO / f"{judge}-1.json").write_text(json.dumps({"databases": got}, ensure_ascii=False), encoding="utf-8")
    print(len(got), "databases; unreadable:", bad)


def count(out: Path) -> None:
    sets_dir = out / INTO
    key = json.loads((sets_dir / "key.json").read_text(encoding="utf-8"))
    systems = sorted({s for k in key.values() for s in k["letters"]})
    result: dict = {"databases": len(key)}
    rng = np.random.default_rng(SEED)
    for judge in ("sonnet", "gemini"):
        ans = {}
        for f in sorted(sets_dir.glob(f"{judge}-*.json")):
            ans |= json.loads(f.read_text(encoding="utf-8"))["databases"]
        dbs = [db for db in key if db in ans and all(l in ans[db]["fives"] for l in set(key[db]["letters"].values()))]
        worth = {s: {db: float(ans[db]["fives"][key[db]["letters"][s]]["worth"]) for db in dbs} for s in systems}
        repeats = {s: {db: float(len(ans[db]["fives"][key[db]["letters"][s]].get("repeats") or [])) for db in dbs} for s in systems}
        best = {s: float(np.mean([ans[db]["best"] == key[db]["letters"][s] for db in dbs])) for s in systems}
        block = {"databases answered": len(dbs)}
        for s in systems:
            block[s] = {"worth of five": round(float(np.mean(list(worth[s].values()))), 2),
                        "panels repeating another": round(float(np.mean(list(repeats[s].values()))), 2),
                        "chosen as best": round(best[s], 2)}
        for a in systems:
            for b in systems:
                if a < b:
                    d = np.array([worth[a][db] - worth[b][db] for db in dbs])
                    boots = [rng.choice(d, len(d)).mean() for _ in range(10000)]
                    block[f"{a} − {b}, worth"] = {"mean": round(float(d.mean()), 2),
                                                  "95%": [round(float(np.percentile(boots, q)), 2) for q in (2.5, 97.5)]}
        result[judge] = block
    (sets_dir / "count.json").write_text(json.dumps(result, indent=1, ensure_ascii=False), encoding="utf-8")
    print(json.dumps(result, indent=1, ensure_ascii=False))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "pack":
        a = [x for x in sys.argv[2:] if not x.startswith("--")]
        pack([Path(x) for x in a[:3]], Path(a[3]), Path(a[4]), a[5:])
    elif sys.argv[1] == "ask":
        ask(Path(sys.argv[2]), sys.argv[3], sys.argv[4], sys.argv[5], int(sys.argv[6]) if len(sys.argv) > 6 else 8)
    elif sys.argv[1] == "gather":
        gather(Path(sys.argv[2]), sys.argv[3])
    else:
        count(Path(sys.argv[2]))
