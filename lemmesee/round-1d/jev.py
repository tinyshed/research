"""TypeSafe's Jev 1.13 as a judge: a model of typed decisions on OpenRouter's
decisions endpoint, asked of each panel alone two yes-or-no questions, keep
and meaning, and answering each with a probability. Cheap enough to label
any number of panels; measured here against the other judges first.

    JUDGE_KEY=... python -I jev.py ask <look dir> [<at once>]

reads the packets' panels and writes <look dir>/jev-<n>.json, with each
probability beside the verdict it rounds to.
"""

from __future__ import annotations

import json
import os
import re
import sys
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

URL = "https://openrouter.ai/api/alpha/decisions"
MODEL = "typesafe/jev-1.13"
QUESTIONS = {
    "keep": {
        "type": "noul",
        "instructions": "Would a person new to this database keep this panel on a first overview dashboard of it?",
        "criteria": {
            "true": "It answers a question a newcomer would want answered first, and its result is worth a chart.",
            "false": "It is minor or obscure, or its result is a single bar, empty, a column of blanks, or meaningless.",
        },
    },
    "meaning": {
        "type": "noul",
        "instructions": "Does the picture mean what it seems to?",
        "criteria": {
            "true": "The numbers shown are what the title says, and adding or comparing them makes sense.",
            "false": "It sums a price or a level with no quantity, mixes units or currencies, counts rows twice through a join, groups by an identifier or by blanks, or misleads another way.",
        },
    },
}


def panels(look: Path):
    """Every panel of every packet: its packet, its name, and what a judge saw."""
    for packet in sorted(look.glob("packet-*.txt"), key=lambda p: int(p.stem.split("-")[1])):
        n = packet.stem.split("-")[1]
        text = packet.read_text(encoding="utf-8")
        for section in re.split(r"\n(?==== database: )", text)[1:]:
            head, *items = re.split(r"\n  panel ", section)
            database = head.split("===")[1].replace("database:", "").strip()
            note = head.split("===", 2)[2].strip()
            for item in items:
                name, rest = item.split("\n", 1)
                title = re.search(r"title: (.*)", rest).group(1)
                rows = rest.split("\n", 2)[2] if rest.count("\n") >= 2 else ""
                yield n, name.strip(), {"database": database, "about": note, "panel title": title,
                                        "result": rows.strip()}


def ask(look: Path, at_once: int = 64) -> None:
    key = os.environ["JUDGE_KEY"]
    items = list(panels(look))

    def one(item):
        n, name, state = item
        body = json.dumps({"model": MODEL, "state": state, "questions": QUESTIONS}).encode()
        for _ in range(3):
            req = urllib.request.Request(URL, data=body, method="POST",
                                         headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            try:
                with urllib.request.urlopen(req, timeout=120) as r:
                    d = json.loads(r.read())
                a = d["answers"]
                return n, name, float(a["keep"]["noul"]), float(a["meaning"]["noul"]), d.get("usage", {}).get("cost", 0.0)
            except Exception:  # noqa: BLE001 — asked again, then left out
                continue
        return n, name, None, None, 0.0

    by_packet: dict[str, dict] = {}
    cost, missing = 0.0, 0
    with ThreadPoolExecutor(at_once) as pool:
        for n, name, keep, meaning, c in pool.map(one, items):
            cost += c or 0.0
            if keep is None:
                missing += 1
                continue
            by_packet.setdefault(n, {})[name] = {"keep": keep >= 0.5, "meaning": "yes" if meaning >= 0.5 else "no",
                                                 "wrong": None, "p_keep": round(keep, 4), "p_meaning": round(meaning, 4)}
    for n, ans in by_packet.items():
        (look / f"jev-{n}.json").write_text(json.dumps({"panels": ans}, ensure_ascii=False), encoding="utf-8")
    print(f"{sum(len(v) for v in by_packet.values())} panels answered, {missing} not; cost ${cost:.4f}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    ask(Path(sys.argv[2]), int(sys.argv[3]) if len(sys.argv) > 3 else 64)
