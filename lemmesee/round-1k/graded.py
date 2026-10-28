"""The judges' per-database prompts asked for a chance instead of a verdict:
round 1d's form and round 1d's cut prompts (`gemini.py cut`), with keep as
true or false replaced by p_keep, an integer from 0 to 100. Everything else
in the prompt is as DeepSeek, GLM and Gemini saw it.

    CLIPROXY_KEY=... python -I graded.py ask <look dir> <endpoint, …/v1> <model> <judge> [<at once>]
    python -I graded.py gather <look dir> <judge>

ask keeps each reply as <look dir>/gemini/<prompt>.<judge>.raw; gather writes
<judge>-<n>.json with p_keep as a share and keep as p_keep >= 0.5.
"""

from __future__ import annotations

import json
import os
import re
import sys
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

VERDICT = " 1. keep: would you keep this panel on a first overview of this database? true or false"
CHANCE = (" 1. p_keep: how likely you are to keep this panel on a first overview of this database, "
          "an integer from 0 (surely not) to 100 (surely)")
ASKED = '{"keep": true or false, '
ASKED_CHANCE = '{"p_keep": an integer from 0 to 100, '


def prompt(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    assert VERDICT in text and ASKED in text, path
    return text.replace(VERDICT, CHANCE).replace(ASKED, ASKED_CHANCE)


def ask(look: Path, url: str, model: str, judge: str, workers: int = 4) -> None:
    key = os.environ["CLIPROXY_KEY"]
    todo = [p for p in sorted((look / "gemini").glob("*.txt"))
            if not ((r := p.with_name(f"{p.stem}.{judge}.raw")).exists() and '"panels"' in r.read_text(encoding="utf-8"))]

    def one(p: Path) -> str:
        body = json.dumps({"model": model, "messages": [{"role": "user", "content": prompt(p)}],
                           "temperature": 0}).encode()
        err = ""
        for _ in range(3):
            req = urllib.request.Request(f"{url}/chat/completions", data=body, method="POST",
                                         headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            try:
                with urllib.request.urlopen(req, timeout=600) as r:
                    text = json.loads(r.read())["choices"][0]["message"]["content"]
            except Exception as e:  # noqa: BLE001 — asked again, then reported
                err = str(e)[:120]
                continue
            if text and '"panels"' in text:
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
        out = {}
        for name, a in panels.items():
            try:
                p = min(100.0, max(0.0, float(a["p_keep"]))) / 100
            except (KeyError, TypeError, ValueError):
                continue
            out[name] = {"keep": p >= 0.5, "p_keep": round(p, 4), "meaning": a.get("meaning"), "wrong": a.get("wrong")}
        by_packet.setdefault(raw.name.split(".")[0].rsplit("-", 1)[0], {}).update(out)
    for packet, panels in by_packet.items():
        (look / f"{judge}-{packet.split('-')[1]}.json").write_text(json.dumps({"panels": panels}, ensure_ascii=False),
                                                                   encoding="utf-8")
    print({p: len(v) for p, v in sorted(by_packet.items())}, "unreadable:", bad)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, look, *rest = sys.argv[1:]
    if cmd == "ask":
        ask(Path(look), rest[0], rest[1], rest[2], int(rest[3]) if len(rest) > 3 else 4)
    else:
        gather(Path(look), rest[0])
