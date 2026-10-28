"""Jev in the fast format: round 1d's keep question alone, of the compact
result (local.compact), each panel alone on OpenRouter's decisions endpoint.
The look learns keep only, so meaning is not asked, and the input is half.

    JUDGE_KEY=... python -I jevfast.py <look dir> [<at once>]

writes <look dir>/jevfast-<n>.json and prints what it cost.
"""

from __future__ import annotations

import json
import os
import sys
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import local  # noqa: E402  (its jev module and compact)

jev = local.jev


PRODUCT = {
    "type": "noul",
    "instructions": "Would the analyst responsible for what this database serves — a product, a shop, a service, a "
                    "league, a register, a study — keep this panel on the screen they open every week to see how it is "
                    "going?",
    "criteria": {
        "true": "It tells how the business or project is doing: who and what is active and how much, what it earns "
                "and costs, what succeeds and fails, how that changes.",
        "false": "It is an inventory of a table, technical bookkeeping, something nobody would act on, or its result "
                 "is a single bar, empty, a column of blanks or meaningless.",
    },
}


def ask(look: Path, at_once: int = 64, product: bool = False) -> None:
    key = os.environ["JUDGE_KEY"]
    items = list(jev.panels(look))
    questions = {"keep": PRODUCT if product else jev.QUESTIONS["keep"]}
    name = "pmjev" if product else "jevfast"

    def one(item):
        n, name, state = item
        body = json.dumps({"model": jev.MODEL, "state": local.compact(state), "questions": questions}).encode()
        for _ in range(3):
            req = urllib.request.Request(jev.URL, data=body, method="POST",
                                         headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            try:
                with urllib.request.urlopen(req, timeout=120) as r:
                    d = json.loads(r.read())
                return n, name, float(d["answers"]["keep"]["noul"]), d.get("usage", {}).get("cost", 0.0)
            except Exception:  # noqa: BLE001 — asked again, then left out
                continue
        return n, name, None, 0.0

    by_packet: dict[str, dict] = {}
    cost, missing = 0.0, 0
    with ThreadPoolExecutor(at_once) as pool:
        for n, name, keep, c in pool.map(one, items):
            cost += c or 0.0
            if keep is None:
                missing += 1
                continue
            by_packet.setdefault(n, {})[name] = {"keep": keep >= 0.5, "wrong": None, "p_keep": round(keep, 4)}
    for n, ans in by_packet.items():
        (look / f"{name}-{n}.json").write_text(json.dumps({"panels": ans}, ensure_ascii=False), encoding="utf-8")
    print(f"{look}: {sum(len(v) for v in by_packet.values())} panels answered, {missing} not; cost ${cost:.4f}", flush=True)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    args = [a for a in sys.argv[1:] if a != "--product"]
    ask(Path(args[0]), int(args[1]) if len(args) > 1 else 64, "--product" in sys.argv)
