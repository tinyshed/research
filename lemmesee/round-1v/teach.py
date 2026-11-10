"""Round 1v: a language model as an offline teacher of keys nobody declared.

    python -I teach.py prompts <set>            the set's prompts, corpus/round-1v/lm/prompts/<set>/
    python -I teach.py collect <model> <set>    the replies read into corpus/round-1v/lm/<model>--<set>.jsonl
    python -I teach.py agree <model> <set>      the teacher against the declarations, counts only
    python -I teach.py taught <file> <set> [<set> ...]  what both teachers add, corpus/round-1v/lm/<file>

A teacher sees what the key model sees and nothing of ours (teaching.md):
round 1p's whole-schema prompt, unchanged (`gemini.render` and `DB_ASK`), a
schema's tables, columns, types and primary keys with every declaration
taken out, and the question which columns refer to which tables. It is asked
through round 1s's `ask.py`, every request recorded; the key comes from the
environment and is never printed or written.

The sets are of SchemaPile-Perm's training and validation splits only, as
round 1b prepared them and under their original names:

    train-declared-20       training schemas that declare keys, 20 tables or more
    train-declared-10       ... 10 to 19 tables
    validation-declared-10  validation schemas that declare keys, 10 tables or more
    train-undeclared-5      training schemas that declare no key, 5 tables or more

What a teacher's answers are for: a column it names that no declaration
names is most likely a key nobody declared. In a schema that declares keys
such a column is taken out of the training sample's negatives or taught as a
key; a schema that declares none is taught from the teacher's keys alone.
No test split and no held-out set is ever shown to a teacher that teaches.
"""

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1p"))
sys.path.insert(0, str(HERE.parent / "round-1b"))

import gemini  # noqa: E402  (round 1p's prompt)

ROOT = HERE.parent / "corpus"
OUT = ROOT / "round-1v" / "lm"
SETS = {
    "train-declared-20": ("train", True, 20, 10 ** 6),
    "train-declared-10": ("train", True, 10, 20),
    "validation-declared-10": ("validation", True, 10, 10 ** 6),
    "train-undeclared-5": ("train", False, 5, 10 ** 6),
}


def chosen(which: str):
    split, declared, lo, hi = SETS[which]
    with open(ROOT / "round-1b" / "hidden.jsonl", encoding="utf-8") as f:
        for line in f:
            s = json.loads(line)
            if s["split"] == split and bool(s["fk"]) == declared and lo <= len(s["tables"]) < hi:
                yield s


def prompts(which: str) -> None:
    d = OUT / "prompts" / which
    d.mkdir(parents=True, exist_ok=True)
    n = chars = 0
    index = {}
    for s in chosen(which):
        text = gemini.DB_ASK.format(schema=gemini.render(s))
        stem = gemini.db_stem(s["id"])
        (d / f"{stem}.txt").write_text(text, encoding="utf-8")
        index[stem] = s["id"]
        n += 1
        chars += len(text)
    (OUT / "prompts" / f"{which}.index.json").write_text(json.dumps(index, indent=0) + "\n", encoding="utf-8")
    print(f"{which}: {n} prompts, {chars} characters")


def read(text: str, s: dict) -> list:
    """A reply's keys that name a column and a table of the schema, one a column (round 1s's reading)."""
    obj = gemini.last_object(text, "keys")
    if obj is None:
        return None
    names = {t["name"].lower(): t for t in s["tables"]}
    out, seen = [], set()
    for e in obj.get("keys") or []:
        if not isinstance(e, dict):
            continue
        t = names.get(str(e.get("table", "")).lower())
        to = names.get(str(e.get("references", "")).lower())
        col = next((c["name"] for c in t["columns"] if c["name"].lower() == str(e.get("column", "")).lower()),
                   None) if t else None
        if not (t and to and col) or (t["name"], col) in seen:
            continue
        seen.add((t["name"], col))
        out.append([t["name"], col, to["name"]])
    return out


def collect(model: str, which: str) -> None:
    d = OUT / "llm" / model / which
    n = unread = unanswered = 0
    with open(OUT / f"{model}--{which}.jsonl", "w", encoding="utf-8", newline="\n") as f:
        for s in chosen(which):
            raw = d / f"{gemini.db_stem(s['id'])}.raw"
            if not raw.exists():
                unanswered += 1
                continue
            keys = read(raw.read_text(encoding="utf-8", errors="replace"), s)
            if keys is None:
                unread += 1
                continue
            f.write(json.dumps({"id": s["id"], "keys": keys}, ensure_ascii=False) + "\n")
            n += 1
    print(f"{model} {which}: {n} read, {unread} unreadable, {unanswered} unanswered")


