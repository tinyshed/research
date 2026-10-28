"""A teacher on the local graphics card: JevK5, an open-weight model of typed
decisions, read through llama.cpp's llama-server, asked exactly what round
1d asks Jev of each panel alone (keep, meaning), answering each with a
probability.

    python -I local.py look <look dir> [<at once>] [--fast]   # writes <look dir>/jevk5[fast]-<n>.json
    python -I local.py jsonl <in.jsonl> <out.jsonl> [<at once>] [--fast]
    python -I local.py ceilings <out dir> [<source>...]   # round 1g's ceilings, these sources beside the others

`jsonl` reads one state a line, {"id": ..., "state": {...}}, and appends one
answer a line; run again, it skips the ids already answered, so it may be
stopped at any time. Nothing is generated: the server evaluates the prompt
and the probabilities of the answer letters A (true) and B (false) are read
at the answer position, as JevK5's own llama.cpp client reads them. The
prompt is JevK5 v0.3's (Qwen3.5's chat template, thinking off).
"""

from __future__ import annotations

import importlib.util
import json
import math
import os
import re
import sys
import time
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
URL = os.environ.get("LOCAL_URL", "http://127.0.0.1:8090")
NAME = "jevk5"
FAST = "jevk5fast"
# the temperature JevK5-GGUF's card gives jevk5-4b-v0.3-Q8_0
TEMPERATURE = float(os.environ.get("LOCAL_TEMPERATURE", "1.22"))
SYSTEM = ("Apply the supplied criterion to the supplied evidence. Choose exactly one listed option. "
          "Respond with only its uppercase letter, with no explanation or reasoning.")
TEMPLATE = ("<|im_start|>system\n{system}<|im_end|>\n"
            "<|im_start|>user\n{user}<|im_end|>\n"
            "<|im_start|>assistant\n<think>\n\n</think>\n\n")


def module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


jev = module("jev", HERE.parent / "round-1d" / "jev.py")
QUESTIONS = jev.QUESTIONS


def post(path: str, payload: dict) -> dict:
    req = urllib.request.Request(URL + path, data=json.dumps(payload).encode(),
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=600) as r:
        return json.loads(r.read())


def prompt(state: dict, question: dict) -> str:
    crit = question.get("criteria") or {}
    options = [f"{k}: {crit.get(k) or f'The proposition is {k}.'}" for k in ("true", "false")]
    user = json.dumps({"evidence": state, "criterion": question["instructions"],
                       "options": [{"letter": "AB"[i], "description": d} for i, d in enumerate(options)]},
                      ensure_ascii=False)
    return TEMPLATE.format(system=SYSTEM, user=user)


def noul(state: dict, question: dict) -> tuple[float, int]:
    """The probability of true, and the tokens the prompt took."""
    # tokenised with special tokens parsed: /completion would read <|im_start|> as plain text
    tokens = post("/tokenize", {"content": prompt(state, question), "add_special": False,
                                "parse_special": True})["tokens"]
    # the hybrid model's state cannot be cut back to a shared prefix, so nothing is cached
    out = post("/completion", {"prompt": tokens, "n_predict": 1, "n_probs": 20, "temperature": 0,
                               "cache_prompt": False})
    seen = {e["token"]: e["logprob"] for e in out["completion_probabilities"][0]["top_logprobs"]}
    floor = min(seen.values(), default=0.0) - 2.0  # a letter outside the top sits below the last
    a, b = seen.get("A", floor), seen.get("B", floor)
    top = max(a, b)
    wa, wb = math.exp((a - top) / TEMPERATURE), math.exp((b - top) / TEMPERATURE)
    return wa / (wa + wb), len(tokens)


def number(m: re.Match) -> str:
    x = float(m.group())
    return f"{x:.2f}".rstrip("0").rstrip(".") if abs(x) >= 1 else f"{x:.3g}"


def compact(state: dict) -> dict:
    """The same result in fewer tokens: no indentation, decimals cut to two
    places or three significant digits. Integers, labels and dates stay."""
    rows = "\n".join(line.strip() for line in state.get("result", "").splitlines())
    return {**state, "result": re.sub(r"-?\d+\.\d+(?:e[+-]?\d+)?", number, rows)}


