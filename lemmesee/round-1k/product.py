"""A judge that thinks of the business: the same panels, packets and replies
as round 1d's judges, with the role changed from a person new to the
database to the analyst of what the database serves, who builds the screen
they will open every week. Judges named pmgemini and pmsonnet, so that no
earlier judge's files share their prefix.

    CLIPROXY_KEY=... python -I product.py ask <look dir> <endpoint, …/v1> <model> <judge> [<at once>]
    python -I product.py gather <look dir> <judge>
    python -I product.py packets <look dir>          # <look dir>/pm/packet-<n>.txt for agents

ask reads round 1d's per-database prompts (`gemini.py cut`) with their form
replaced, and keeps each reply as <look dir>/gemini/<prompt>.<judge>.raw;
gather writes <judge>-<n>.json with keep, meaning, wrong and instead.
"""

from __future__ import annotations

import json
import os
import re
import sys
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

FORM = """You are the analyst responsible for what each database below serves — a product, a shop, a service, a league, a
register, a study — and you must build the first screen of a dashboard you will open every week to see how it is
going: who and what is active and how much, what it earns and what it costs, what succeeds and what fails, and how all
of that changes. You did not make the panels below and are not told what made them.

For every panel answer:
 1. keep: would this panel earn a place on that weekly screen? true or false. Keep what tells you how the business or
    project is doing. Drop an inventory of a table (rows by an internal code or by an id), technical bookkeeping
    (import times, update stamps, logs of the machinery) and what you would never act on.
 2. meaning: does the picture mean what it seems to? "yes", "no", or "cannot tell"
 3. wrong: if not, what is wrong, in a few words; else null
 4. instead: if you would not keep it, what you would put on the screen in its place, in a few words; else null
A panel is shown by its title, which says what it computes, and the first rows of its result as they came back from
the database. A single bar, an empty result or a column of blanks is not worth a chart."""

ASK = ('Reply with only one JSON object and no other text: {"panels": {"<panel id>": {"keep": true or false, '
       '"meaning": "yes" or "no" or "cannot tell", "wrong": a few words or null, "instead": a few words or null}, ...}} '
       '— every panel id below must be answered. Do not read, search or open any file, and run no command.')

WRITE = ("Do not read, search or open any file other than this one, and run no command; when done, write your answers "
         "with the Write tool, once, to the path given to you.\n\n"
         'Write one JSON object: {"panels": {"<panel id>": {"keep": bool, "meaning": "...", "wrong": str|null, '
         '"instead": str|null}, ...}} — every panel id below must be answered.')


CHANCE = os.environ.get("PRODUCT_CHANCE") == "1"  # keep asked as a chance from 0 to 100, for a teacher


def prompt(path: Path) -> str:
    """A cut prompt with the form and the request replaced, its databases kept."""
    text = path.read_text(encoding="utf-8")
    at = text.index("\n=== database: ")
    form, ask_ = FORM, ASK
    if CHANCE:
        form = form.replace("keep: would this panel earn a place on that weekly screen? true or false.",
                            "p_keep: how likely this panel is to earn a place on that weekly screen, an integer from "
                            "0 (surely not) to 100 (surely).")
        ask_ = ask_.replace('"keep": true or false', '"p_keep": an integer from 0 to 100')
    return f"{form}\n\n{ask_}\n\n{text[at:].strip()}\n"


def ask(look: Path, url: str, model: str, judge: str, workers: int = 8) -> None:
    key = os.environ["CLIPROXY_KEY"]
    todo = [p for p in sorted((look / "gemini").glob("packet-*-*.txt"))
            if not ((r := p.with_name(f"{p.stem}.{judge}.raw")).exists() and '"panels"' in r.read_text(encoding="utf-8"))]

    def one(p: Path) -> str:
        body = json.dumps({"model": model, "messages": [{"role": "user", "content": prompt(p)}], "temperature": 0}).encode()
        err = ""
        for _ in range(3):
            req = urllib.request.Request(f"{url}/chat/completions", data=body, method="POST",
                                         headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            try:
                with urllib.request.urlopen(req, timeout=600) as r:
                    text = json.loads(r.read())["choices"][0]["message"]["content"] or ""
            except Exception as e:  # noqa: BLE001 — asked again, then reported
                err = str(e)[:120]
                continue
            if '"panels"' in text:
                p.with_name(f"{p.stem}.{judge}.raw").write_text(text, encoding="utf-8")
                return f"{p.name} ok"
            err = "no panels in the reply"
        return f"{p.name} failed: {err}"

    with ThreadPoolExecutor(workers) as pool:
        for line in pool.map(one, todo):
            print(line, flush=True)


def gather(look: Path, judge: str) -> None:
    by_packet: dict[str, dict] = {}
    bad = []
    for raw in sorted((look / "gemini").glob(f"*.{judge}.raw")):
        text = raw.read_text(encoding="utf-8", errors="replace")
        panels = None
        for m in re.finditer(r"\{\s*\"panels\"", text):  # the last whole answer
            try:
                panels = json.JSONDecoder().raw_decode(text, m.start())[0]["panels"]
            except (json.JSONDecodeError, KeyError):
                continue
        if panels is None:
            bad.append(raw.name)
            continue
        for a in panels.values():
            # a chance, as a teacher answers, is kept as a share beside the verdict it rounds to
            if isinstance(a, dict) and "p_keep" in a:
                try:
                    a["p_keep"] = min(100.0, max(0.0, float(a["p_keep"]))) / 100
                    a["keep"] = a["p_keep"] >= 0.5
                except (TypeError, ValueError):
                    pass
        by_packet.setdefault(raw.name.split(".")[0].rsplit("-", 1)[0], {}).update(panels)
    for packet, panels in by_packet.items():
        (look / f"{judge}-{packet.split('-')[1]}.json").write_text(json.dumps({"panels": panels}, ensure_ascii=False),
                                                                   encoding="utf-8")
    print({p: len(v) for p, v in sorted(by_packet.items())}, "unreadable:", bad)


def packets(look: Path) -> None:
    out = look / "pm"
    out.mkdir(exist_ok=True)
    n = 0
    for p in sorted(look.glob("packet-*.txt")):
        text = p.read_text(encoding="utf-8")
        at = text.index("\n=== database: ")
        (out / p.name).write_text(f"{FORM}\n\n{WRITE}\n\n{text[at:].strip()}\n", encoding="utf-8")
        n += 1
    print(f"{n} packets in {out}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, look, *rest = sys.argv[1:]
    if cmd == "ask":
        ask(Path(look), rest[0], rest[1], rest[2], int(rest[3]) if len(rest) > 3 else 8)
    elif cmd == "gather":
        gather(Path(look), rest[0])
    elif cmd == "packets":
        packets(Path(look))
