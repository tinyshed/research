"""Round 1t's pairs: round 1p's `both` with cues that do not lean on one
house style.

Round 1s's held-out reading, looked into on 8 November 2026, found a key
named after its target's primary key rather than its table (`zoneref` into
`sites`, whose key is `zoneregistry`), references ending in words round 1b's
list of key words lacks, and names shortened (`SEC_JOIN` into
`SecurityProfile`). The cues added here, version 5, read those without a
list of styles:

- the column against the target's key: the longest common prefix of the
  two names over each's length, and the column's and the key's stems equal,
  a stem being a name without the ending most of the schema's keys or most
  of its columns share;
- the column's stem as a shortening of the target: its letters in order in
  the target's name or in one of its words, first letter first;
- how many tables carry a column of the same name, and how many columns end
  in the column's last word, both over the schema;
- whether the column's last word ends the schema's keys.

    variant "both"      round 1p's pairs and features, as round 1s fitted them
    variant "cues"      the same pairs, version 5's features added
"""

import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1s"))
sys.path.insert(0, str(HERE.parent / "round-1p"))
sys.path.insert(0, str(HERE))

import letters  # noqa: E402,F401  (first: the trigrams sorted, in this process and the pool's)

import numpy as np  # noqa: E402
import scipy.sparse as sp  # noqa: E402
from sklearn.feature_extraction import FeatureHasher  # noqa: E402

import graph  # noqa: E402
import keys  # noqa: E402
import widen  # noqa: E402
from train import HASHED, norm, tokens  # noqa: E402

VARIANTS = {"both": False, "cues": True, "six": True, "seven": True, "eight": True}


def lcp(a: str, b: str) -> int:
    n = 0
    for x, y in zip(a, b):
        if x != y:
            break
        n += 1
    return n


def common_end(names: list[str], share: float = 0.5, shortest: int = 2) -> str:
    """The longest ending at least `share` of the names carry, each name
    longer than it, or ''."""
    names = [n for n in names if n]
    if len(names) < 3:
        return ""
    best = ""
    for k in range(shortest, 12):
        c = Counter(n[-k:] for n in names if len(n) > k)
        if not c:
            break
        end, n = c.most_common(1)[0]
        if n >= share * len(names):
            best = end
        else:
            break
    return best


def strip_end(name: str, end: str) -> str:
    return name[:-len(end)] if end and name.endswith(end) and len(name) > len(end) else name


def abbreviates(a: str, b: str) -> bool:
    """`a` is `b` shortened: `b`'s first letter, then `a`'s letters in order."""
    if len(a) < 2 or not b or a[0] != b[0] or len(a) >= len(b):
        return False
    j = 1
    for c in a[1:]:
        j = b.find(c, j)
        if j < 0:
            return False
        j += 1
    return True


def schema_info(s: dict) -> dict:
    pk_names = [norm(t["pk"][0]) for t in s["tables"] if len(t["pk"]) == 1]
    cols = [norm(c["name"]) for t in s["tables"] for c in t["columns"]
            if not (c["primary"] and len(t["pk"]) == 1)]
    pk_end = common_end(pk_names)
    col_end = common_end(cols, share=0.15, shortest=2)
    carried = Counter(norm(c["name"]) for t in s["tables"] for c in {norm(x["name"]): x for x in t["columns"]}.values())
    last = Counter((tokens(c["name"]) or [""])[-1] for t in s["tables"] for c in t["columns"])
    pk_last = Counter((tokens(t["pk"][0]) or [""])[-1] for t in s["tables"] if len(t["pk"]) == 1)
    return {"pk_end": pk_end, "col_end": col_end, "carried": carried, "last": last, "pk_last": pk_last,
            "columns": max(1, sum(len(t["columns"]) for t in s["tables"])), "tables": len(s["tables"])}


def stem5(name: str, info: dict) -> str:
    n = norm(name)
    for end in (info["col_end"], info["pk_end"]):
        if end and n.endswith(end) and len(n) - len(end) >= 2:
            return n[:-len(end)]
    tk = tokens(name)
    if len(tk) > 1 and tk[-1] in keys.KEY_WORDS:
        return "".join(tk[:-1])
    return n


