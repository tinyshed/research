"""Round 1z, part B: a local language model asked about whole schemas through Ollama.

    python -I local.py ask <tag> <set> [--no-reasoning] [--sample] [--first <n>]
    python -I local.py sample                    plain A's sample of 180, written once

<set> is a folder of round 1w's prompts (corpus/round-1w/lm/prompts/<set>/,
checked against its hashes.txt): `style3-db`, `plain-a-db`, `db`,
`heldout-db`, `fresh-db`. One user message a prompt, no system prompt,
temperature 0, seed 0, the tag's own chat template, reasoning left at the
runtime's default. The window is the smallest of WINDOWS that holds the
prompt twice (one token estimated for 2.5 bytes); the output limit is
32 768 tokens or what the window leaves, whichever is smaller. The runtime
is told neither to cut a prompt nor to slide the window, so a prompt that
does not fit is refused and a reply that fills the window stops.

A reply is kept as it came: <stem>.raw the message's content, <stem>.json
the runtime's whole answer beside what was asked. A prompt that does not fit
gets a record and an empty .raw. The run can be stopped and started again;
it asks only what has no record.

While it asks, the graphics card's memory (nvidia-smi) and the runtime's
resident memory (its processes, by name) are read every two seconds; their
highest go into <set>/run-<time>.json with what `ollama ps` says of the
loaded model.
"""

import hashlib
import json
import math
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
CORPUS = HERE.parent / "corpus"
PROMPTS = CORPUS / "round-1w" / "lm" / "prompts"
OUT = CORPUS / "round-1z" / "llm"
STOP = CORPUS / "round-1z" / "stop"
HOST = "http://127.0.0.1:11435"
WINDOWS = (16384, 32768, 65536)
MAX_TOKENS = 32768
BYTES_A_TOKEN = 2.5
SAMPLE_SEED = 20261126
BANDS = [(5, 10), (10, 20), (20, 40), (40, 100), (100, 10 ** 6)]


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def call(path: str, body: dict | None = None, timeout: float = 7200) -> dict:
    req = urllib.request.Request(HOST + path, data=json.dumps(body).encode() if body is not None else None,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read())


def folder(tag: str, reasoning: bool = True) -> str:
    return tag.replace(":", "--") + ("" if reasoning else "--no-reasoning")


def db_stem(sid: str) -> str:
    return "db-" + hashlib.sha1(sid.encode()).hexdigest()[:16]


def sample() -> list:
    """Plain A's sample as the README fixed it: a quarter of each band, rounded up."""
    import numpy as np
    path = CORPUS / "round-1z" / "sample-plain-a.json"
    if path.exists():
        return json.load(open(path, encoding="utf-8"))["stems"]
    schemas = [json.loads(line) for line in open(CORPUS / "round-1v" / "heldout-github-plain-a" / "hidden.jsonl",
                                                 encoding="utf-8")]
    stems, bands = [], {}
    for b, (lo, hi) in enumerate(BANDS):
        ids = sorted(s["id"] for s in schemas if lo <= len(s["tables"]) < hi and s["fk"])
        order = np.random.default_rng(SAMPLE_SEED + b).permutation(len(ids))
        take = [ids[i] for i in order[:math.ceil(len(ids) / 4)]]
        bands[f"{lo}-{hi - 1}" if hi < 10 ** 6 else f"{lo}+"] = {"schemas": len(ids), "taken": len(take)}
        stems += [db_stem(i) for i in take]
    path.write_text(json.dumps({"seed": SAMPLE_SEED, "written": now(), "bands": bands, "stems": sorted(stems)},
                               indent=1) + "\n", encoding="utf-8")
    print(json.dumps(bands))
    return sorted(stems)


