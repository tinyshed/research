"""Jev as a teacher of the ranker: it cannot write a schema's ten questions,
so it is asked of the generator's own. For each of round 1c's 480 schemas,
the fifty best questions by round 1c's ranker, each asked alone with the
schema as a teacher reads it: would the analyst of what the database serves
put it on the screen they open every week? Its first five by that chance
stand as a teacher's first five, its next five as the rest of its ten.

    JUDGE_KEY=... python -I jevrank.py ask <teacher dir> <round 1c learn> [<per schema>] [<at once>]

writes <teacher dir>/answers-jev/<schema key>.json, {title: chance}, and prints
what it cost; a schema already answered is not asked again.
"""

from __future__ import annotations

import json
import os
import sys
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT / "round-1c"))
import learn  # noqa: E402
import rank  # noqa: E402
from aq import generate, identity, load  # noqa: E402

URL = "https://openrouter.ai/api/alpha/decisions"
MODEL = "typesafe/jev-1.13"
QUESTION = {
    "type": "noul",
    "instructions": "Would the analyst responsible for what this database serves — a product, a shop, a service, a "
                    "league, a register, a study — put this question on the screen they open every week to see how "
                    "it is going?",
    "criteria": {
        "true": "It tells how the business or project is doing: who and what is active and how much, what it earns "
                "and costs, what succeeds and fails, how that changes.",
        "false": "It is an inventory of a table, technical bookkeeping, or something nobody would act on.",
    },
}


def candidates(snap: dict, model, table, k: int) -> list[str]:
    """The k best questions by round 1c's ranker, one title an identity."""
    schema = load(snap, reads_n=False)
    qs = [q for q in generate(schema) if rank.veto(schema, q) is None]
    if not qs:
        return []
    row = {"features": [rank.features(schema, q) for q in qs], "slots": [rank.slots(schema, q) for q in qs]}
    X, _, _ = learn.matrix_free(row, model.feature_name(), table)
    out, seen = [], set()
    for i in np.argsort(-model.predict(X), kind="stable"):
        ident = identity(schema, qs[i])
        if ident not in seen:
            seen.add(ident)
            out.append(qs[i].title(schema))
        if len(out) == k:
            break
    return out


def ask(tdir: Path, learn_dir: Path, k: int = 50, at_once: int = 40) -> None:
    import lightgbm as lgb
    key = os.environ["JUDGE_KEY"]
    model = lgb.Booster(model_file=str(learn_dir / "ranker.txt"))
    table = json.loads((learn_dir / "words.json").read_text(encoding="utf-8"))
    out = tdir / "answers-jev"
    out.mkdir(exist_ok=True)
    items = []
    for line in (tdir / "schemas.jsonl").open(encoding="utf-8"):
        snap = json.loads(line)
        if (out / f"{snap['key']}.json").exists():
            continue
        text = (tdir / "text" / f"{snap['key']}.txt").read_text(encoding="utf-8")
        items += [(snap["key"], t, text) for t in candidates(snap, model, table, k)]
    print(f"{len(items)} questions to ask", flush=True)

    def one(item):
        skey, title, text = item
        body = json.dumps({"model": MODEL, "state": {"database tables": text, "question": title},
                           "questions": {"keep": QUESTION}}).encode()
        for _ in range(3):
            req = urllib.request.Request(URL, data=body, method="POST",
                                         headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            try:
                with urllib.request.urlopen(req, timeout=120) as r:
                    d = json.loads(r.read())
                return skey, title, float(d["answers"]["keep"]["noul"]), d.get("usage", {}).get("cost", 0.0)
            except Exception:  # noqa: BLE001 — asked again, then left out
                continue
        return skey, title, None, 0.0

    by_schema: dict[str, dict] = {}
    cost, missing = 0.0, 0
    with ThreadPoolExecutor(at_once) as pool:
        for skey, title, p, c in pool.map(one, items):
            cost += c or 0.0
            if p is None:
                missing += 1
                continue
            by_schema.setdefault(skey, {})[title] = round(p, 4)
    for skey, answers in by_schema.items():
        (out / f"{skey}.json").write_text(json.dumps(answers, ensure_ascii=False), encoding="utf-8")
    print(f"{len(by_schema)} schemas, {sum(len(v) for v in by_schema.values())} questions answered, {missing} not; "
          f"cost ${cost:.4f}", flush=True)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, *a = sys.argv[1:]
    if cmd == "ask":
        ask(Path(a[0]), Path(a[1]), int(a[2]) if len(a) > 2 else 50, int(a[3]) if len(a) > 3 else 40)
