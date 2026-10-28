"""Round 1g's third pass: what the teachers know of words, written down once
as a lexicon the look can carry. The words are the commonest in
SchemaPile-Perm's table and column names — not the judged databases', so a
word the look meets and the lexicon lacks stays unknown, as in use — and a
language model that teaches the look anyway, DeepSeek, says of each how
likely it names an identifier, money, a count, a sum that means something,
a record's technical time, a business event's time, a good grouping, an
opaque code, and how much a newcomer would care.

    python -I lexicon.py vocab <schemapile-perm.json.gz> <out> [<words>]
    JUDGE_KEY=... python -I lexicon.py ask <out> <endpoint, …/v1> <model> [<at once>]
    python -I lexicon.py gather <out>

vocab writes <out>/lexicon/vocab.json; ask keeps each reply in
<out>/lexicon/batch-<n>.raw; gather writes <out>/lexicon/lexicon.json, a
word to nine numbers, and reports its size packed.
"""

from __future__ import annotations

import gzip
import json
import re
import struct
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE.parent / "round-1c"))
from aq import singular, tokens  # noqa: E402

PROPS = ("id", "money", "count", "sum_ok", "audit_time", "event_time", "good_group", "cryptic", "importance")
BATCH = 200

ASK = """You are writing a lexicon for a program that suggests first-overview dashboards for unfamiliar
databases. Below are words that occur in table and column names (split from snake_case and camelCase,
lower case). For each word give nine numbers from 0 to 1, how likely a column or table whose name holds the
word is each of these:

 id: an identifier, key, code or reference number (id, uuid, sku, zip, phone, number as in card number)
 money: an amount of money (price, cost, revenue, salary, fee, balance, amount)
 count: a count or quantity of things (qty, population, tonnage, votes, views, stock)
 sum_ok: a value whose sum over rows means something (amount yes; price, rating, age, temperature no)
 audit_time: a record's technical time (updated, modified, synced, inserted, last_update)
 event_time: the time a real-world event happened (ordered, departure, birth, paid, shipped)
 good_group: a good category to group rows by on an overview (status, type, country, genre, gender, region)
 cryptic: an opaque code or abbreviation whose values mean little without a lookup (cd, flg, lid, loc, x)
 importance: how much a person new to such a database would care about it on a first overview

A word that is not a word of its own (a fragment, a number, a name) gets low numbers where nothing fits.
Reply with only one JSON object and no other text: {"words": {"<word>": [id, money, count, sum_ok,
audit_time, event_time, good_group, cryptic, importance], ...}} — every word below, numbers rounded to one
decimal.

Words:
"""


def vocab(source: Path, out: Path, n: int = 4000) -> None:
    """The words in the most schemas, from table and column names, as
    round 1c's question splits them."""
    d = json.load(gzip.open(source, "rt", encoding="utf-8"))
    df: Counter = Counter()
    for schema in d.values():
        seen = set()
        for tname, t in (schema.get("TABLES") or {}).items():
            for w in tokens(tname):
                seen.add(singular(w))
                seen.add(w)
            for cname in (t.get("COLUMNS") or {}):
                seen.update(tokens(cname))
        df.update(w for w in seen if re.fullmatch(r"[a-z][a-z]+", w))
    words = [w for w, _ in df.most_common(n)]
    (out / "lexicon").mkdir(parents=True, exist_ok=True)
    (out / "lexicon" / "vocab.json").write_text(json.dumps({"schemas": len(d), "words": words,
                                                            "in schemas": [df[w] for w in words]}), encoding="utf-8")
    print(f"{len(words)} words from {len(d)} schemas; the last in {df[words[-1]]} schemas")


def ask(out: Path, url: str, model: str, workers: int = 20) -> None:
    import os
    import urllib.request
    from concurrent.futures import ThreadPoolExecutor
    key = os.environ["JUDGE_KEY"]
    words = json.loads((out / "lexicon" / "vocab.json").read_text(encoding="utf-8"))["words"]
    batches = [words[i:i + BATCH] for i in range(0, len(words), BATCH)]

    def one(k: int) -> str:
        raw = out / "lexicon" / f"batch-{k + 1}.raw"
        if raw.exists() and '"words"' in raw.read_text(encoding="utf-8"):
            return f"batch {k + 1} had"
        body = json.dumps({"model": model, "temperature": 0,
                           "messages": [{"role": "user", "content": ASK + "\n".join(batches[k])}]}).encode()
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
            if text and '"words"' in text:
                raw.write_text(text, encoding="utf-8")
                return f"batch {k + 1} ok"
            err = "no words in the reply"
        return f"batch {k + 1} failed: {err}"

    with ThreadPoolExecutor(workers) as pool:
        for line in pool.map(one, range(len(batches))):
            print(line, flush=True)


def gather(out: Path) -> None:
    words = json.loads((out / "lexicon" / "vocab.json").read_text(encoding="utf-8"))["words"]
    lex, decoder = {}, json.JSONDecoder()
    for raw in sorted((out / "lexicon").glob("batch-*.raw")):
        text = raw.read_text(encoding="utf-8", errors="replace")
        for m in re.finditer(r"\{\s*\"words\"", text):
            try:
                got = decoder.raw_decode(text, m.start())[0]["words"]
            except (json.JSONDecodeError, KeyError):
                continue
            for w, v in got.items():
                if isinstance(v, list) and len(v) == len(PROPS) and all(isinstance(x, (int, float)) for x in v):
                    lex[w.lower()] = [max(0.0, min(1.0, float(x))) for x in v]
    missing = [w for w in words if w not in lex]
    (out / "lexicon" / "lexicon.json").write_text(json.dumps({"props": PROPS, "words": lex}), encoding="utf-8")
    # as it would ship: each word, a byte of length, its letters, nine bytes of tenths
    packed = sum(1 + len(w.encode()) + len(PROPS) for w in lex)
    print(f"{len(lex)} words, {len(missing)} of the vocabulary missing; about {packed} bytes packed "
          f"({struct.calcsize('<B')} byte a length)")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "vocab":
        vocab(Path(sys.argv[2]), Path(sys.argv[3]), int(sys.argv[4]) if len(sys.argv) > 4 else 4000)
    elif cmd == "ask":
        ask(Path(sys.argv[2]), sys.argv[3], sys.argv[4], int(sys.argv[5]) if len(sys.argv) > 5 else 20)
    else:
        gather(Path(sys.argv[2]))
