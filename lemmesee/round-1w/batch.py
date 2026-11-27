"""Round 1w: the language models asked in batches through OpenRouter, every
request recorded.

    python -I batch.py prompts                         the whole-schema prompts of every set, and their hashes
    OPENROUTER_API_KEY=... python -I batch.py submit <model> <set> [<first n>]
    OPENROUTER_API_KEY=... python -I batch.py collect  every batch submitted and not yet collected
    python -I batch.py spent                           what the collected batches were charged

The protocol is round 1s's (ask.py) and the prompt round 1p's (gemini.py's
`render` and `DB_ASK`), unchanged: one user message a database, no system
prompt, at most 32 768 output tokens, temperature 0 where the model's page
lists the parameter, each identifier's default reasoning. What differs is
the road: OpenRouter's batch endpoint (POST /api/v1/batches, the
`/v1/chat/completions` shape), which answers within a day at about half the
price, in place of a request at a time through a proxy. A batch holds one
model's prompts of one set, at most CHUNK of them.

<model> is an OpenRouter slug (`anthropic/claude-sonnet-5.5`); its replies
go to corpus/round-1w/lm/llm/<the slug's last part>/<set>/ as round 1s kept
them, `<stem>.raw` the reply as it came and `<stem>.json` its record, so
that round 1s's lm.py reads them. A batch's own record, with what it was
charged, stays in corpus/round-1w/lm/batches/. The key comes from the
environment and is never printed or written.
"""

import hashlib
import json
import os
import shutil
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1p"))

CORPUS = HERE.parent / "corpus"
OUT = CORPUS / "round-1w" / "lm"
API = "https://openrouter.ai/api/v1"
TEMPERATURE = 0
MAX_TOKENS = 32768
CHUNK = 250
# set: where its prompts come from; the first three were written by the earlier rounds and are copied
COPIED = {"db": CORPUS / "round-1s" / "prompts" / "db",
          "heldout-db": CORPUS / "round-1s" / "prompts" / "heldout-db",
          "fresh-db": CORPUS / "round-1t" / "lm" / "prompts" / "fresh-db"}
