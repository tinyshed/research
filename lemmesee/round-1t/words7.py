"""Round 1t's version 7 cues: a name read as words, run-together words split
and shortened words expanded, before it is compared with its target.

Two tables travel with the model:

- `vocab.txt`, the 5 363 words of at least two letters that name SchemaPile's
  training tables and columns 20 times or more, with their counts; a word
  run together with others (`custref`, `orderdate`) is split into those
  words where they cover it, by the split whose words are the most frequent
  (the product of their frequencies; a letter left over costs as much as a
  word seen once);
- `abbrev.json`, 999 shortened words of SchemaPile's training names (2 to 6
  letters, seen 15 times or more) expanded by Claude Haiku 5.5, asked as
  Claude Code agents on 8 November 2026 to give each word's likely
  expansions: `cust` customer, `qty` quantity, `dept` department.

A name's words are its tokens, each split, each expanded (an expansion of
several words gives them all), singular; a key word (`id`, `ref`, ...) is
left out. The cues compare the column's words with the target table's and
with its key's.
"""

import json
import math
from functools import lru_cache
from pathlib import Path

from train import singular, tokens

HERE = Path(__file__).resolve().parent
KEY_WORDS = {"id", "key", "code", "no", "num", "nr", "fk", "ref", "pk", "uid", "link", "ptr"}

VOCAB = {}
for line in open(HERE / "vocab.txt", encoding="utf-8"):
    w, n = line.split()
    VOCAB[w] = int(n)
TOTAL = sum(VOCAB.values())
COST = {w: -math.log(n / TOTAL) for w, n in VOCAB.items()}
UNKNOWN = -math.log(1 / TOTAL)  # a letter no word covers
LONGEST = max(len(w) for w in VOCAB)
ABBREV = json.load(open(HERE / "abbrev.json", encoding="utf-8"))


@lru_cache(maxsize=200_000)
def split(token: str) -> tuple:
    """The token as vocabulary words, or itself where it is one or no split is cheaper."""
    n = len(token)
    if n < 5 or token in VOCAB:
        return (token,)
    best = [0.0] + [math.inf] * n
    back = [0] * (n + 1)
    for i in range(1, n + 1):
        c = best[i - 1] + UNKNOWN
        if c < best[i]:
            best[i], back[i] = c, i - 1
        for j in range(max(0, i - LONGEST), i - 1):
            w = token[j:i]
            if w in COST and best[j] + COST[w] < best[i]:
                best[i], back[i] = best[j] + COST[w], j
    parts, i = [], n
    while i > 0:
        parts.append(token[back[i]:i])
        i = back[i]
    parts.reverse()
    if any(len(p) == 1 for p in parts):
        return (token,)
    return tuple(parts)


@lru_cache(maxsize=200_000)
def words(name: str) -> frozenset:
    out = set()
    for tok in tokens(name):
        for w in split(tok):
            if w in KEY_WORDS:
                continue
            out.add(singular(w))
            for e in ABBREV.get(w, ()):
                for x in e.split():
                    if len(x) >= 2 and x not in KEY_WORDS:
                        out.add(singular(x))
    return frozenset(out)


def add_cues(f: dict, column: dict, target: dict, key: str) -> None:
    cw = words(column["name"])
    tw = words(target["name"])
    kw = words(key)
    f["v7 words shared with the table"] = min(len(cw & tw), 3) / 3
    f["v7 words shared with the key"] = min(len(cw & kw), 3) / 3
    f["v7 table's words all in the column"] = float(bool(tw) and tw <= cw)
    f["v7 key's words all in the column"] = float(bool(kw) and kw <= cw)
    f["v7 column words"] = min(len(cw), 6) / 6


# Version 8: `related.json`, for 4 000 of the vocabulary's words, the
# entities Claude Haiku 5.5 (Claude Code agents, 8 November 2026) says a
# column so named most likely refers to (author: user, person; pilot:
# driver), singular. A column's words, so mapped, against its target's.
RELATED = {}
if (HERE / "related.json").exists():
    RELATED = {k: frozenset(singular(x) for x in v) for k, v in
               json.load(open(HERE / "related.json", encoding="utf-8")).items()}


def add_related(f: dict, column: dict, target: dict) -> None:
    cw = words(column["name"])
    tw = words(target["name"])
    hits = sum(1 for w in cw for r in RELATED.get(w, ()) if r in tw)
    f["v8 the column's entity names the table"] = float(hits > 0)
    f["v8 entity matches"] = min(hits, 3) / 3
    f["v8 the table's entity names the column"] = float(any(r in cw for w in tw for r in RELATED.get(w, ())))