class Watch(threading.Thread):
    """The card's memory and the runtime's resident memory, every two seconds."""

    def __init__(self):
        super().__init__(daemon=True)
        self.card = self.host = 0
        self.card_before = self.read_card()
        self.stop = False

    @staticmethod
    def read_card() -> int:
        try:
            out = subprocess.run(["nvidia-smi", "--query-gpu=memory.used", "--format=csv,noheader,nounits"],
                                 capture_output=True, text=True, timeout=20).stdout
            return int(out.strip().splitlines()[0])
        except Exception:
            return 0

    @staticmethod
    def read_host() -> int:
        import psutil
        total = 0
        for p in psutil.process_iter(["name", "memory_info"]):
            try:
                if (p.info["name"] or "").lower().startswith(("ollama", "llama")):
                    total += p.info["memory_info"].rss
            except Exception:
                pass
        return total

    def run(self):
        while not self.stop:
            self.card = max(self.card, self.read_card())
            self.host = max(self.host, self.read_host())
            time.sleep(2)


def ask(tag: str, which: str, only: list | None, first: int | None, reasoning: bool = True) -> None:
    if STOP.exists():
        raise SystemExit("paused: corpus/round-1z/stop exists")
    src = PROMPTS / which
    hashes = dict(line.split()[::-1] for line in open(PROMPTS / "hashes.txt", encoding="utf-8"))
    files = sorted(src.glob("db-*.txt"))
    if only is not None:
        files = [p for p in files if p.stem in set(only)]
        assert len(files) == len(only), "a prompt of the sample is missing"
    files = files[:first]
    for p in files:
        assert hashlib.sha256(p.read_bytes()).hexdigest() == hashes[f"{which}/{p.name}"], f"{p.name} is not round 1w's prompt"
    dst = OUT / folder(tag, reasoning) / which
    dst.mkdir(parents=True, exist_ok=True)
    listed = {m["name"]: m for m in call("/api/tags")["models"]}[tag]
    version = call("/api/version")["version"]
    todo = [p for p in files if not (dst / f"{p.stem}.json").exists()]
    # by window, then by size: the runtime loads the model again when the window changes
    plan = []
    for p in todo:
        est = math.ceil(p.stat().st_size / BYTES_A_TOKEN)
        # the smallest window that holds the prompt twice; the largest, if the prompt alone fits it
        window = next((w for w in WINDOWS if w >= 2 * est), WINDOWS[-1] if est <= WINDOWS[-1] else None)
        plan.append((window or 10 ** 9, p.stat().st_size, p, est))
    plan.sort(key=lambda x: (x[0], x[1], x[2].name))
    # whatever the runtime still holds is let go first, so the card's reading before is without a model
    for m in call("/api/ps").get("models", []):
        call("/api/chat", {"model": m["name"], "messages": [], "keep_alive": 0}, timeout=120)
    time.sleep(3)
    watch = Watch()
    watch.start()
    t0 = time.time()
    loaded = {}
    print(f"{tag}{'' if reasoning else ', reasoning off'} {which}: {len(todo)} of {len(files)} prompts to ask, the card held {watch.card_before} MiB before", flush=True)
    done = 0
    try:
        for window, _, p, est in plan:
            if STOP.exists():  # the round is paused: what is asked stays, the rest is asked by the next run
                print("  stopped by corpus/round-1z/stop", flush=True)
                break
            record = {"prompt": p.name, "set": which, "asked": tag, "digest": listed["digest"], "details": listed["details"],
                      "runtime": f"ollama {version}", "estimated prompt tokens": est, "asked at": now()}
            if window == 10 ** 9:
                record.update(finish="does not fit", window=None)
                (dst / f"{p.stem}.raw").write_text("", encoding="utf-8")
                (dst / f"{p.stem}.json").write_text(json.dumps(record, indent=1) + "\n", encoding="utf-8")
                continue
            options = {"temperature": 0, "seed": 0, "num_ctx": window, "num_predict": min(MAX_TOKENS, window - est)}
            body = {"model": tag, "messages": [{"role": "user", "content": p.read_text(encoding="utf-8")}],
                    "stream": False, "options": options, "truncate": False, "shift": False, "keep_alive": "10m"}
            if not reasoning:
                body["think"] = False
            record.update(window=window, options=options, reasoning="the tag's default" if reasoning else "off")
            reply, error = None, None
            for attempt in range(3):
                try:
                    t1 = time.time()
                    reply = call("/api/chat", body)
                    record["wall seconds"] = time.time() - t1
                    if (reply.get("message") or {}).get("content") or (reply.get("message") or {}).get("thinking") \
                            or reply.get("done_reason") == "length":
                        break
                    error, reply = "an empty reply", None
                except urllib.error.HTTPError as e:
                    error = f"{e.code} {e.read().decode(errors='replace')[:500]}"
                    if e.code == 400 and ("exceed" in error or "too long" in error or "context" in error):
                        break  # the runtime refuses a prompt longer than the window
                except Exception as e:  # noqa: BLE001
                    error = repr(e)[:500]
                record["attempts"] = attempt + 1
            if reply is None:
                fits = not (error and error.startswith("400"))
                record.update(finish="failed" if fits else "does not fit", error=error)
                if fits:  # a failure is not an answer: nothing is kept, the next run asks again
                    print(f"  {p.stem}: failed, {error}", flush=True)
                    continue
                (dst / f"{p.stem}.raw").write_text("", encoding="utf-8")
            else:
                msg = reply.get("message") or {}
                (dst / f"{p.stem}.raw").write_text(msg.get("content") or "", encoding="utf-8")
                record.update(finish=reply.get("done_reason"), answer=reply,
                              **{"input tokens": reply.get("prompt_eval_count"), "output tokens": reply.get("eval_count"),
                                 "seconds": (reply.get("total_duration") or 0) / 1e9,
                                 "seconds, prompt": (reply.get("prompt_eval_duration") or 0) / 1e9,
                                 "seconds, reply": (reply.get("eval_duration") or 0) / 1e9,
                                 "seconds, load": (reply.get("load_duration") or 0) / 1e9,
                                 "reasoning characters": len(msg.get("thinking") or "")})
                if window not in loaded:
                    try:
                        loaded[window] = [m for m in call("/api/ps")["models"] if m["name"] == tag][0]
                    except Exception:  # noqa: BLE001
                        loaded[window] = None
            (dst / f"{p.stem}.json").write_text(json.dumps(record, indent=1) + "\n", encoding="utf-8")
            done += 1
            if done % 10 == 0 or window > WINDOWS[0]:
                print(f"  {done}/{len(plan)} {p.stem} window {window} in {record.get('input tokens')} out "
                      f"{record.get('output tokens')} {record.get('seconds', 0):.0f} s {record['finish']}"
                      f"  ({time.time() - t0:.0f} s)", flush=True)
    finally:
        watch.stop = True
        try:
            call("/api/chat", {"model": tag, "messages": [], "keep_alive": 0}, timeout=120)  # the card is given back
        except Exception:  # noqa: BLE001
            pass
        run = {"tag": tag, "reasoning": "the tag's default" if reasoning else "off", "set": which, "digest": listed["digest"], "bytes": listed["size"], "details": listed["details"],
               "runtime": f"ollama {version}", "started": datetime.fromtimestamp(t0, timezone.utc).isoformat(timespec="seconds"),
               "ended": now(), "asked": done, "seconds": time.time() - t0,
               "card MiB before": watch.card_before, "card MiB at most": watch.card,
               "runtime resident bytes at most": watch.host,
               "loaded, by window": {str(k): ({"size": v.get("size"), "size_vram": v.get("size_vram"),
                                              "context_length": v.get("context_length")} if v else None)
                                     for k, v in loaded.items()}}
        (dst / f"run-{datetime.now(timezone.utc):%Y%m%dT%H%M%S}.json").write_text(json.dumps(run, indent=1) + "\n",
                                                                                  encoding="utf-8")
        print(json.dumps(run), flush=True)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "sample":
        print(len(sample()), "schemas")
    else:
        args = sys.argv[2:]
        first = int(args[args.index("--first") + 1]) if "--first" in args else None
        ask(args[0], args[1], sample() if "--sample" in args else None, first, "--no-reasoning" not in args)
