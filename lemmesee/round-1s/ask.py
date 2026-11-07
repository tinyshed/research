"""Round 1s: the language models asked under one protocol, every request
recorded.

    python -I ask.py prompts <round 1p out> <round 1q cascade out> <out>
    python -I ask.py cascade <models> <round 1p out> <out>
    CLIPROXY_KEY=... python -I ask.py run cliproxy <endpoint, …/v1> <model> <out> <set> [<at once>]
    ANTHROPIC_API_KEY=... python -I ask.py run anthropic - <model> <out> <set> [<at once>]

`prompts` copies round 1p's pair and whole-schema prompts and round 1q's
cascade prompts, cut from `both, roles`, into <out>/prompts/<set>/ and writes
their SHA-256 into <out>/prompts/hashes.txt. `cascade` cuts the same
cascade from the refit `both` (round 1q's cut, its band and question text)
into <out>/prompts/cascade-both/. `run` asks every prompt of a set not yet
answered and keeps, in <out>/llm/<model>/<set>/, each reply as it came
(`<stem>.raw`) and its record (`<stem>.json`): the model the response names,
the tokens it counts, the time from sending to the whole body, the attempts
and their errors, the finish reason, the times in UTC. Keys come from the
environment and are never printed or written.
"""

import hashlib
import json
import os
import shutil
import sys
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

TEMPERATURE = 0
MAX_TOKENS = 32768
ATTEMPTS = 8
SETS = ("pair", "db", "cascade-roles", "cascade-both")


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="milliseconds")


def prompts(round_1p: Path, cascade_roles: Path, out: Path) -> None:
    d = out / "prompts"
    for s in ("pair", "db", "cascade-roles"):
        (d / s).mkdir(parents=True, exist_ok=True)
    for p in sorted((round_1p / "gemini").glob("pair-*.txt")):
        shutil.copyfile(p, d / "pair" / p.name)
    for p in sorted((round_1p / "gemini").glob("db-*.txt")):
        shutil.copyfile(p, d / "db" / p.name)
    for p in sorted((cascade_roles / "prompts").glob("*.txt")):
        shutil.copyfile(p, d / "cascade-roles" / p.name)
    shutil.copyfile(cascade_roles / "questions.jsonl", d / "cascade-roles" / "questions.jsonl")
    shutil.copyfile(cascade_roles / "sure.jsonl", d / "cascade-roles" / "sure.jsonl")
    hashes(d)


