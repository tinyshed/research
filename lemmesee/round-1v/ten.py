"""Round 1v's version 10 cues: a name read against the whole schema.

Version 9's pairs (round 1t's six.py with words7 and roles), each with cues
that say how a name sits among the schema's tables, none from a list of
styles:

- **homes**: how many tables have the column's name as their one-column
  key, how many carry it as one column of a key of several, and whether the
  pair's target is the only table whose whole key it is. Version 6 lets a
  name point at every table that carries it in a key, and a name four tables
  key by had no way to say which of them is its home;
- **the target carries the name outside its key**: a table that itself has a
  column so named, not in its key, is another table pointing the same way;
- **a table's name less the schema's affixes**: a first or a last word a
  tenth of the schema's tables share (three at least), or letters run
  together that three tenths of them start with, taken off; the column's
  stem equal to that core, how many tables have the same core, and the core
  inside the column's name;
- **how many tables hold the stem, and which table inside the column is the
  longest**: `stem in target` and `target in stem` say a name is inside
  another, not how many tables the same holds for;
- **the column's stem less the schema's own start**: letters 15 % of the
  schema's columns begin with (`cod` of `codusu`, `codper`, `codemp`), taken
  off, the rest read against the target's name and its key.
"""

import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1t"))

import cues  # noqa: E402  (round 1t's: the earlier rounds on the path, the trigrams sorted)

import numpy as np  # noqa: E402
import scipy.sparse as sp  # noqa: E402
from sklearn.feature_extraction import FeatureHasher  # noqa: E402

from functools import lru_cache  # noqa: E402

import graph  # noqa: E402
import keys  # noqa: E402
import roles  # noqa: E402
import six  # noqa: E402
import style  # noqa: E402
import words7  # noqa: E402
from train import HASHED, norm, singular, tokens  # noqa: E402

# where a pair's version 10 fields sit in its meta, after round 1p's key column
HOME, CARRIED, VIEW, CORE, INSIDE = 15, 16, 17, 18, 19
VIEWS = {"pk": 0, "pseudo": 1, "part": 2, "unique": 3}


def common_start(names: list[str], share: float, shortest: int = 2, longest: int = 8) -> str:
    """The longest start at least `share` of the names carry, each name
    three letters longer than it, or ''."""
    names = [n for n in names if n]
    if len(names) < 4:
        return ""
    best = ""
    for k in range(shortest, longest + 1):
        c = Counter(n[:k] for n in names if len(n) >= k + 3)
        if not c:
            break
        start, n = c.most_common(1)[0]
        if n >= max(3, share * len(names)):
            best = start
        else:
            break
    return best


def schema_info(s: dict) -> dict:
    homes, parts, unkeyed = Counter(), Counter(), defaultdict(set)
    for t in s["tables"]:
        pk = [norm(p) for p in t["pk"]]
        if len(pk) == 1:
            homes[pk[0]] += 1
        else:
            for p in pk:
                parts[p] += 1
        for c in t["columns"]:
            n = norm(c["name"])
            if n not in pk:
                unkeyed[n].add(t["name"])
    toks = {t["name"]: tokens(t["name"]) for t in s["tables"]}
    need = max(3, 0.1 * len(toks))
    first = Counter(tk[0] for tk in toks.values() if len(tk) > 1)
    last = Counter(tk[-1] for tk in toks.values() if len(tk) > 1)
    pre = {w for w, n in first.items() if n >= need}
    suf = {w for w, n in last.items() if n >= need}
    run = common_start([norm(n) for n in toks], share=0.3)
    core = {}
    for name, tk in toks.items():
        t2 = list(tk)
        if len(t2) > 1 and t2[0] in pre:
            t2 = t2[1:]
        if len(t2) > 1 and t2[-1] in suf:
            t2 = t2[:-1]
        c = "".join(t2)
        if len(t2) == len(tk) and run and c.startswith(run) and len(c) - len(run) >= 3:
            c = c[len(run):]
        core[name] = singular(c)
    plain = {name: singular(norm(name)) for name in toks}
    cols = [norm(c["name"]) for t in s["tables"] for c in t["columns"]]
    return {"homes": homes, "parts": parts, "unkeyed": unkeyed, "core": core, "plain": plain,
            "cores": Counter(core.values()), "start": common_start(cols, share=0.15),
            "holding": {}, "inside": {}}