RENDERED = {"plain-a-db": CORPUS / "round-1v" / "heldout-github-plain-a" / "hidden.jsonl",
            "style3-db": CORPUS / "round-1v" / "heldout-github-style3" / "hidden.jsonl"}


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def prompts() -> None:
    import gemini  # noqa: E402  (round 1p's prompt)
    d = OUT / "prompts"
    for name, src in COPIED.items():
        (d / name).mkdir(parents=True, exist_ok=True)
        for p in sorted(src.glob("db-*.txt")):
            shutil.copyfile(p, d / name / p.name)
    frozen = json.load(open(CORPUS / "round-1v" / "frozen.json", encoding="utf-8"))
    for name, src in RENDERED.items():
        assert hashlib.sha256(src.read_bytes()).hexdigest() == frozen[name[:-3]], f"{name} is not the frozen set"
        (d / name).mkdir(parents=True, exist_ok=True)
        for line in open(src, encoding="utf-8"):
            s = json.loads(line)
            if s["fk"]:
                (d / name / f"{gemini.db_stem(s['id'])}.txt").write_text(
                    gemini.DB_ASK.format(schema=gemini.render(s)), encoding="utf-8")
    with open(d / "hashes.txt", "w", encoding="utf-8", newline="\n") as f:
        for name in (*COPIED, *RENDERED):
            files = sorted((d / name).glob("*.txt"))
            for p in files:
                f.write(f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {name}/{p.name}\n")
            print(f"{name}: {len(files)} prompts, {sum(p.stat().st_size for p in files)} bytes")


def call(method: str, path: str, body: dict | None = None) -> dict:
    req = urllib.request.Request(API + path, method=method,
                                 data=json.dumps(body).encode() if body is not None else None,
                                 headers={"Content-Type": "application/json",
                                          "Authorization": f"Bearer {os.environ['OPENROUTER_API_KEY']}"})
    try:
        with urllib.request.urlopen(req, timeout=600) as r:
            return json.loads(r.read())
    except urllib.error.HTTPError as e:
        raise SystemExit(f"{method} {path}: {e.code} {e.read().decode(errors='replace')[:2000]}")


def takes_temperature(model: str) -> bool:
    """Whether the model's page lists the parameter (the public list of models, no key)."""
    with urllib.request.urlopen(f"{API}/models", timeout=120) as r:
        models = {m["id"]: m for m in json.loads(r.read())["data"]}
    return "temperature" in (models[model].get("supported_parameters") or [])


def folder(model: str) -> str:
    return model.split("/")[-1]


def submit(model: str, which: str, first: int | None) -> None:
    src = OUT / "prompts" / which
    dst = OUT / "llm" / folder(model) / which
    asked = set()
    for b in (OUT / "batches").glob("*.json"):
        rec = json.load(open(b, encoding="utf-8"))
        if rec["model"] == model and rec["set"] == which and rec.get("status") not in ("failed", "expired", "cancelled"):
            asked.update(rec["stems"])
    todo = [p for p in sorted(src.glob("*.txt"))[:first]
            if p.stem not in asked and not ((dst / f"{p.stem}.raw").exists() and (dst / f"{p.stem}.raw").stat().st_size)]
    temperature = takes_temperature(model)
    (OUT / "batches").mkdir(parents=True, exist_ok=True)
    for i in range(0, len(todo), CHUNK):
        part = todo[i:i + CHUNK]
        requests = []
        for p in part:
            body = {"messages": [{"role": "user", "content": p.read_text(encoding="utf-8")}], "max_tokens": MAX_TOKENS}
            if temperature:
                body["temperature"] = TEMPERATURE
            requests.append({"custom_id": p.stem, "body": body})
        # the endpoint and the model before the requests: the body is read as a stream
        resp = call("POST", "/batches", {"endpoint": "/v1/chat/completions", "model": model,
                                         "completion_window": "24h", "requests": requests})
        rec = {"id": resp["id"], "model": model, "set": which, "stems": [p.stem for p in part],
               "temperature": TEMPERATURE if temperature else None, "max tokens": MAX_TOKENS,
               "submitted": now(), "status": resp.get("status"), "provider": resp.get("provider")}
        (OUT / "batches" / f"{resp['id']}.json").write_text(json.dumps(rec, indent=1) + "\n", encoding="utf-8")
        print(f"{model} {which}: {len(part)} prompts in {resp['id']} ({resp.get('status')}, {resp.get('provider')})",
              flush=True)


def feed(queue: list, keep: float) -> None:
    """Submit the queue a chunk at a time. OpenRouter reserves a batch's
    worst case at submit (every reply at MAX_TOKENS), and the balance is
    shared with other work: a chunk is sent only while the reserves of the
    open batches and its own stay at `keep` dollars or less. `queue` is
    model, set and chunk size, in turn."""
    import time
    with urllib.request.urlopen(f"{API}/models", timeout=120) as r:
        price = {m["id"]: float(m["pricing"]["completion"]) for m in json.loads(r.read())["data"]}
    jobs = [(queue[i], queue[i + 1], int(queue[i + 2])) for i in range(0, len(queue), 3)]
    per = lambda model: price.get(model + ":batch", price[model] / 2)  # noqa: E731
    global CHUNK
    while True:
        collect()
        left = []
        for model, which, size in jobs:
            src = OUT / "prompts" / which
            dst = OUT / "llm" / folder(model) / which
            asked = set()
            for b in (OUT / "batches").glob("*.json"):
                rec = json.load(open(b, encoding="utf-8"))
                if rec["model"] == model and rec["set"] == which and rec.get("status") not in ("failed", "expired", "cancelled"):
                    asked.update(rec["stems"])
            todo = [p for p in sorted(src.glob("*.txt")) if p.stem not in asked
                    and not ((dst / f"{p.stem}.raw").exists() and (dst / f"{p.stem}.raw").stat().st_size)]
            if not todo:
                continue
            left.append((model, which))
            n = min(size, len(todo))
            reserve = n * MAX_TOKENS * per(model) * 1.05
            # The reserves of the batches still open are summed here, from the records: the
            # balance the API reports lags a submit by minutes, and on 14 November six chunks
            # sent in twelve seconds against it reserved $26.75 of $18.36.
            held = 0.0
            for b in (OUT / "batches").glob("*.json"):
                rec = json.load(open(b, encoding="utf-8"))
                if not rec.get("collected") and rec.get("status") not in ("failed", "expired", "cancelled"):
                    held += len(rec["stems"]) * MAX_TOKENS * per(rec["model"]) * 1.05
            if held + reserve <= keep:
                first = sorted(src.glob("*.txt")).index(todo[n - 1]) + 1
                CHUNK = size
                print(f"{now()} held ${held:.2f}, reserve ${reserve:.2f}: {model} {which} {n}", flush=True)
                submit(model, which, first)
        open_ = [b for b in (OUT / "batches").glob("*.json") if not json.load(open(b, encoding="utf-8")).get("collected")
                 and json.load(open(b, encoding="utf-8")).get("status") not in ("failed", "expired", "cancelled")]
        if not left and not open_:
            print(f"{now()} the queue is empty and every batch collected", flush=True)
            return
        time.sleep(150)


def sync(model: str, which: str, workers: int) -> None:
    """The prompts of a set no batch holds and no reply answers, asked a
    request at a time, `workers` in flight: the same body as a batch's
    request, at the full price and with no reserve on the balance. The
    owner allowed it for GPT-6.1 Terra alone, on 14 November 2026."""
    import time
    from concurrent.futures import ThreadPoolExecutor
    src = OUT / "prompts" / which
    dst = OUT / "llm" / folder(model) / which
    dst.mkdir(parents=True, exist_ok=True)
    asked = set()
    for b in (OUT / "batches").glob("*.json"):
        rec = json.load(open(b, encoding="utf-8"))
        if rec["model"] == model and rec["set"] == which and rec.get("status") not in ("failed", "expired", "cancelled"):
            asked.update(rec["stems"])
    if os.environ.get("ASK_WHAT_A_BATCH_HOLDS"):  # a batch that does not end: its prompts asked here too
        asked = set()
    todo = [p for p in sorted(src.glob("*.txt")) if p.stem not in asked
            and not ((dst / f"{p.stem}.raw").exists() and (dst / f"{p.stem}.raw").stat().st_size)]
    temperature = takes_temperature(model)
    key = os.environ["OPENROUTER_API_KEY"]
    stop = []

    def one(p: Path) -> str:
        if stop:
            return f"{p.stem}: not asked, the balance"
        body = {"model": model, "messages": [{"role": "user", "content": p.read_text(encoding="utf-8")}],
                "max_tokens": MAX_TOKENS}
        if temperature:
            body["temperature"] = TEMPERATURE
        errors, started = [], now()
        for attempt in range(6):
            if attempt:
                time.sleep(min(5 * 2 ** (attempt - 1), 120))
            t0 = time.perf_counter()
            try:
                req = urllib.request.Request(f"{API}/chat/completions", data=json.dumps(body).encode(), method="POST",
                                             headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
                with urllib.request.urlopen(req, timeout=900) as r:
                    resp = json.loads(r.read())
                choice = resp["choices"][0]
                text = (choice.get("message") or {}).get("content") or ""
                if not text:
                    raise ValueError(f"an empty reply, finish {choice.get('finish_reason')}")
            except urllib.error.HTTPError as e:
                errors.append(f"{e.code} {e.read().decode(errors='replace')[:200]}")
                if e.code == 402:
                    stop.append(1)
                    return f"{p.stem}: 402, the balance"
                continue
            except Exception as e:  # noqa: BLE001  (a reset, a timeout, a reply cut short: asked again)
                errors.append(repr(e)[:200])
                continue
            usage = resp.get("usage") or {}
            (dst / f"{p.stem}.raw").write_text(text, encoding="utf-8")
            (dst / f"{p.stem}.json").write_text(json.dumps({
                "prompt": p.name, "set": which, "asked": model, "provider": "openrouter, a request at a time",
                "served by": resp.get("provider"), "finish": choice.get("finish_reason"),
                "answered by": resp.get("model"), "usage": usage,
                "input tokens": usage.get("prompt_tokens"), "output tokens": usage.get("completion_tokens"),
                "reasoning tokens": (usage.get("completion_tokens_details") or {}).get("reasoning_tokens"),
                "cached tokens": (usage.get("prompt_tokens_details") or {}).get("cached_tokens"),
                "cost": usage.get("cost"), "temperature": TEMPERATURE if temperature else None,
                "max tokens": MAX_TOKENS, "seconds": time.perf_counter() - t0, "attempts": attempt + 1,
                "errors": errors, "started": started, "ended": now(), "in flight": workers}, indent=1) + "\n",
                encoding="utf-8")
            return ""
        return f"{p.stem}: {errors[-1] if errors else 'unanswered'}"

    print(f"{model} {which}: {len(todo)} prompts to ask, {workers} in flight", flush=True)
    with ThreadPoolExecutor(workers) as pool:
        failed = [f for f in pool.map(one, todo) if f]
    cost = sum((json.load(open(dst / f"{p.stem}.json", encoding="utf-8")).get("cost") or 0.0)
               for p in todo if (dst / f"{p.stem}.json").exists())
    print(f"{model} {which}: {len(todo) - len(failed)} answered, {len(failed)} not, ${cost:.4f}", flush=True)
    for f in failed[:10]:
        print("  " + f[:300])


def collect() -> None:
    for path in sorted((OUT / "batches").glob("*.json")):
        rec = json.load(open(path, encoding="utf-8"))
        if rec.get("collected"):
            continue
        b = call("GET", f"/batches/{rec['id']}")
        rec.update(status=b.get("status"), provider=b.get("provider"), request_counts=b.get("request_counts"),
                   usage=b.get("usage"), error=b.get("error"), checked=now())
        if b.get("status") == "completed":
            dst = OUT / "llm" / folder(rec["model"]) / rec["set"]
            dst.mkdir(parents=True, exist_ok=True)
            failed = []
            for item in b.get("results") or []:
                stem = item["custom_id"]
                body = (item.get("response") or {}).get("body") or {}
                choices = body.get("choices") or []
                if item.get("error") or not choices:
                    failed.append({"stem": stem, "error": item.get("error") or body.get("error")})
                    continue
                msg = choices[0].get("message") or {}
                usage = body.get("usage") or {}
                text = msg.get("content") or ""
                (dst / f"{stem}.raw").write_text(text, encoding="utf-8")
                (dst / f"{stem}.json").write_text(json.dumps({
                    "prompt": f"{stem}.txt", "set": rec["set"], "asked": rec["model"], "provider": "openrouter, batch",
                    "served by": b.get("provider"), "batch": rec["id"], "finish": choices[0].get("finish_reason"),
                    "answered by": body.get("model"), "usage": usage,
                    "input tokens": usage.get("prompt_tokens"), "output tokens": usage.get("completion_tokens"),
                    "reasoning tokens": (usage.get("completion_tokens_details") or {}).get("reasoning_tokens"),
                    "cached tokens": (usage.get("prompt_tokens_details") or {}).get("cached_tokens"),
                    "temperature": rec["temperature"], "max tokens": rec["max tokens"],
                    "submitted": rec["submitted"], "collected": now()}, indent=1) + "\n", encoding="utf-8")
            rec["failed"] = failed
            rec["collected"] = now()
        path.write_text(json.dumps(rec, indent=1) + "\n", encoding="utf-8")
        cost = (rec.get("usage") or {}).get("cost")
        print(f"{rec['model']:38} {rec['set']:11} {len(rec['stems']):4d} {rec['status']:11} {rec.get('request_counts')} "
              f"cost {cost} failed {len(rec.get('failed') or [])}", flush=True)


def spent() -> None:
    total = {}
    for path in sorted((OUT / "batches").glob("*.json")):
        rec = json.load(open(path, encoding="utf-8"))
        u = rec.get("usage") or {}
        k = (rec["model"], rec["set"])
        t = total.setdefault(k, {"prompts": 0, "cost": 0.0, "input": 0, "output": 0, "open": 0})
        t["prompts"] += len(rec["stems"])
        t["cost"] += u.get("cost") or 0.0
        t["input"] += u.get("prompt_tokens") or 0
        t["output"] += u.get("completion_tokens") or 0
        t["open"] += not rec.get("collected") and rec.get("status") not in ("failed", "expired", "cancelled")
    for (m, s), t in sorted(total.items()):
        print(f"{m:38} {s:11} {t['prompts']:4d} prompts  in {t['input']:8d} out {t['output']:8d}  ${t['cost']:.4f}"
              + (f"  ({t['open']} batches open)" if t["open"] else ""))
    print(f"in all ${sum(t['cost'] for t in total.values()):.4f}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "prompts":
        prompts()
    elif cmd == "submit":
        submit(sys.argv[2], sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else None)
    elif cmd == "collect":
        collect()
    elif cmd == "sync":
        sync(sys.argv[2], sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else 4)
    elif cmd == "feed":
        feed(sys.argv[3:], float(sys.argv[2]))
    else:
        spent()
