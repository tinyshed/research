"""Each gold question against round 1's pipeline as it last ran: inexpressible,
not generated, removed by a hard rule, abstained, outranked, or among a
configuration's five — and, where it was run, how it came out.

    python -I diagnose.py <corpus/see> [<name>]

reads <name>.json beside it — questions.json when no name is given — and
each fixture's proposal and runs from <corpus/see>/<fixture>-<engine>/,
prints what it found about the questions nothing generated, and writes
diagnosis.md, or <name>-diagnosis.md, beside it.
"""

import json
import sys
from collections import Counter
from pathlib import Path

CONFIGS = ["O", "A", "B1"]


def norm(name: str) -> str:
    return "".join(c.lower() for c in name if c.isalnum())


def ref(r) -> str | None:
    return f"{norm(r['table'])}.{norm(r['column'])}" if r else None


def matches(c: dict, m: dict) -> bool:
    if c["template"] != m["template"] or norm(c["table"]) != norm(m["table"]):
        return False
    if "aggregate" in m and c["aggregate"] != m["aggregate"]:
        return False
    for field in ("measure", "time", "category"):
        if field in m and ref(c.get(field)) != norm(m[field].split(".")[0]) + "." + norm(m[field].split(".")[1]):
            return False
    if "join_to" in m and (not c.get("join") or norm(c["join"]["to"]["table"]) != norm(m["join_to"])):
        return False
    if c["template"] == "T5" and "measure" not in m and c.get("measure"):
        return False
    return True


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def standing(ids, scores, panels):
    """The best a configuration did with any of the candidates."""
    best = None
    for i in ids:
        if i in panels:
            return (0, f"in the five, #{panels.index(i) + 1}")
        s = scores.get(i, {})
        if s.get("removed_by"):
            here = (3, f"removed: {s['removed_by'].split(':')[0]}")
        elif s.get("abstained"):
            here = (2, f"abstained: {s['abstained']}")
        else:
            here = (1, f"rank {s.get('rank')}")
        if best is None or here < best:
            best = here
    return best


def column_hint(proposal: dict, dotted: str) -> str:
    table, column = dotted.split(".")
    for t in proposal["roles"]:
        if norm(t["table"]) == norm(table):
            for c in t["columns"]:
                if norm(c["column"]) == norm(column):
                    held = ", ".join(f"{h['role']} ({'/'.join(e['kind'] for e in h['evidence'])})" for h in c["held"])
                    return f"{dotted}: {held or 'no role'}"
            roles = ", ".join(h["role"] for h in t["held"]) or "no role"
            return f"{dotted}: no such column; {table} is {roles}"
    return f"{dotted}: no such table"


def main(see: Path, name: str | None) -> None:
    here = Path(__file__).parent
    gold = load(here / f"{name or 'questions'}.json")
    lines = [
        "# Diagnosis of the gold questions",
        "",
        "Written by `diagnose.py` from the last run of `see.sh` on each fixture.",
        "**in the five** — the question is among the configuration's five;",
        "**rank** — generated and allowed, outranked; **abstained**, **removed** — by",
        "the code given; **not generated** — a template could say it and the",
        "generator did not; **inexpressible** — no template can, for the reason given.",
        "",
    ]
    totals = Counter()
    for fixture, spec in gold["fixtures"].items():
        folder = see / f"{fixture}-{spec['engine']}"
        proposal = load(folder / "proposal" / "candidates.json")
        scores = {k: {s["id"]: s for s in load(folder / "proposal" / f"scores-{k}.json")["candidates"]} for k in CONFIGS}
        panels = {k: [p["id"] for p in load(folder / "proposal" / f"panels-{k}.json")["panels"]] for k in CONFIGS}
        runs = {p["id"]: p for p in load(folder / "runs.json")["panels"]}
        lines += [f"## {fixture}, {spec['engine']}", "", "| # | Question | O | A | B1 | Run |", "|---|---|---|---|---|---|"]
        for n, q in enumerate(spec["questions"], 1):
            if q["needs"]:
                status = {k: "—" for k in CONFIGS}
                verdict = "inexpressible: " + "; ".join(q["needs"])
                lines.append(f"| {n} | {q['question']} | {verdict} | | | |")
                totals["inexpressible"] += 1
                continue
            ids = [c["id"] for c in proposal["candidates"] if matches(c, q["match"])]
            if not ids:
                lines.append(f"| {n} | {q['question']} | not generated | | | |")
                totals["not generated"] += 1
                hints = [column_hint(proposal, q["match"][f]) for f in ("measure", "time", "category") if f in q["match"]]
                print(f"{fixture} {n}, {q['question']}: not generated — " + " | ".join(hints))
                continue
            status = {k: standing(ids, scores[k], panels[k]) for k in CONFIGS}
            ran = next((runs[i] for i in ids if i in runs), None)
            run = "—" if ran is None else (ran["error"] or f"{len(ran['rows'])} rows")
            lines.append(f"| {n} | {q['question']} | " + " | ".join(status[k][1] for k in CONFIGS) + f" | {run} |")
            totals["best in B1: " + ["in the five", "outranked", "abstained", "removed"][status["B1"][0]]] += 1
            totals["best in A: " + ["in the five", "outranked", "abstained", "removed"][status["A"][0]]] += 1
        lines.append("")
    lines += ["## Totals", "", "| | Questions |", "|---|---|"]
    lines += [f"| {k} | {v} |" for k, v in sorted(totals.items())]
    (here / (f"{name}-diagnosis.md" if name else "diagnosis.md")).write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(dict(totals))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), sys.argv[2] if len(sys.argv) > 2 else None)