def holding(stem: str, info: dict) -> frozenset:
    """The tables whose name, or core, holds the stem."""
    got = info["holding"].get(stem)
    if got is None:
        got = frozenset(n for n, p in info["plain"].items() if stem in p or stem in info["core"][n]) \
            if len(stem) >= 3 else frozenset()
        info["holding"][stem] = got
    return got


def inside(cname: str, info: dict) -> dict:
    """table: the letters of its name, or of its core, that lie inside the column's name."""
    got = info["inside"].get(cname)
    if got is None:
        got = {}
        for n, p in info["plain"].items():
            c = info["core"][n]
            m = max(len(p) if len(p) >= 3 and p in cname else 0, len(c) if len(c) >= 3 and c in cname else 0)
            if m:
                got[n] = m
        info["inside"][cname] = got
    return got


def add_cues(f: dict, column: dict, target: dict, key: str, info: dict) -> tuple:
    c = norm(column["name"])
    k = norm(key)
    g = target["name"]
    view = next((v for v in VIEWS if f.get("v6 view:" + v)), "pk")
    homes = info["homes"][c]
    home = float(c == k and view == "pk" and homes == 1)
    carried = float(g in info["unkeyed"].get(c, ()))
    f["v10 home"] = home
    f["v10 homes"] = min(homes, 4) / 4
    f["v10 key part elsewhere"] = min(info["parts"][c], 4) / 4
    f["v10 target carries the name unkeyed"] = carried
    stem = cues.stem5(column["name"], info["v5"]) or c
    core = info["core"][g]
    is_core = float(len(core) >= 2 and singular(stem) == core)
    f["v10 stem is the core"] = is_core
    f["v10 tables of this core"] = 1 / info["cores"][core]
    hold = holding(singular(stem), info)
    f["v10 tables holding the stem"] = (1 / len(hold)) if g in hold else 0.0
    ins = inside(c, info)
    longest = max(ins.values(), default=0)
    f["v10 table inside the column"] = (1 / len(ins)) if g in ins else 0.0
    is_inside = float(g in ins and ins[g] == longest)
    f["v10 the longest table inside the column"] = is_inside
    start = info["start"]
    if start and c.startswith(start) and len(c) - len(start) >= 2:
        rest = c[len(start):]
        rs = cues.strip_end(rest, info["v5"]["col_end"])
        p = norm(target["name"])
        f["v10 less the start: shortens the table"] = float(
            cues.abbreviates(rs, p) or cues.abbreviates(rs, core) or rs in (core, singular(p)))
        f["v10 less the start: starts the table"] = cues.lcp(rs, core) / max(1, len(rs))
        f["v10 less the start: starts the key"] = cues.lcp(rs, k) / max(1, len(rs))
        f["v10 less the start: inside the key"] = float(len(rs) >= 3 and rs in k)
    return home, carried, VIEWS[view], is_core, is_inside


# Version 11: the people of a schema. Read from the misses of the GitHub
# development sets on 9 November 2026: a column named for someone
# (`created_by`, `author_id`, `owner`, `user_id`, `arrendador_id`) into the
# schema's table of people (`users`, `profiles`, `usuarios`), which the
# declarations of SchemaPile seldom hold. Round 1t's lexicon of roles reads a
# table as one of people when any of its words names a person, so
# `user_request` and `user_preferences` are tables of people to it. Here a
# table is one of people by its last word, the head of its name; a column is
# someone's by the lexicon's actor words, by a person word among its own, or
# by ending in a word for "by" once its key words are taken off.
BY = frozenset({"by", "por", "par", "oleh", "von", "door", "da", "przez", "tarafindan"})