def hashes(d: Path) -> None:
    with open(d / "hashes.txt", "w", encoding="utf-8") as f:
        for s in SETS:
            for p in sorted((d / s).glob("*")) if (d / s).exists() else []:
                f.write(f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {s}/{p.name}\n")
    for s in SETS:
        n = len(list((d / s).glob("*.txt"))) if (d / s).exists() else 0
        print(f"{s}: {n} prompts")


def cascade(models: Path, round_1p: Path, out: Path) -> None:
    import letters  # noqa: F401, E402  (the refit's features: the trigrams sorted)
    sys.path.insert(0, str(HERE.parent / "round-1q"))
    import cascade as cut  # noqa: E402  (round 1q's)
    d = out / "prompts" / "cascade-both"
    cut.cut(models, round_1p, "both", d)
    for p in (d / "prompts").glob("*.txt"):
        p.replace(d / p.name)
    (d / "prompts").rmdir()
    hashes(out / "prompts")


def ask_cliproxy(url: str, key: str, model: str, text: str) -> dict:
    body = json.dumps({"model": model, "messages": [{"role": "user", "content": text}],
                       "temperature": TEMPERATURE, "max_tokens": MAX_TOKENS}).encode()
    req = urllib.request.Request(f"{url}/chat/completions", data=body, method="POST",
                                 headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
    with urllib.request.urlopen(req, timeout=900) as r:
        resp = json.loads(r.read())
    choice = resp["choices"][0]
    usage = resp.get("usage") or {}
    return {"text": choice["message"].get("content") or "", "finish": choice.get("finish_reason"),
            "answered by": resp.get("model"), "usage": usage,
            "input tokens": usage.get("prompt_tokens"), "output tokens": usage.get("completion_tokens"),
            "reasoning tokens": (usage.get("completion_tokens_details") or {}).get("reasoning_tokens"),
            "cached tokens": (usage.get("prompt_tokens_details") or {}).get("cached_tokens"),
            "temperature": TEMPERATURE}


def ask_anthropic(client, model: str, text: str) -> dict:
    # Claude Haiku 5.5 refuses a temperature other than its default, so none
    # is sent; thinking and effort are its defaults
    with client.messages.stream(model=model, max_tokens=MAX_TOKENS,
                                messages=[{"role": "user", "content": text}]) as stream:
        msg = stream.get_final_message()
    usage = msg.usage.model_dump() if hasattr(msg.usage, "model_dump") else dict(msg.usage)
    return {"text": "".join(b.text for b in msg.content if getattr(b, "type", "") == "text"),
            "finish": msg.stop_reason, "answered by": msg.model, "usage": usage,
            "input tokens": usage.get("input_tokens"), "output tokens": usage.get("output_tokens"),
            "reasoning tokens": (usage.get("output_tokens_details") or {}).get("thinking_tokens"),
            "cached tokens": usage.get("cache_read_input_tokens"),
            "temperature": None}


def run(provider: str, url: str, model: str, out: Path, which: str, workers: int) -> None:
    src = out / "prompts" / which
    dst = out / "llm" / model / which
    dst.mkdir(parents=True, exist_ok=True)
    todo = [p for p in sorted(src.glob("*.txt"))
            if not ((dst / f"{p.stem}.raw").exists() and (dst / f"{p.stem}.raw").stat().st_size > 0)]
    if provider == "cliproxy":
        key = os.environ["CLIPROXY_KEY"]
        call = lambda text: ask_cliproxy(url, key, model, text)  # noqa: E731
    else:
        import anthropic  # noqa: E402
        client = anthropic.Anthropic(max_retries=0)  # retries are counted here
        call = lambda text: ask_anthropic(client, model, text)  # noqa: E731

    def one(p: Path) -> str:
        text = p.read_text(encoding="utf-8")
        errors, started = [], now()
        for attempt in range(ATTEMPTS):
            if attempt:
                # A refusal for too many requests waits longer each time
                time.sleep(min(5 * 2 ** (attempt - 1), 120) if errors and "429" in errors[-1] else 2)
            t0 = time.perf_counter()
            try:
                r = call(text)
            except Exception as e:  # noqa: BLE001 — a failed request is asked again, then reported
                errors.append(f"{type(e).__name__}: {str(e)[:160]}")
                continue
            seconds = time.perf_counter() - t0
            if not r["text"]:
                errors.append(f"an empty reply ({r['finish']})")
                continue
            reply_text = r.pop("text")
            (dst / f"{p.stem}.raw").write_text(reply_text, encoding="utf-8")
            record = {"prompt": p.name, "set": which, "prompt characters": len(text), "asked": model,
                      "provider": provider, **r, "max tokens": MAX_TOKENS, "seconds": seconds,
                      "attempts": attempt + 1, "errors": errors, "started": started, "ended": now(),
                      "in flight": workers}
            (dst / f"{p.stem}.json").write_text(json.dumps(record, indent=1) + "\n", encoding="utf-8")
            return f"{p.name} ok {seconds:.1f} s"
        (dst / f"{p.stem}.failed.json").write_text(json.dumps({"prompt": p.name, "errors": errors,
                                                               "started": started, "ended": now()}, indent=1),
                                                   encoding="utf-8")
        return f"{p.name} failed: {errors[-1] if errors else ''}"

    print(f"{model} {which}: {len(todo)} to ask", flush=True)
    with ThreadPoolExecutor(workers) as pool:
        for line in pool.map(one, todo):
            print(line, flush=True)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    a = sys.argv
    if a[1] == "prompts":
        prompts(Path(a[2]), Path(a[3]), Path(a[4]))
    elif a[1] == "cascade":
        cascade(Path(a[2]), Path(a[3]), Path(a[4]))
    else:
        run(a[2], a[3], a[4], Path(a[5]), a[6], int(a[7]) if len(a) > 7 else 4)
