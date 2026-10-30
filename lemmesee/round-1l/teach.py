"""Round 1c's teachers asked the product's question: the same 480 SchemaPile
schemas in the same batches of six, the role changed from a person just given
the database to the analyst of what it serves, building the screen they open
every week. The answers are in round 1c's form, so its learn.py reads them.

    python -I teach.py batches <round 1c teacher dir> <out>
    TEACHER_KEY=... python -I teach.py ask <out> <endpoint, …/v1> <model> <teacher> [<at once>]
    python -I teach.py gather <out> <teacher>

batches writes <out>/batches/<k>.txt and copies schemas.jsonl and text/; ask keeps each
reply as <out>/answers-<teacher>/<k>.raw and, when it reads, <k>.json; a
batch already answered is not asked again. gather reads the replies again.
"""

from __future__ import annotations

import json
import os
import shutil
import sys
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

PROMPT = """You are writing lists of questions from database schemas alone. Do not read, search or open any files and do not run any commands or tools: answer from the text below only.

For each schema below, you are the analyst responsible for what that database serves — a product, a shop, a service, a league, a register, a study — and you are building the screen you will open every week to see how it is going: who and what is active and how much, what it earns and what it costs, what succeeds and what fails, and how all of that changes. Write the 10 questions that screen should answer, the most important first: your first five are the screen itself. Each question must be answerable by one SQL query over that schema's tables as they are — a count, sum or average over rows, grouped by a time period and/or one breakdown, or a top 12 — and drawn as one chart. If a schema holds little of a business, still give its best ten.

Rules:
- Use only the tables and columns listed for that schema. Do not invent any.
- Leave out an inventory of a table (rows by an internal code or an id), technical bookkeeping (import times, update stamps, logs of the machinery), what nobody would act on, and the same activity counted again from another table.
- It is fine for a question to need joins, an expression (such as price × quantity) or a filter: say so in the fields.
- A time period may be a year or a month held as an integer column: give it as the column with bucket "year" or "month".

Reply with only one JSON object and no other text: {"<schema key>": [10 question objects], ...} for every schema key below. Each question object:
{"question": "a short English question",
 "why": "why it matters, in a few words",
 "base": "the table whose rows are counted or aggregated",
 "measure": "count(*)" or "sum(table.column)" or "avg(table.column)" or "count(distinct table.column)" or "sum(an expression over columns)",
 "time": null or {"column": "table.column", "bucket": "day" | "week" | "month" | "quarter" | "year"},
 "breakdown": null or "table.column" — what the result is grouped by besides time,
 "top": null or 12 — if it lists the top items,
 "filter": null or a SQL condition,
 "joins": ["child_table.column -> parent_table.column", ...] or []}
"""


def batches(first: Path, out: Path) -> None:
    """Round 1c's batches with the product's prompt in place of its own."""
    (out / "batches").mkdir(parents=True, exist_ok=True)
    shutil.copyfile(first / "schemas.jsonl", out / "schemas.jsonl")
    shutil.copytree(first / "text", out / "text", dirs_exist_ok=True)
    n = 0
    for p in sorted((first / "batches").glob("*.txt")):
        text = p.read_text(encoding="utf-8")
        at = text.index("=== schema ")
        (out / "batches" / p.name).write_text(PROMPT + "\n" + text[at:], encoding="utf-8")
        n += 1
    print(f"{n} batches")


def read(text: str, keys: list[str]) -> dict | None:
    """The last whole JSON object of the reply that answers the batch's schemas."""
    found = None
    for at in (i for i, c in enumerate(text) if c == "{"):
        try:
            obj = json.JSONDecoder().raw_decode(text, at)[0]
        except json.JSONDecodeError:
            continue
        if isinstance(obj, dict) and any(k in obj for k in keys):
            found = {k: v for k, v in obj.items() if k in keys and isinstance(v, list)}
    return found


def keys_of(batch: Path) -> list[str]:
    return [line.split()[2] for line in batch.read_text(encoding="utf-8").splitlines() if line.startswith("=== schema ")]


def ask(out: Path, url: str, model: str, teacher: str, workers: int = 8) -> None:
    key = os.environ["TEACHER_KEY"]
    answers = out / f"answers-{teacher}"
    answers.mkdir(exist_ok=True)
    todo = [p for p in sorted((out / "batches").glob("*.txt")) if not (answers / f"{p.stem}.json").exists()]

    def one(p: Path) -> str:
        keys = keys_of(p)
        body = json.dumps({"model": model, "messages": [{"role": "user", "content": p.read_text(encoding="utf-8")}],
                           "temperature": 0, "max_tokens": 32000}).encode()
        err = ""
        for attempt in range(6):
            req = urllib.request.Request(f"{url}/chat/completions", data=body, method="POST",
                                         headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            try:
                with urllib.request.urlopen(req, timeout=900) as r:
                    d = json.loads(r.read())
                text = d["choices"][0]["message"]["content"] or ""
            except urllib.error.HTTPError as e:
                err = str(e)[:120]
                if e.code == 429:  # the free routes limit a minute's requests: wait, then ask again
                    time.sleep(60 * (attempt + 1))
                continue
            except Exception as e:  # noqa: BLE001 — asked again, then reported
                err = str(e)[:120]
                continue
            (answers / f"{p.stem}.raw").write_text(text, encoding="utf-8")
            got = read(text, keys)
            if got and len(got) == len(keys):
                (answers / f"{p.stem}.json").write_text(json.dumps(got, ensure_ascii=False), encoding="utf-8")
                return f"{p.stem} ok"
            err = f"{len(got or {})} of {len(keys)} schemas in the reply"
        return f"{p.stem} failed: {err}"

    with ThreadPoolExecutor(workers) as pool:
        for line in pool.map(one, todo):
            print(line, flush=True)


def gather(out: Path, teacher: str) -> None:
    answers = out / f"answers-{teacher}"
    done, partial = 0, []
    for raw in sorted(answers.glob("*.raw")):
        keys = keys_of(out / "batches" / f"{raw.stem}.txt")
        got = read(raw.read_text(encoding="utf-8", errors="replace"), keys)
        if got:
            (answers / f"{raw.stem}.json").write_text(json.dumps(got, ensure_ascii=False), encoding="utf-8")
            done += 1
        if not got or len(got) < len(keys):
            partial.append(f"{raw.stem}: {len(got or {})} of {len(keys)}")
    print(f"{done} batches read", *partial, sep="\n")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, out, *rest = sys.argv[1:]
    if cmd == "batches":
        batches(Path(out), Path(rest[0]))
    elif cmd == "ask":
        ask(Path(out), rest[0], rest[1], rest[2], int(rest[3]) if len(rest) > 3 else 8)
    elif cmd == "gather":
        gather(Path(out), rest[0])
