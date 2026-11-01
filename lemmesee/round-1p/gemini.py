"""Round 1p's language-model baseline: Gemini 3.8 Flash through a
CLIProxyAPI, in both tasks, beside round 1b's model measured the same way.

    python -I gemini.py cut <out>
    CLIPROXY_KEY=... python -I gemini.py ask <out> <endpoint, …/v1> <model> [<at once>]
    python -I gemini.py measure <out> [<suffix>]

`cut` writes into <out>/gemini/ `pair-<hash>.txt`, the paper's own prompt
for each pair of bench.jsonl, and `db-<hash>.txt`, each database of hidden.jsonl as round
1b's model sees it — names, types, NOT NULL, UNIQUE, primary keys — asking
for every foreign key. `ask` keeps each reply beside its prompt as `.raw`.
`measure` writes gemini.json: the paper's task for the model, and round
1b's task for it and for round 1b's model from open-predictions.jsonl, both
against every one-column key the databases declare.
"""

from __future__ import annotations

import hashlib
import json
import time
import os
import re
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np

QUESTION = ("You are given the following SQL database tables: \n{tables}\nOutput a json string with the "
            "following schema {{table, column, referencedTable, referencedColumn}} that contains the foreign "
            "key relationship between the two tables.")
PAIR_REPLY = "\nReply with only that JSON object and no other text."
DB_ASK = (
    "Below is a relational database's schema with its foreign key declarations removed: each table with its "
    "columns, their types, NOT NULL and UNIQUE where declared, and its primary key.\n\n{schema}\n\n"
    "List every column that refers to rows of a table of this schema, its own table included, as a foreign "
    "key would, and the table it refers to; a column refers to at most one table. Reply with only one JSON "
    'object and no other text: {{"keys": [{{"table": "...", "column": "...", "references": "..."}}, ...]}}, '
    "an empty list if there are none."
)
SEED = 20261101


def pair_stem(p: dict) -> str:
    """A pair's prompt file, named by what it asks so a larger run keeps it."""
    key = json.dumps([p["source"], p["db"], p["table"], p["foreign_table"]], ensure_ascii=False)
    return "pair-" + hashlib.sha1(key.encode()).hexdigest()[:16]


def db_stem(sid: str) -> str:
    return "db-" + hashlib.sha1(sid.encode()).hexdigest()[:16]


def table_string(t: dict) -> str:
    return t["name"].lower() + "(" + ", ".join(c["name"].lower() for c in t["columns"]) + ")"


def render(s: dict) -> str:
    lines = []
    for t in s["tables"]:
        pk = f" — primary key ({', '.join(t['pk'])})" if t["pk"] else " — no primary key"
        lines.append(f'table "{t["name"]}"{pk}')
        for c in t["columns"]:
            flags = (" NOT NULL" if c["nullable"] is False else "") + (" UNIQUE" if c["unique"] else "")
            lines.append(f'  "{c["name"]}" {c["type"] or "untyped"}{flags}')
    return "\n".join(lines)


def cut(out: Path) -> None:
    d = out / "gemini"
    d.mkdir(exist_ok=True)
    schemas = [json.loads(line) for line in open(out / "hidden.jsonl", encoding="utf-8")]
    tables = {s["id"]: {t["name"]: t for t in s["tables"]} for s in schemas}
    bench = [json.loads(line) for line in open(out / "bench.jsonl", encoding="utf-8")]
    for p in bench:
        ts = tables[f"{p['source']}/{p['db']}"]
        prompt = QUESTION.format(tables="\n".join([table_string(ts[p["table"]]), table_string(ts[p["foreign_table"]])]))
        (d / f"{pair_stem(p)}.txt").write_text(prompt + PAIR_REPLY, encoding="utf-8")
    for s in schemas:
        if s["fk"]:
            (d / f"{db_stem(s['id'])}.txt").write_text(DB_ASK.format(schema=render(s)), encoding="utf-8")
    print(f"{len(bench)} pairs, {sum(1 for s in schemas if s['fk'])} databases")


