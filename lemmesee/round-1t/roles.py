"""Round 1t's version 9 cues: a column named by its role, read against the
pair's target; added on 9 November 2026 for the mended fit.

`roles.json`, written once, offline, by Claude Haiku 5.5 as Claude Code
agents wrote the earlier lexicons: words naming a reference to a row of the
same table (`jefe`, `chefe`, `boss`, `parent`), words naming the person who
created or owns a row (`criador`, `createdby`, `owner`), and words naming a
table of people (`usuario`, `funcionario`, `employee`), across languages.
The style set's errors showed `codigo_jefe` into `empleado` and `criador`
into `profissional` won by another target. Four cues:

- the column carries a self word and the pair points into its own table;
- it carries a self word and the pair points elsewhere;
- it carries an actor word and the target's name carries a person word;
- it carries an actor word and the target's does not.

A word is carried when it is one of the name's tokens, or, four letters or
longer, the name run together begins or ends with it.
"""

import json
import re
from functools import lru_cache
from pathlib import Path

HERE = Path(__file__).resolve().parent
LEX = json.load(open(HERE / "roles.json", encoding="utf-8")) if (HERE / "roles.json").exists() else {}
SELF = frozenset(LEX.get("self", ()))
ACTOR = frozenset(LEX.get("actor", ()))
PERSON = frozenset(LEX.get("person", ()))


def _tokens(name: str) -> list[str]:
    s = re.sub(r"([a-z0-9])([A-Z])", r"\1 \2", name)
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1 \2", s)
    return [t for t in re.split(r"[^A-Za-z]+", s.lower()) if t]


@lru_cache(maxsize=400_000)
def carries(name: str, kind: str) -> bool:
    words = {"self": SELF, "actor": ACTOR, "person": PERSON}[kind]
    toks = _tokens(name)
    if any(t in words for t in toks):
        return True
    run = "".join(toks)
    return any(len(w) >= 4 and (run.startswith(w) or run.endswith(w)) for w in words)


def add_cues(f: dict, column: dict, target: dict, table: dict) -> None:
    c = column["name"]
    own = target["name"] == table["name"]
    if carries(c, "self"):
        f["v9 self word, own table"] = float(own)
        f["v9 self word, another table"] = float(not own)
    if carries(c, "actor"):
        person = carries(target["name"], "person")
        f["v9 actor word, a table of people"] = float(person)
        f["v9 actor word, another table"] = float(not person)
