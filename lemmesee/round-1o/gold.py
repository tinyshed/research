"""Round 1o's gold numbers: for each of round 1m's twelve bench schemas, the
row of numbers the person running that application would open the weekly
screen with, written by a Haiku agent from the schema and round 1m's spec
of what the application is, before it sees what kpi.py makes.

    python -I gold.py prompts <bench dir> <schemas an agent>
    python -I gold.py check <bench dir>

prompts writes <bench dir>/gold-prompts/agent-<k>.txt, each asking an agent
to write <bench dir>/<key>/numbers.json; check reads them against the schemas.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

PROMPT = """You are the person responsible for each application below: you run it for its users, and every week you open one screen to see how it is going. Do not run any command and do not read any file other than this one; when done, write each schema's answer with the Write tool, once, to the path given beside it.

For each schema, write the row of six to eight numbers that screen opens with: single figures, each one query over the schema's tables as they are, such as how many use the product, how many did the thing it is for, how many did it twice or more, the share of one among another, how many are new in the last 7 or 30 days, what it earned in the last 30 days, the share of failures in the last 7 days. Choose what matters for this application; leave out what nobody would act on.

Write one JSON object a schema, exactly in this form:
{"numbers": [
  {"title": "a short title",
   "what": "count" | "count distinct" | "two or more" | "sum" | "average" | "share",
   "table": "<the table whose rows are counted or summed>",
   "column": null, or the column summed or averaged, or for "count distinct" and "two or more" the column whose values are counted (a key to the people),
   "where": null, or a SQL condition on that table's rows, such as a status that counts as done,
   "days": null, or 7 or 30: only rows of the last so many days, by the table's own time,
   "over": null, or for a "share" the number it is divided by, in this same form}
  ...]}

Rules:
- Use only the schema's own tables and columns, spelled as given.
- A "share" names its top as what, table, column, where and days, and its bottom in "over".
- Six to eight numbers a schema, the most important first.
"""


def prompts(bench: Path, per: int) -> None:
    keys = sorted(p.parent.name for p in bench.glob("m*/spec.json"))
    out = bench / "gold-prompts"
    out.mkdir(exist_ok=True)
    for k in range(0, len(keys), per):
        body = [PROMPT]
        for key in keys[k:k + per]:
            about = json.loads((bench / key / "spec.json").read_text(encoding="utf-8")).get("about", "")
            path = (bench / key / "numbers.json").resolve()
            body.append(f"\n=== schema {key}: write its answer to {path} ===\nWhat it is: {about}\n\n"
                        + (bench / key / "text.txt").read_text(encoding="utf-8"))
        (out / f"agent-{k // per}.txt").write_text("\n".join(body), encoding="utf-8")
    print(f"{(len(keys) + per - 1) // per} prompts")


def check(bench: Path) -> None:
    for p in sorted(bench.glob("m*/schema.json")):
        n = p.with_name("numbers.json")
        if not n.exists():
            print(p.parent.name, "no numbers")
            continue
        snap = json.loads(p.read_text(encoding="utf-8"))
        cols = {t["name"]: {c["name"] for c in t["columns"]} for t in snap["tables"]}
        bad = []

        def one(x, where):
            if x.get("table") not in cols:
                bad.append(f"{where}: no table {x.get('table')}")
            elif x.get("column") and x["column"] not in cols[x["table"]]:
                bad.append(f"{where}: no column {x['table']}.{x['column']}")
            if x.get("over"):
                one(x["over"], where + " over")

        try:
            nums = json.loads(n.read_text(encoding="utf-8"))["numbers"]
        except (json.JSONDecodeError, KeyError) as e:
            print(p.parent.name, "unreadable:", e)
            continue
        for i, x in enumerate(nums):
            one(x, f"#{i + 1}")
        print(p.parent.name, f"{len(nums)} numbers;", "ok" if not bad else "; ".join(bad))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, *a = sys.argv[1:]
    if cmd == "prompts":
        prompts(Path(a[0]), int(a[1]))
    elif cmd == "check":
        check(Path(a[0]))