def ask(out: Path, url: str, model: str, workers: int = 8, raws: Path | None = None) -> None:
    """Every prompt not yet answered, through an OpenAI-style endpoint,
    several at once. The key comes from the environment and is never written."""
    import urllib.request
    from concurrent.futures import ThreadPoolExecutor

    key = os.environ["CLIPROXY_KEY"]
    # Another model's replies go to a folder of their own, the prompts shared
    raws = raws or out / "gemini"
    raws.mkdir(parents=True, exist_ok=True)
    reply = lambda p: raws / f"{p.stem}.raw"  # noqa: E731
    todo = [p for p in sorted((out / "gemini").glob("*.txt"))
            if not (reply(p).exists() and reply(p).stat().st_size > 0)]

    def one(p: Path) -> str:
        body = json.dumps({"model": model, "messages": [{"role": "user", "content": p.read_text(encoding="utf-8")}],
                           "temperature": 0}).encode()
        err = ""
        for attempt in range(8):
            if attempt:
                # A refusal for too many requests waits longer each time
                time.sleep(min(5 * 2 ** (attempt - 1), 120) if "429" in err else 2)
            req = urllib.request.Request(f"{url}/chat/completions", data=body, method="POST",
                                         headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            try:
                with urllib.request.urlopen(req, timeout=600) as r:
                    text = json.loads(r.read())["choices"][0]["message"]["content"]
            except Exception as e:  # noqa: BLE001 — a failed request is asked again, then reported
                err = str(e)[:120]
                continue
            if not text:
                err = "an empty reply"
                continue
            reply(p).write_text(text, encoding="utf-8")
            return f"{p.name} ok"
        return f"{p.name} failed: {err}"

    with ThreadPoolExecutor(workers) as pool:
        for line in pool.map(one, todo):
            print(line, flush=True)


def last_object(text: str, must: str):
    """The last whole JSON object of a reply holding the given key."""
    found, decoder = None, json.JSONDecoder()
    for m in re.finditer(r"\{", text):
        try:
            obj, _ = decoder.raw_decode(text, m.start())
        except json.JSONDecodeError:
            continue
        if isinstance(obj, dict) and must in obj:
            found = obj
    return found


def prf(tp: int, fp: int, fn: int):
    p = tp / (tp + fp) if tp + fp else 0.0
    r = tp / (tp + fn) if tp + fn else 0.0
    return p, r, 2 * p * r / (p + r) if p + r else 0.0


def open_counts(truth: dict, preds: dict, ids) -> tuple[int, int, int]:
    tp = fp = fn = 0
    for i in ids:
        t, p = truth.get(i, set()), preds.get(i, set())
        tp += len(t & p)
        fp += len(p - t)
        fn += len(t - p)
    return tp, fp, fn


def measure(out: Path, suffix: str = "", raws: Path | None = None, label: str = "gemini") -> None:
    d = raws or out / "gemini"
    schemas = [json.loads(line) for line in open(out / "hidden.jsonl", encoding="utf-8")]
    bench = [json.loads(line) for line in open(out / "bench.jsonl", encoding="utf-8")]
    result = {}

    # The paper's task
    unread, rows = 0, []
    for p in bench:
        raw = d / f"{pair_stem(p)}.raw"
        obj = last_object(raw.read_text(encoding="utf-8", errors="replace"), "column") if raw.exists() else None
        unread += obj is None
        got = [str((obj or {}).get(k, "")).lower() for k in ("table", "column", "referencedTable", "referencedColumn")]
        want = [p["table"].lower(), p["column"].lower(), p["foreign_table"].lower(), p["referred_column"].lower()]
        rows.append({**p, "hit": got == want})
    for src in ("spider", "bird", "ctu"):
        kept = [r for r in rows if r["source"] == src and not r["occurs_in_perm"] and not r["too_large"]]
        result[f"{src}: the paper's task"] = {"pairs": len(kept), "recall_at_1": sum(r["hit"] for r in kept) / len(kept)}
    result["pairs unreadable"] = unread

    # Round 1b's task, against every one-column key declared
    truth, ours, theirs, unreadable, unknown = {}, defaultdict(set), defaultdict(set), 0, 0
    for s in schemas:
        if not s["fk"]:
            continue
        truth[s["id"]] = {(a, c, b) for a, c, b in s["fk"]}
        names = {t["name"].lower(): t for t in s["tables"]}
        raw = d / f"{db_stem(s['id'])}.raw"
        obj = last_object(raw.read_text(encoding="utf-8", errors="replace"), "keys") if raw.exists() else None
        if obj is None:
            unreadable += 1
            continue
        seen = set()
        for e in obj.get("keys") or []:
            t = names.get(str(e.get("table", "")).lower())
            to = names.get(str(e.get("references", "")).lower())
            col = next((c["name"] for c in t["columns"] if c["name"].lower() == str(e.get("column", "")).lower()),
                       None) if t else None
            if not (t and to and col):
                unknown += 1
                theirs[s["id"]].add(("?", str(e), ""))  # a key naming nothing of the schema is a false one
                continue
            if (t["name"], col) in seen:
                continue
            seen.add((t["name"], col))
            theirs[s["id"]].add((t["name"], col, to["name"]))
    for line in open(out / f"open-predictions{suffix}.jsonl", encoding="utf-8"):
        sid, table, column, target, _ = json.loads(line)
        ours[sid].add((table, column, target))
    source = {s["id"]: s["source"] for s in schemas}
    novel = {s["id"]: s["novel"] for s in schemas}
    rng = np.random.default_rng(SEED)
    for name in ("spider", "bird", "ctu", "all"):
        for subset in ("all", "novel"):
            ids = [i for i in truth if (name == "all" or source[i] == name) and (subset == "all" or novel[i])]
            a, b = prf(*open_counts(truth, ours, ids)), prf(*open_counts(truth, theirs, ids))
            diffs = []
            for _ in range(1000):
                pick = list(rng.choice(ids, len(ids)))
                diffs.append(prf(*open_counts(truth, ours, pick))[2] - prf(*open_counts(truth, theirs, pick))[2])
            result[f"{name}, {subset}: round 1b's task"] = {
                "databases": len(ids), "keys declared": sum(len(truth[i]) for i in ids),
                "round 1b's model": dict(zip(("precision", "recall", "f1"), a)),
                "gemini": dict(zip(("precision", "recall", "f1"), b)),
                "f1_model_minus_gemini_95": [float(np.percentile(diffs, 2.5)), float(np.percentile(diffs, 97.5))],
            }
    result["databases unreadable"] = unreadable
    result["keys naming nothing of the schema"] = unknown
    (out / f"{label}{suffix}.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "cut":
        cut(Path(sys.argv[2]))
    elif sys.argv[1] == "ask":
        ask(Path(sys.argv[2]), sys.argv[3], sys.argv[4], int(sys.argv[5]) if len(sys.argv) > 5 else 8,
            Path(sys.argv[6]) if len(sys.argv) > 6 else None)
    else:
        measure(Path(sys.argv[2]), sys.argv[3] if len(sys.argv) > 3 else "",
                Path(sys.argv[4]) if len(sys.argv) > 4 else None, sys.argv[5] if len(sys.argv) > 5 else "gemini")