def agree(model: str, which: str) -> None:
    taught = {}
    for line in open(OUT / f"{model}--{which}.jsonl", encoding="utf-8"):
        r = json.loads(line)
        taught[r["id"]] = {tuple(k) for k in r["keys"]}
    tp = fp = fn = same_column = 0
    by = {}
    for s in chosen(which):
        if s["id"] not in taught:
            continue
        truth = {tuple(k) for k in s["fk"]}
        t = taught[s["id"]]
        declared_columns = {(a, c) for a, c, _ in truth}
        tp += len(truth & t)
        fp += len(t - truth)
        fn += len(truth - t)
        same_column += sum(1 for a, c, g in t - truth if (a, c) in declared_columns)
        n = len(s["tables"])
        b = next(k for k in ((0, 5), (5, 10), (10, 20), (20, 40), (40, 10 ** 6)) if k[0] <= n < k[1])
        e = by.setdefault(b, [0, 0, 0, 0])
        e[0] += 1
        e[1] += len(truth)
        e[2] += len(truth & t)
        e[3] += len({(a, c) for a, c, _ in t} - declared_columns)
    p = tp / max(tp + fp, 1)
    r = tp / max(tp + fn, 1)
    print(f"{model} {which}: {len(taught)} schemas; against the declarations P {p:.3f} R {r:.3f}; "
          f"{fp} keys no declaration names, {same_column} of them on a declared column (another target)")
    for b in sorted(by):
        e = by[b]
        print(f"  tables {b[0]}-{b[1] - 1 if b[1] < 10 ** 6 else ''}: schemas {e[0]}, declared {e[1]}, found {e[2]} "
              f"({e[2] / max(e[1], 1):.3f}), undeclared columns named {e[3]} ({e[3] / max(e[1], 1):.2f} a declared key)")


TEACHERS = ("gemini-3.5-flash-lite", "gemini-3-flash")


def taught(name: str, sets: list) -> None:
    """What the two teachers add to a schema's declarations, by index into
    its tables and columns (a renamed copy keeps the order): `both`, a
    column no declaration names that both teachers point at the same table;
    `one`, a column no declaration names that one of them names, or both
    with different tables. A declared column is left as declared."""
    out = {}
    counts = {"schemas": 0, "declared": 0, "both": 0, "one": 0, "schemas with a taught key": 0}
    for which in sets:
        answers = []
        for m in TEACHERS:
            path = OUT / f"{m}--{which}.jsonl"
            got = {}
            for line in open(path, encoding="utf-8"):
                r = json.loads(line)
                got[r["id"]] = {(a, c): g for a, c, g in r["keys"]}
            answers.append(got)
        for s in chosen(which):
            if any(s["id"] not in a for a in answers):
                continue
            declared = {(a, c) for a, c, _ in s["fk"]}
            ti = {t["name"]: i for i, t in enumerate(s["tables"])}
            ci = [{c["name"]: j for j, c in enumerate(t["columns"])} for t in s["tables"]]
            a, b = (x[s["id"]] for x in answers)
            both, one = [], []
            for k in sorted(set(a) | set(b)):
                if k in declared:
                    continue
                t, c = ti[k[0]], ci[ti[k[0]]][k[1]]
                if a.get(k) is not None and a.get(k) == b.get(k):
                    both.append([t, c, ti[a[k]]])
                else:
                    one.append([t, c])
            out[s["id"]] = {"both": both, "one": one}
            counts["schemas"] += 1
            counts["declared"] += len(declared)
            counts["both"] += len(both)
            counts["one"] += len(one)
            counts["schemas with a taught key"] += bool(both)
    (OUT / name).write_text(json.dumps(out) + "\n", encoding="utf-8")
    print(json.dumps({"sets": sets, **counts}))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "taught":
        taught(sys.argv[2], sys.argv[3:])
    elif cmd == "prompts":
        prompts(sys.argv[2])
    elif cmd == "collect":
        collect(sys.argv[2], sys.argv[3])
    else:
        agree(sys.argv[2], sys.argv[3])