@lru_cache(maxsize=400_000)
def people_head(name: str) -> bool:
    toks = roles._tokens(name)
    if not toks:
        return False
    last = toks[-1]
    forms = {last, singular(last), last[:-2] if last.endswith("es") else last}
    if forms & roles.PERSON:
        return True
    run = "".join(toks)
    return any(len(w) >= 5 and (run.endswith(w) or run.endswith(w + "s")) for w in roles.PERSON)


@lru_cache(maxsize=400_000)
def someone(name: str) -> tuple:
    """(named for an actor, carries a person word) of a column's name."""
    words = list(style.pieces(name))
    while len(words) > 1 and words[-1] in words7.KEY_WORDS:
        words.pop()
    by = len(words) > 1 and words[-1] in BY
    person = any(w in roles.PERSON or singular(w) in roles.PERSON for w in words)
    return bool(by or roles.carries(name, "actor")), bool(person)


def add_people(f: dict, column: dict, target: dict, info: dict) -> None:
    actor, person = someone(column["name"])
    if not (actor or person):
        return
    head = people_head(target["name"])
    n = len(info["people"])
    f["v11 tables of people"] = min(n, 4) / 4
    if actor:
        f["v11 actor into a table of people" if head else "v11 actor into another table"] = 1.0
    if person:
        f["v11 person word into a table of people" if head else "v11 person word into another table"] = 1.0
    if head:
        f["v11 the only table of people"] = float(n == 1)
        f["v11 the shortest table of people"] = float(len(norm(target["name"])) == info["people shortest"])
    elif n:
        f["v11 a table of people stands elsewhere"] = 1.0


def pairs(schemas, split, guesses, people=False):
    for s in schemas:
        if s["split"] != split or not s["fk"]:
            continue
        info = schema_info(s)
        info["v5"] = cues.schema_info(s)
        if people:
            info["people"] = [t["name"] for t in s["tables"] if people_head(t["name"])]
            info["people shortest"] = min((len(norm(n)) for n in info["people"]), default=0)
        for row in six.pairs([s], split, guesses, seven=True, nine=True):
            _, table, column, target, _, _, f, key = row
            extra = add_cues(f, column, target, key, info)
            if people:
                add_people(f, column, target, info)
            yield (*row, extra)


def build(job):
    schemas, split, guesses, people = job
    hasher = FeatureHasher(n_features=HASHED, input_type="dict", alternate_sign=False)
    feats, meta = [], []
    for s, t, c, g, label, rule, f, key, extra in pairs(schemas, split, guesses, people):
        feats.append(f)
        meta.append((s["id"], s["repo"], bool(s.get("novel")), t["name"], c["name"], g["name"], g is t, label, rule,
                     f.get("v2 base", 0.0), f.get("v2 same type", 0.0), f.get("v2 named as the key", 0.0),
                     f.get("ends in id", 0.0), f.get("v2 bare", 0.0), key, *extra))
    x = hasher.transform(feats).astype(np.float32) if feats else sp.csr_matrix((0, HASHED), dtype=np.float32)
    return x, meta


def build_all(pool, schemas, split, guesses, people=False):
    chosen = [s for s in schemas if s["split"] == split and s["fk"]]
    by_schema = defaultdict(dict)
    for k, v in guesses.items():
        by_schema[k[0]][k] = v
    n = graph.WORKERS * 4
    jobs = [(c, split, {k: v for s in c for k, v in by_schema[s["id"]].items()}, people)
            for c in (chosen[i::n] for i in range(n)) if c]
    xs, meta = [], []
    for x, m in pool.map(build, jobs):
        xs.append(x)
        meta.extend(m)
    if not xs:
        return sp.csr_matrix((0, HASHED), dtype=np.float32), []
    return sp.vstack(xs).tocsr(), meta
