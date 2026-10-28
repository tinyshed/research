"""Rules a language model writes: the knowledge the look lacks is what
words mean beside what a result shows, which a language model has and 93
databases cannot teach. Instead of labels, it writes a scoring program;
programs are kept by how well they order panels as their teachers would,
and the best is counted by the judges that never teach.

    python -I funsearch.py data <out>                    # panels as the programs see them
    python -I funsearch.py check <out> <program.py>      # a program's fitness, and its count on the 93
    CLIPROXY_KEY=... python -I funsearch.py evolve <out> <endpoint> <model> <rounds> [<per round>]

Programs evolve on the GitTables pilot, taught by Jev (fast) and Luna; the
93 databases of rounds 1d-1f are never shown to the writer and count the
best program once, by Opus, Sonnet and Gemini. A program is a function
`score(panel) -> float` run in a subprocess with no builtins beyond
arithmetic, no imports and two seconds for every panel of a set.
"""

from __future__ import annotations

import json
import math
import os
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE.parent))
import local  # noqa: E402

API = """Each panel is a dict:
  title: str                         e.g. "sum of orders.amount by customers.country, the twelve most"
  aggregate: "count"|"sum"|"avg"|"distinct"|...
  shape: "series"|"breakdown"|"top"|"split"|"distribution"
  words: {"base": [...], "measure": [...], "time": [...], "by": [...]}   lower-case tokens of each part's names
  rows: [[label, value], ...]        the first twelve rows of the result, as text; None for a blank
  result: {rows, empty, values_missing, top_share, distinct_values, all_equal, cv, negative, log_total,
           zeros, blank_labels, timed, span_days, coverage, buckets}  numbers; NaN where they do not apply
  lexicon: {"<part>_<prop>": float}  for part in base, measure, time, by and prop in id, money, count, sum_ok,
           audit_time, event_time, good_group, cryptic, importance: the largest over the part's words, NaN if none known
Return a float: higher means a person new to the database would rather keep this panel on a first overview."""

RUNNER = r"""
import json, math, sys
SAFE = {k: __builtins__[k] if isinstance(__builtins__, dict) else getattr(__builtins__, k)
        for k in ("abs", "min", "max", "len", "sum", "any", "all", "float", "int", "str", "bool", "round", "sorted",
                  "range", "enumerate", "zip", "isinstance", "list", "dict", "set", "tuple", "map", "filter")}
src = open(sys.argv[1], encoding="utf-8").read()
env = {"__builtins__": SAFE, "math": math}
exec(compile(src, "program", "exec"), env)
panels = json.load(open(sys.argv[2], encoding="utf-8"))
out = []
for p in panels:
    try:
        v = float(env["score"](p))
        out.append(v if math.isfinite(v) else 0.0)
    except Exception:
        out.append(0.0)
print(json.dumps(out))
"""


def panel_view(p: dict, ident, lex: dict, dg) -> dict:
    lx = dg.lexicon(p["slots"], lex)
    return {"title": p["title"], "aggregate": ident[1][0], "shape": p.get("template"),
            "words": {s: [w.split(":", 1)[-1] for w in p["slots"].get(s, []) if not w.startswith("#")] for s in dg.SLOTS},
            "rows": [[None if v is None else str(v)[:40] for v in r] for r in (p.get("rows") or [])[:12]],
            "result": {k: (None if not math.isfinite(v) else v) for k, v in p["result"].items()},
            "lexicon": {k[2:].replace("_known", "_known"): (None if not math.isfinite(v) else v) for k, v in lx.items()}}


