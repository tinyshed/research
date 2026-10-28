"""Round 1h's panels by title alone: SchemaPile-Perm schemas that look like
an application, none like a judged database, round 1c's thirty best
questions on each, and the three teachers asked whether a person new to the
database would keep each on a first overview, from its title and the
schema's tables.

    python -I names.py select <schemapile-perm.json.gz> <corpus/see> <out> [<schemas>]
    python -I names.py candidates <round 1c learn> <out>
    JUDGE_KEY=... python -I names.py jev <out> [<at once>]
    JUDGE_KEY=... python -I names.py batch <out> <model> <judge>
    JUDGE_KEY=... python -I names.py sync <out> <model> <judge> [<at once>]
    python -I names.py gather <out> <judge>

select writes <out>/schemas.jsonl; candidates <out>/panels.jsonl; jev
<out>/labels/jev.json; batch submits one batch a model to OpenRouter and
keeps its replies in <out>/raw/<judge>/, and sync asks the same prompts one
by one where a model has no batch; gather writes <out>/labels/<judge>.json.
"""

from __future__ import annotations

import gzip
import hashlib
import json
import os
import random
import re
import sys
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE.parent / "round-1c"))
import learn  # noqa: E402
import rank  # noqa: E402
import teacher  # noqa: E402
from aq import generate, identity, load  # noqa: E402

SEED = 20261104
TOP = 30
OPENROUTER = "https://openrouter.ai/api/v1"


def select(source: Path, see_dir: Path, out: Path, n: int = 4000) -> None:
    """Round 1c's filter of schemas that look like an application, one per
    repository and table set, none whose tables overlap a judged database's
    by 40 % or more."""
    corpus = json.load(gzip.open(source, "rt", encoding="utf-8"))
    judged = [{t["name"].lower() for t in json.loads(p.read_text(encoding="utf-8"))["tables"]}
              for p in see_dir.glob("*/snapshot.json")]
    ids = sorted(corpus)
    random.Random(SEED).shuffle(ids)
    seen_repos, seen_sigs, chosen, near = set(), set(), [], 0
    for sid in ids:
        s = corpus[sid]
        if not teacher.looks_like_an_application(s):
            continue
        repo = teacher.repository(s["INFO"].get("URL") or sid)
        names = {t.lower() for t in s["TABLES"]}
        sig = hashlib.sha256(json.dumps(sorted(names)).encode()).hexdigest()[:16]
        if repo in seen_repos or sig in seen_sigs:
            continue
        if any(len(names & j) / len(names | j) >= 0.4 for j in judged):
            near += 1
            continue
        seen_repos.add(repo)
        seen_sigs.add(sig)
        chosen.append((sid, repo, s))
        if len(chosen) == n:
            break
    out.mkdir(parents=True, exist_ok=True)
    with (out / "schemas.jsonl").open("w", encoding="utf-8") as f:
        for k, (sid, repo, s) in enumerate(chosen):
            snap = teacher.snapshot(sid, s)
            snap["key"], snap["repo"] = f"n{k:04d}", repo
            f.write(json.dumps(snap) + "\n")
    print(f"{len(chosen)} schemas; {near} left out as like a judged database")


def schemas(out: Path):
    with (out / "schemas.jsonl").open(encoding="utf-8") as f:
        for line in f:
            yield json.loads(line)


def candidates(learn_dir: Path, out: Path) -> None:
    """Each schema's thirty best distinct questions by round 1c's ranker,
    tier S, with what the student of names will read of them."""
    import lightgbm as lgb
    model = lgb.Booster(model_file=str(learn_dir / "ranker.txt"))
    names = model.feature_name()
    table = json.loads((learn_dir / "words.json").read_text(encoding="utf-8"))
    n_panels = 0
    with (out / "panels.jsonl").open("w", encoding="utf-8") as f:
        for snap in schemas(out):
            try:
                schema = load(snap, False)
                qs = [q for q in generate(schema) if rank.veto(schema, q) is None]
            except Exception as e:  # noqa: BLE001 — a schema the generator cannot read is skipped and counted
                print(f"{snap['key']}: {str(e)[:80]}")
                continue
            if not qs:
                continue
            feats = [rank.features(schema, q) for q in qs]
            slots = [rank.slots(schema, q) for q in qs]
            X, _, _ = learn.matrix_free({"features": feats, "slots": slots}, names, table)
            scores = model.predict(X)
            seen, k = set(), 0
            for i in np.argsort(-scores, kind="stable"):
                ident = identity(schema, qs[i])
                if ident in seen:
                    continue
                seen.add(ident)
                f.write(json.dumps({"id": f"{snap['key']}-{k:02d}", "schema": snap["key"], "repo": snap["repo"],
                                    "title": qs[i].title(schema), "score": round(float(scores[i]), 4), "rank": k + 1,
                                    "slots": slots[i],
                                    "features": {a: (round(b, 4) if isinstance(b, float) else b) for a, b in feats[i].items()}})
                        + "\n")
                k += 1
                n_panels += 1
                if k == TOP:
                    break
    print(f"{n_panels} panels")


