"""Round 1h's lexicon of values: the commonest short text values in
SchemaPile-Perm's columns, each said by DeepSeek — a teacher already — to be
a label a newcomer reads without a lookup, or an opaque code; a breakdown's
labels are then read against it.

    python -I values.py vocab <schemapile-perm.json.gz> <out> [<values>]
    JUDGE_KEY=... python -I values.py ask <out> <endpoint, …/v1> <model> [<at once>]
    python -I values.py gather <out>

vocab writes <out>/values/vocab.json; ask keeps each reply in
<out>/values/batch-<n>.raw; gather writes <out>/values/values.json, a value
to two numbers, and reports its size packed.
"""

from __future__ import annotations

import gzip
import json
import os
import re
import sys
import urllib.request
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

BATCH = 250

ASK = """You are writing a lexicon for a program that draws first-overview charts of unfamiliar databases. Below are
values that occur in text columns. Each may appear as a label on a chart's bar. For each give two numbers from 0
to 1: "reads", how likely a person new to the database understands the value shown as a label without looking
anything up (Paris, active, female, Visa, Monday: high; M and F: middling; L, X, 005, A1: low); and "code", how
likely it is an opaque code, abbreviation or identifier rather than a word or name.

Reply with only one JSON object and no other text: {"values": {"<value>": [reads, code], ...}} — every value below,
numbers rounded to one decimal.

Values:
"""


def vocab(source: Path, out: Path, n: int = 6000) -> None:
    d = json.load(gzip.open(source, "rt", encoding="utf-8"))
    df: Counter = Counter()
    for schema in d.values():
        for t in (schema.get("TABLES") or {}).values():
            for c in (t.get("COLUMNS") or {}).values():
                vals = {str(v).strip().lower() for v in (c.get("VALUES") or []) if isinstance(v, str)}
                df.update(v for v in vals if 0 < len(v) <= 30 and "\n" not in v)
    values = [v for v, _ in df.most_common(n)]
    (out / "values").mkdir(parents=True, exist_ok=True)
    (out / "values" / "vocab.json").write_text(json.dumps({"values": values, "in columns": [df[v] for v in values]}),
                                               encoding="utf-8")
    print(f"{len(values)} values; the last in {df[values[-1]]} columns")


def ask(out: Path, url: str, model: str, workers: int = 24) -> None:
    key = os.environ["JUDGE_KEY"]
    values = json.loads((out / "values" / "vocab.json").read_text(encoding="utf-8"))["values"]
    parts = [values[i:i + BATCH] for i in range(0, len(values), BATCH)]

    def one(k: int) -> str:
        raw = out / "values" / f"batch-{k + 1}.raw"
        if raw.exists() and '"values"' in raw.read_text(encoding="utf-8"):
            return f"batch {k + 1} had"
        body = json.dumps({"model": model, "temperature": 0, "messages": [
            {"role": "user", "content": ASK + "\n".join(json.dumps(v) for v in parts[k])}]}).encode()
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
            if text and '"values"' in text:
                raw.write_text(text, encoding="utf-8")
                return f"batch {k + 1} ok"
            err = "no values in the reply"
        return f"batch {k + 1} failed: {err}"

    with ThreadPoolExecutor(workers) as pool:
        for line in pool.map(one, range(len(parts))):
            print(line, flush=True)


def gather(out: Path) -> None:
    values = json.loads((out / "values" / "vocab.json").read_text(encoding="utf-8"))["values"]
    lex, decoder = {}, json.JSONDecoder()
    for raw in sorted((out / "values").glob("batch-*.raw")):
        text = raw.read_text(encoding="utf-8", errors="replace")
        for m in re.finditer(r"\{\s*\"values\"", text):
            try:
                got = decoder.raw_decode(text, m.start())[0]["values"]
            except (json.JSONDecodeError, KeyError):
                continue
            for v, x in got.items():
                if isinstance(x, list) and len(x) == 2 and all(isinstance(y, (int, float)) for y in x):
                    lex[v.lower()] = [max(0.0, min(1.0, float(y))) for y in x]
    (out / "values" / "values.json").write_text(json.dumps({"values": lex}), encoding="utf-8")
    packed = sum(1 + len(v.encode()) + 2 for v in lex)
    print(f"{len(lex)} values, {sum(1 for v in values if v not in lex)} of the vocabulary missing; about {packed} bytes packed")


def read(labels: list, lex: dict) -> dict[str, float]:
    """A breakdown's labels against the lexicon: how many it knows, and how
    readable and how coded those are."""
    ls = [str(x).strip().lower() for x in labels if x is not None and str(x).strip()]
    known = [lex[x] for x in ls if x in lex]
    nan = float("nan")
    return {"v_known": len(known) / len(ls) if ls else nan,
            "v_reads": sum(k[0] for k in known) / len(known) if known else nan,
            "v_code": sum(k[1] for k in known) / len(known) if known else nan}


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "vocab":
        vocab(Path(sys.argv[2]), Path(sys.argv[3]), int(sys.argv[4]) if len(sys.argv) > 4 else 6000)
    elif sys.argv[1] == "ask":
        ask(Path(sys.argv[2]), sys.argv[3], sys.argv[4], int(sys.argv[5]) if len(sys.argv) > 5 else 24)
    else:
        gather(Path(sys.argv[2]))