def data(out: Path) -> None:
    sys.argv = [sys.argv[0]]
    dg = local.module("fs_diagnose", ROOT / "round-1g" / "diagnose.py")
    look1d = dg.look1d
    corpus = ROOT / "corpus"
    lex = json.loads((corpus / "round-1g" / "lexicon" / "lexicon.json").read_text(encoding="utf-8"))
    out.mkdir(parents=True, exist_ok=True)
    # the 93, as round 1g reads them: words placed by the generator, judges' answers
    D = dg.load_all([corpus / f"round-1{x}" / "look" for x in "def"], corpus / "see")
    teach93 = {j: {} for j in ("jevfast", "lunap")}
    for x in "def":
        for j in teach93:
            teach93[j] |= look1d.labels(corpus / f"round-1{x}" / "look", j)
    rows93 = []
    for fe in D["fes"]:
        for p, ident in zip(D["mine"][fe], D["ids"][fe]):
            v = panel_view(p, ident, lex, dg)
            v |= {"_db": fe, "_family": D["family"][fe], "_id": p["id"],
                  "_judges": {j: bool(D["ans"][j][p["id"]]["keep"]) for j in dg.COUNTERS if p["id"] in D["ans"][j]}}
            rows93.append(v)
    (out / "panels-93.json").write_text(json.dumps(rows93, ensure_ascii=False), encoding="utf-8")
    # the pilot, as round 1k built it, its words placed the same way
    gt = corpus / "round-1k" / "gt"
    ans = {j: look1d.labels(gt / "look", j) for j in ("jevfast", "lunap")}
    rows_gt = []
    for pool in sorted((gt / "look").glob("*/pool.json")):
        fe = pool.parent.name
        snap = json.loads((gt / "see" / fe / "snapshot.json").read_text(encoding="utf-8"))
        schema = look1d.load(snap, True)
        by_title = {}
        for q in look1d.generate(schema):
            by_title.setdefault(q.title(schema), (look1d.rank.slots(schema, q), look1d.identity(schema, q)))
        for p in json.loads(pool.read_text(encoding="utf-8"))["panels"]:
            slots, ident = by_title.get(p["title"], ({s: ["#none"] for s in dg.SLOTS}, (0, ("count", frozenset()), None, None)))
            p["slots"] = slots
            v = panel_view(p, ident, lex, dg)
            t = [ans[j][p["id"]]["p_keep"] for j in ans if p["id"] in ans[j]]
            v |= {"_db": fe, "_id": p["id"], "_teachers": float(np.mean(t)) if t else None}
            rows_gt.append(v)
    (out / "panels-gt.json").write_text(json.dumps(rows_gt, ensure_ascii=False), encoding="utf-8")
    print(f"{len(rows93)} panels of the 93, {len(rows_gt)} of the pilot")


def run_program(program: Path, panels_path: Path, timeout: float) -> list[float] | None:
    runner = program.with_suffix(".runner.py")
    runner.write_text(RUNNER, encoding="utf-8")
    try:
        p = subprocess.run([sys.executable, "-I", str(runner), str(program), str(panels_path)], capture_output=True,
                           text=True, timeout=timeout)
        return json.loads(p.stdout) if p.returncode == 0 else None
    except (subprocess.TimeoutExpired, json.JSONDecodeError):
        return None
    finally:
        runner.unlink(missing_ok=True)


def within_auc(groups: dict, scores, labels) -> float:
    joint = local.module("fs_joint", ROOT / "round-1j" / "joint.py")
    aucs = []
    for idx in groups.values():
        y = np.asarray([labels[i] for i in idx], dtype=float)
        if np.isnan(y).any() or len(set(y >= 0.5)) < 2:
            continue
        s = np.asarray([scores[i] for i in idx])
        # a fractional teacher target: AUC against keep at one half
        a = joint.auc((y >= 0.5).astype(int), s)
        if math.isfinite(a):
            aucs.append(a)
    return float(np.mean(aucs)) if aucs else math.nan


def fitness(out: Path, program: Path) -> float:
    panels = json.loads((out / "panels-gt.json").read_text(encoding="utf-8"))
    s = run_program(program, out / "panels-gt.json", 2.0 * len(panels) / 1000 + 10)
    if s is None:
        return math.nan
    groups = {}
    for i, p in enumerate(panels):
        groups.setdefault(p["_db"], []).append(i)
    return within_auc(groups, s, [p["_teachers"] if p["_teachers"] is not None else math.nan for p in panels])