def panels(out: Path) -> list[dict]:
    with (out / "panels.jsonl").open(encoding="utf-8") as f:
        return [json.loads(line) for line in f]


def tables_line(snap: dict) -> str:
    return "; ".join(f"{t['name']} ({', '.join(c['name'] for c in t['columns'][:12])}"
                     f"{', …' if len(t['columns']) > 12 else ''})" for t in snap["tables"])


QUESTION = {
    "keep": {
        "type": "noul",
        "instructions": "Would a person new to this database keep this panel on a first overview dashboard of it?",
        "criteria": {
            "true": "It answers a question a newcomer would want answered first, and a chart of it would mean what its title says.",
            "false": "It is minor or obscure, it sums or averages what does not add up (an identifier, a code, a price without a quantity), or it groups by something meaningless.",
        },
    },
}


def jev(out: Path, at_once: int = 64) -> None:
    """Each panel alone, with its schema's tables: a probability of keep."""
    key = os.environ["JUDGE_KEY"]
    snaps = {s["key"]: s for s in schemas(out)}
    items = panels(out)
    path = out / "labels" / "jev.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    done = json.loads(path.read_text(encoding="utf-8")) if path.exists() else {}
    todo = [p for p in items if p["id"] not in done]

    def one(p):
        state = {"database tables": tables_line(snaps[p["schema"]])[:4000], "panel title": p["title"]}
        body = json.dumps({"model": "typesafe/jev-1.13", "state": state, "questions": QUESTION}).encode()
        for _ in range(3):
            req = urllib.request.Request(f"{OPENROUTER.removesuffix('/v1')}/alpha/decisions", data=body, method="POST",
                                         headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            try:
                with urllib.request.urlopen(req, timeout=120) as r:
                    d = json.loads(r.read())
                return p["id"], float(d["answers"]["keep"]["noul"]), d.get("usage", {}).get("cost", 0.0)
            except Exception:  # noqa: BLE001 — asked again, then left out
                continue
        return p["id"], None, 0.0

    cost, k = 0.0, 0
    with ThreadPoolExecutor(at_once) as pool:
        for pid, v, c in pool.map(one, todo):
            cost += c or 0.0
            if v is not None:
                done[pid] = round(v, 4)
            k += 1
            if k % 2000 == 0:
                path.write_text(json.dumps(done), encoding="utf-8")
                print(f"{k} of {len(todo)}, ${cost:.3f}", flush=True)
    path.write_text(json.dumps(done), encoding="utf-8")
    print(f"{len(done)} of {len(items)} panels answered; ${cost:.4f} this run")


PROMPT = """You are judging panels proposed for a first overview dashboard of a database you see only as its schema.
For each panel id below, say whether a person new to this database would keep it on a first overview: it answers a
question a newcomer would want answered first, and a chart of it would mean what its title says — not a sum or
average of an identifier, a code or a price without its quantity, not a grouping by something meaningless, not minor.

Reply with only one JSON object and no other text: {"panels": {"<panel id>": true or false, ...}} — every id below.

Schema:
{schema}
Panels:
{panels}
"""


def prompts(out: Path) -> dict[str, str]:
    snaps = {s["key"]: s for s in schemas(out)}
    by = {}
    for p in panels(out):
        by.setdefault(p["schema"], []).append(p)
    return {k: PROMPT.replace("{schema}", teacher.text(snaps[k])[:12000]).replace(
        "{panels}", "\n".join(f"{p['id']}: {p['title']}" for p in ps)) for k, ps in by.items()}


def _post(url: str, body: dict, key: str, timeout: int = 600):
    req = urllib.request.Request(url, data=json.dumps(body).encode(), method="POST",
                                 headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read())


def _get(url: str, key: str):
    req = urllib.request.Request(url, headers={"Authorization": f"Bearer {key}"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return json.loads(r.read())


def batch(out: Path, model: str, judge: str) -> None:
    """Every schema's prompt in batches through OpenRouter's batch endpoint,
    each reply kept; polled once a minute until each batch ends."""
    key = os.environ["JUDGE_KEY"]
    raw = out / "raw" / judge
    raw.mkdir(parents=True, exist_ok=True)
    todo = {k: v for k, v in prompts(out).items() if not (raw / f"{k}.txt").exists()}
    keys = sorted(todo)
    ids = []
    for i in range(0, len(keys), 1000):
        part = keys[i:i + 1000]
        body = {"endpoint": "/v1/chat/completions", "model": model,
                "requests": [{"custom_id": k, "body": {"messages": [{"role": "user", "content": todo[k]}],
                                                      "temperature": 0}} for k in part]}
        try:
            d = _post(f"{OPENROUTER}/batches", body, key)
        except urllib.error.HTTPError as e:
            print(f"submit refused: {e.code} {e.read()[:300]!r}")
            return
        ids.append(d["id"])
        print(f"submitted {d['id']}: {len(part)} prompts, {d.get('status')}", flush=True)
    (raw / "batches.json").write_text(json.dumps(ids), encoding="utf-8")
    poll(out, judge, key)


def poll(out: Path, judge: str, key: str | None = None) -> None:
    key = key or os.environ["JUDGE_KEY"]
    raw = out / "raw" / judge
    pending = json.loads((raw / "batches.json").read_text(encoding="utf-8"))
    while pending:
        time.sleep(60)
        still = []
        for bid in pending:
            try:
                d = _get(f"{OPENROUTER}/batches/{bid}", key)
            except Exception as e:  # noqa: BLE001 — polled again next minute
                print(f"{bid}: {str(e)[:80]}", flush=True)
                still.append(bid)
                continue
            status = d.get("status")
            if status in ("completed", "failed", "expired", "cancelled") and (status != "completed" or d.get("results")):
                n = 0
                for r in d.get("results") or []:
                    try:
                        text = r["response"]["body"]["choices"][0]["message"]["content"]
                    except (KeyError, IndexError, TypeError):
                        continue
                    if text:
                        (raw / f"{r['custom_id']}.txt").write_text(text, encoding="utf-8")
                        n += 1
                print(f"{bid}: {status}, {n} replies; cost {d.get('usage', {}).get('cost')}", flush=True)
            else:
                still.append(bid)
                print(f"{bid}: {status} {d.get('request_counts', '')}", flush=True)
        pending = still


def sync(out: Path, model: str, judge: str, at_once: int = 32, shard: str = "0/1") -> None:
    """The same prompts one by one, for a model with no batch variant; a
    shard i/n takes every n-th schema, and a schema answered meanwhile by
    another is skipped."""
    key = os.environ["JUDGE_KEY"]
    raw = out / "raw" / judge
    raw.mkdir(parents=True, exist_ok=True)
    i_shard, n_shard = map(int, shard.split("/"))
    todo = {k: v for j, (k, v) in enumerate(sorted(prompts(out).items()))
            if j % n_shard == i_shard and not (raw / f"{k}.txt").exists()}

    def one(k):
        if (raw / f"{k}.txt").exists():
            return True
        for _ in range(3):
            try:
                d = _post(f"{OPENROUTER}/chat/completions", {"model": model, "temperature": 0,
                                                            "messages": [{"role": "user", "content": todo[k]}]}, key, 300)
                text = d["choices"][0]["message"]["content"]
            except Exception:  # noqa: BLE001 — asked again, then left out
                continue
            if text and '"panels"' in text:
                (raw / f"{k}.txt").write_text(text, encoding="utf-8")
                return True
        return False

    ok = 0
    with ThreadPoolExecutor(at_once) as pool:
        for i, r in enumerate(pool.map(one, sorted(todo))):
            ok += r
            if (i + 1) % 200 == 0:
                print(f"{i + 1} of {len(todo)}, {ok} answered", flush=True)
    print(f"{ok} of {len(todo)} answered")


def gather(out: Path, judge: str) -> None:
    got, bad = {}, 0
    decoder = json.JSONDecoder()
    for f in (out / "raw" / judge).glob("n*.txt"):
        text = f.read_text(encoding="utf-8", errors="replace")
        ans = None
        for m in re.finditer(r"\{\s*\"panels\"", text):
            try:
                ans = decoder.raw_decode(text, m.start())[0]["panels"]
            except (json.JSONDecodeError, KeyError):
                continue
        if ans is None:
            bad += 1
            continue
        for pid, v in ans.items():
            if isinstance(v, bool) and pid.startswith(f.stem + "-"):
                got[pid] = v
    (out / "labels").mkdir(exist_ok=True)
    (out / "labels" / f"{judge}.json").write_text(json.dumps(got), encoding="utf-8")
    print(f"{len(got)} panels from {judge}; {bad} replies unreadable")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, a = sys.argv[1], sys.argv[2:]
    if cmd == "select":
        select(Path(a[0]), Path(a[1]), Path(a[2]), int(a[3]) if len(a) > 3 else 4000)
    elif cmd == "candidates":
        candidates(Path(a[0]), Path(a[1]))
    elif cmd == "jev":
        jev(Path(a[0]), int(a[1]) if len(a) > 1 else 64)
    elif cmd == "batch":
        batch(Path(a[0]), a[1], a[2])
    elif cmd == "poll":
        poll(Path(a[0]), a[1])
    elif cmd == "sync":
        sync(Path(a[0]), a[1], a[2], int(a[3]) if len(a) > 3 else 32, a[4] if len(a) > 4 else "0/1")
    else:
        gather(Path(a[0]), a[1])