def decide(states: list[dict], pool: ThreadPoolExecutor, fast: bool = False) -> list[dict]:
    """Every question of every state, each a request of its own, so that the
    server's slots stay full. Fast asks keep alone, of the compact result:
    the look learns keep only."""
    names = ("keep",) if fast else ("keep", "meaning")
    asked = [(compact(s) if fast else s, QUESTIONS[q]) for s in states for q in names]
    got = list(pool.map(lambda sq: noul(*sq), asked))
    out = []
    for at in range(0, len(got), len(names)):
        (keep, n1), *rest = got[at:at + len(names)]
        a = {"keep": keep >= 0.5, "wrong": None, "p_keep": round(keep, 4), "tokens": n1}
        if rest:
            meaning, n2 = rest[0]
            a |= {"meaning": "yes" if meaning >= 0.5 else "no", "p_meaning": round(meaning, 4)}
            a["tokens"] += n2
        out.append(a)
    return out


def look(look_dir: Path, at_once: int, fast: bool = False) -> None:
    items = list(jev.panels(look_dir))
    t0, tokens = time.perf_counter(), 0
    by_packet: dict[str, dict] = {}
    with ThreadPoolExecutor(at_once) as pool:
        for (n, name, _), a in zip(items, decide([it[2] for it in items], pool, fast)):
            tokens += a.pop("tokens")
            by_packet.setdefault(n, {})[name] = a
    for n, ans in by_packet.items():
        (look_dir / f"{FAST if fast else NAME}-{n}.json").write_text(json.dumps({"panels": ans}, ensure_ascii=False),
                                                                     encoding="utf-8")
    dt = time.perf_counter() - t0
    print(f"{look_dir}: {len(items)} panels in {dt:.1f} s, {len(items) / dt:.1f} a second, "
          f"{tokens / len(items):.0f} tokens a panel, {tokens / dt:.0f} tokens a second", flush=True)


def jsonl(src: Path, dst: Path, at_once: int, fast: bool = False) -> None:
    done = set()
    if dst.exists():
        done = {json.loads(line)["id"] for line in dst.read_text(encoding="utf-8").splitlines() if line.strip()}
    todo = [json.loads(line) for line in src.read_text(encoding="utf-8").splitlines() if line.strip()]
    todo = [x for x in todo if x["id"] not in done]
    print(f"{len(done)} answered, {len(todo)} to go", flush=True)
    t0, count = time.perf_counter(), 0
    with dst.open("a", encoding="utf-8") as out, ThreadPoolExecutor(at_once) as pool:
        for at in range(0, len(todo), 1000):
            chunk = todo[at:at + 1000]
            for x, a in zip(chunk, decide([x["state"] for x in chunk], pool, fast)):
                out.write(json.dumps({"id": x["id"], **a}, ensure_ascii=False) + "\n")
            out.flush()
            count += len(chunk)
            dt = time.perf_counter() - t0
            print(f"{count} in {dt:.0f} s, {count / dt:.1f} a second", flush=True)


def ceilings(out: Path, names: tuple[str, ...] = (NAME, FAST)) -> None:
    """Round 1g's ceilings, with each named source beside the teachers; then
    the teachers' mean with each in Jev's place."""
    sys.argv = [sys.argv[0]]  # diagnose reads its own arguments only under __main__
    diagnose = module("diagnose", HERE.parent / "round-1g" / "diagnose.py")
    corpus = HERE.parent / "corpus"
    dirs = [corpus / "round-1d" / "look", corpus / "round-1e" / "look", corpus / "round-1f" / "look"]
    out.mkdir(parents=True, exist_ok=True)
    teachers = diagnose.look1e.TEACHERS
    diagnose.TEACHERS = tuple(teachers) + tuple(names)
    diagnose.ceilings(dirs, corpus / "see", out)
    (out / "ceilings.json").replace(out / "ceilings-beside.json")
    for local in names:
        diagnose.look1e.TEACHERS = tuple(local if t == "jev" else t for t in teachers)
        diagnose.ceilings(dirs, corpus / "see", out)
        (out / "ceilings.json").replace(out / f"ceilings-{local}-for-jev.json")
        diagnose.look1e.TEACHERS = tuple(teachers) + (local,)
        diagnose.ceilings(dirs, corpus / "see", out)
        (out / "ceilings.json").replace(out / f"ceilings-{local}-added.json")
    diagnose.look1e.TEACHERS = teachers


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    fast = "--fast" in sys.argv
    cmd, *args = [a for a in sys.argv[1:] if a != "--fast"]
    if cmd == "look":
        look(Path(args[0]), int(args[1]) if len(args) > 1 else 32, fast)
    elif cmd == "jsonl":
        jsonl(Path(args[0]), Path(args[1]), int(args[2]) if len(args) > 2 else 32, fast)
    elif cmd == "ceilings":
        ceilings(Path(args[0]), tuple(args[1:]) or (NAME, FAST))