def add_cues(f: dict, s: dict, table: dict, column: dict, target: dict, key: str, info: dict) -> None:
    c = norm(column["name"])
    k = norm(key)
    g = norm(target["name"])
    cs, ks = stem5(column["name"], info), strip_end(k, info["pk_end"])
    gw = [w for w in tokens(target["name"]) if len(w) >= 3]
    f["v5 prefix with key"] = lcp(c, k) / max(1, len(c))
    f["v5 prefix with key, of key"] = lcp(c, k) / max(1, len(k))
    f["v5 prefix with table"] = lcp(c, g) / max(1, len(c))
    f["v5 stems equal"] = float(bool(cs) and cs == ks)
    f["v5 stem starts key"] = float(len(cs) >= 2 and ks.startswith(cs))
    f["v5 key stem in column"] = float(len(ks) >= 3 and ks in c and ks != c)
    f["v5 shortens table"] = float(abbreviates(cs, g) or any(abbreviates(cs, w) or cs == w for w in gw))
    f["v5 shortens key"] = float(abbreviates(cs, ks))
    f["v5 table word starts stem"] = float(any(len(w) >= 3 and cs.startswith(w[:max(3, len(w) // 2)]) for w in gw))
    f["v5 tables carrying the name"] = min(info["carried"][c], 10) / 10
    last = (tokens(column["name"]) or [""])[-1]
    f["v5 columns ending so"] = min(info["last"][last] / info["tables"], 3) / 3
    f["v5 ends as keys end"] = float(bool(info["pk_end"]) and c.endswith(info["pk_end"]) and c != k)
    f["v5 last word ends keys"] = info["pk_last"][last] / max(1, info["tables"])
    f["v5 column end"] = float(bool(info["col_end"]) and c.endswith(info["col_end"]))


def pairs(schemas, split, guesses, variant):
    if variant in ("six", "seven", "eight"):
        import six  # noqa: E402
        yield from six.pairs(schemas, split, guesses, seven=variant in ("seven", "eight"), eight=variant == "eight")
        return
    with_cues = VARIANTS[variant]
    for s in schemas:
        if s["split"] != split or not s["fk"]:
            continue
        info = schema_info(s) if with_cues else None
        for row in widen.pairs([s], split, guesses, own_key=True, pseudo=True):
            if with_cues:
                _, table, column, target, _, _, f, key = row
                add_cues(f, s, table, column, target, key, info)
            yield row


def build(job):
    schemas, split, guesses, variant = job
    hasher = FeatureHasher(n_features=HASHED, input_type="dict", alternate_sign=False)
    feats, meta = [], []
    for s, t, c, g, label, rule, f, key in pairs(schemas, split, guesses, variant):
        feats.append(f)
        meta.append((s["id"], s["repo"], bool(s.get("novel")), t["name"], c["name"], g["name"], g is t, label, rule,
                     f.get("v2 base", 0.0), f.get("v2 same type", 0.0), f.get("v2 named as the key", 0.0),
                     f.get("ends in id", 0.0), f.get("v2 bare", 0.0), key))
    x = hasher.transform(feats).astype(np.float32) if feats else sp.csr_matrix((0, HASHED), dtype=np.float32)
    return x, meta


def build_all(pool, schemas, split, guesses, variant):
    from collections import defaultdict
    chosen = [s for s in schemas if s["split"] == split and s["fk"]]
    by_schema = defaultdict(dict)
    for k, v in guesses.items():
        by_schema[k[0]][k] = v
    n = graph.WORKERS * 4
    jobs = [(c, split, {k: v for s in c for k, v in by_schema[s["id"]].items()}, variant)
            for c in (chosen[i::n] for i in range(n)) if c]
    xs, meta = [], []
    for x, m in pool.map(build, jobs):
        xs.append(x)
        meta.extend(m)
    if not xs:
        return sp.csr_matrix((0, HASHED), dtype=np.float32), []
    return sp.vstack(xs).tocsr(), meta