def check(out: Path, program: Path) -> None:
    panels = json.loads((out / "panels-93.json").read_text(encoding="utf-8"))
    s = run_program(program, out / "panels-93.json", 30)
    result = {"fitness on the pilot": fitness(out, program)}
    if s is not None:
        groups = {}
        for i, p in enumerate(panels):
            groups.setdefault(p["_db"], []).append(i)
        for j in ("opus", "sonnet", "gemini"):
            labels = [float(p["_judges"][j]) if j in p["_judges"] else math.nan for p in panels]
            result[f"{j} AUC on the 93"] = within_auc(groups, s, labels)
    print(json.dumps(result, indent=1))


PROMPT = """You write a scoring function for panels proposed on the first overview dashboard of a database nobody has
seen. A panel is one aggregate question: a count, sum or average, over time or by a category, with the first rows of
its result. What makes a panel worth keeping is mostly what its words mean beside what its result shows: a sum of card
numbers, a breakdown by a one-letter code or a timeline of a last-update column is not worth it; revenue by month,
delay by carrier or population by continent is.

{api}

Write Python: `def score(panel):` and helpers, using only arithmetic, `math`, and builtins such as len, min, max, sum,
any, all, sorted. No imports, no files. Keep it under 120 lines.

The best functions so far, with their fitness (within-table AUC against the teachers' keep, higher is better):
{best}

Panels the best one orders most wrongly (teachers' keep, its score, the panel):
{errors}

Write {n} new, different functions that should do better. Separate them with a line `# ---`. Reply with code only."""


def evolve(out: Path, url: str, model: str, rounds: int, per_round: int = 4) -> None:
    key = os.environ["CLIPROXY_KEY"]
    progs = out / "programs"
    progs.mkdir(exist_ok=True)
    panels = json.loads((out / "panels-gt.json").read_text(encoding="utf-8"))
    pop = {p: fitness(out, p) for p in sorted(progs.glob("p*.py"))}
    if not pop:
        seed = progs / "p0000.py"
        seed.write_text("def score(panel):\n    return -panel['result'].get('top_share') or 0.0\n", encoding="utf-8")
        pop[seed] = fitness(out, seed)
    for r in range(rounds):
        best = sorted((p for p in pop if math.isfinite(pop[p])), key=lambda p: -pop[p])[:3]
        s = run_program(best[0], out / "panels-gt.json", 60) or [0.0] * len(panels)
        worst = sorted(range(len(panels)), key=lambda i: -abs((panels[i]["_teachers"] or 0.5) - (s[i] > np.median(s))))[:12]
        errors = "\n".join(f"{panels[i]['_teachers']:.2f} {s[i]:.2f} {panels[i]['title']} | "
                           f"{'; '.join(' | '.join(str(x) for x in row) for row in panels[i]['rows'][:3])[:120]}" for i in worst)
        text = PROMPT.format(api=API, n=per_round, errors=errors,
                             best="\n\n".join(f"# fitness {pop[p]:.4f}\n{p.read_text(encoding='utf-8')}" for p in best))
        body = json.dumps({"model": model, "messages": [{"role": "user", "content": text}]}).encode()
        req = urllib.request.Request(f"{url}/chat/completions", data=body, method="POST",
                                     headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
        with urllib.request.urlopen(req, timeout=600) as resp:
            reply = json.loads(resp.read())["choices"][0]["message"]["content"]
        reply = re.sub(r"```(?:python)?", "", reply)
        for code in [c.strip() for c in reply.split("# ---") if "def score" in c]:
            path = progs / f"p{len(list(progs.glob('p*.py'))):04d}.py"
            path.write_text(code + "\n", encoding="utf-8")
            pop[path] = fitness(out, path)
        top = max((p for p in pop if math.isfinite(pop[p])), key=lambda p: pop[p])
        print(f"round {r + 1}: {len(pop)} programs, best {top.name} at {pop[top]:.4f}", flush=True)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, out, *rest = sys.argv[1:]
    out = Path(out)
    if cmd == "data":
        data(out)
    elif cmd == "check":
        check(out, Path(rest[0]))
    elif cmd == "evolve":
        evolve(out, rest[0], rest[1], int(rest[2]), int(rest[3]) if len(rest) > 3 else 4)
