"""Judges outside Claude: each packet cut by database, so a prompt fits a
command line, asked to answer in its reply rather than in a file, and the
answers gathered into one file a packet, as the other judges'. Gemini 3.8
Flash, through `agy` and then a CLIProxyAPI in front of it; DeepSeek V4.1
Flash and GLM-5.3 Flash, through OpenRouter.

    python -I gemini.py cut <look dir>
    JUDGE_KEY=... python -I gemini.py ask <look dir> <endpoint, …/v1> <model> <judge> [<at once>] [--packets=1,2]
    python -I gemini.py gather <look dir> <judge>

cut writes <look dir>/gemini/<packet>-<k>.txt; ask keeps each reply beside
it, <same>.raw for Gemini and <same>.<judge>.raw for another; gather writes
<judge>-<n>.json.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ASK = (
    "Reply with only one JSON object and no other text: "
    '{"panels": {"<panel id>": {"keep": true or false, "meaning": "yes" or "no" or "cannot tell", '
    '"wrong": a few words or null}, ...}} — every panel id below must be answered. '
    "Do not read, search or open any file, and run no command."
)


def cut(look: Path) -> None:
    out = look / "gemini"
    out.mkdir(exist_ok=True)
    n = 0
    for packet in sorted(look.glob("packet-*.txt"), key=lambda p: int(p.stem.split("-")[1])):
        text = packet.read_text(encoding="utf-8")
        head, *sections = re.split(r"\n(?==== database: )", text)
        # the form, without its instruction to write a file
        form = head.split("\nWrite one JSON object")[0]
        form = form.replace("Do not read, search or open any file other than this one, and run no\n"
                            "command; when done, write your answers with the Write tool, once, to the path given to you.", "")
        for k, section in enumerate(sections, 1):
            (out / f"{packet.stem}-{k}.txt").write_text(f"{form.strip()}\n\n{ASK}\n\n{section.strip()}\n", encoding="utf-8")
            n += 1
    print(f"{n} prompts")


def raw_of(prompt: Path, judge: str) -> Path:
    return prompt.with_suffix(".raw") if judge == "gemini" else prompt.with_name(f"{prompt.stem}.{judge}.raw")


def ask(look: Path, url: str, model: str, judge: str = "gemini", workers: int = 8, packets: set[str] | None = None) -> None:
    """Every prompt this judge has not answered, through an OpenAI-style
    endpoint, several at once; only these packets' when they are named, as a
    judge that counts answers only the databases held out. The key comes
    from the environment, JUDGE_KEY, and is never written."""
    import os
    import urllib.request
    from concurrent.futures import ThreadPoolExecutor

    key = os.environ.get("JUDGE_KEY") or os.environ["CLIPROXY_KEY"]
    todo = []
    for p in sorted((look / "gemini").glob("*.txt")):
        if packets and p.stem.split("-")[1] not in packets:
            continue
        raw = raw_of(p, judge)
        if not (raw.exists() and '"panels"' in raw.read_text(encoding="utf-8", errors="replace")):
            todo.append(p)

    def one(p: Path) -> str:
        body = json.dumps({"model": model, "messages": [{"role": "user", "content": p.read_text(encoding="utf-8")}],
                           "temperature": 0}).encode()
        for _ in range(3):
            req = urllib.request.Request(f"{url}/chat/completions", data=body, method="POST",
                                         headers={"Content-Type": "application/json", "Authorization": f"Bearer {key}"})
            try:
                with urllib.request.urlopen(req, timeout=600) as r:
                    text = json.loads(r.read())["choices"][0]["message"]["content"]
            except Exception as e:  # noqa: BLE001 — a failed request is asked again, then reported
                err = str(e)[:120]
                continue
            if not text:
                # GLM answered once with no content at all
                err = "an empty reply"
                continue
            raw_of(p, judge).write_text(text, encoding="utf-8")
            if '"panels"' in text:
                return f"{p.name} ok"
            err = "no panels in the reply"
        return f"{p.name} failed: {err}"

    with ThreadPoolExecutor(workers) as pool:
        for line in pool.map(one, todo):
            print(line, flush=True)


def gather(look: Path, judge: str = "gemini") -> None:
    by_packet: dict[str, dict] = {}
    bad = []
    raws = [r for r in (look / "gemini").glob("*.raw") if (r.name.count(".") == 1 if judge == "gemini" else r.name.endswith(f".{judge}.raw"))]
    for raw in sorted(raws):
        text = raw.read_text(encoding="utf-8", errors="replace")
        # the last whole answer: a reply may hold two, as `agy` wrote twice
        panels = None
        decoder = json.JSONDecoder()
        for m in re.finditer(r"\{\s*\"panels\"", text):
            try:
                obj, _ = decoder.raw_decode(text, m.start())
                panels = obj["panels"]
            except (json.JSONDecodeError, KeyError):
                continue
        if panels is None:
            bad.append(raw.name)
            continue
        packet = raw.name.split(".")[0].rsplit("-", 1)[0]
        by_packet.setdefault(packet, {}).update(panels)
    for packet, panels in by_packet.items():
        n = packet.split("-")[1]
        (look / f"{judge}-{n}.json").write_text(json.dumps({"panels": panels}, ensure_ascii=False), encoding="utf-8")
    print({p: len(v) for p, v in sorted(by_packet.items())}, "unreadable:", bad)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "ask":
        packets = next((set(a.split("=")[1].split(",")) for a in sys.argv if a.startswith("--packets=")), None)
        args = [a for a in sys.argv if not a.startswith("--")]
        ask(Path(args[2]), args[3], args[4], args[5] if len(args) > 5 else "gemini",
            int(args[6]) if len(args) > 6 else 8, packets)
    elif sys.argv[1] == "gather":
        gather(Path(sys.argv[2]), sys.argv[3] if len(sys.argv) > 3 else "gemini")
    else:
        cut(Path(sys.argv[2]))
