"""Round 1w: Claude models asked through Anthropic's Message Batches API,
every request recorded.

    ANTHROPIC_API_KEY=... python -I batch_anthropic.py submit <model> <set> [<first n>]
    ANTHROPIC_API_KEY=... python -I batch_anthropic.py collect
    python -I batch_anthropic.py spent

Round 1s's protocol for a Claude model (ask.py's `ask_anthropic`), unchanged
but for the road: one user message a database, no system prompt, at most
32 768 output tokens, no temperature (the models refuse one other than their
default), thinking and effort at the model's defaults; a batch in place of a
request at a time, at half the price. The prompts are batch.py's. Replies go
to corpus/round-1w/lm/llm/<model>/<set>/ as round 1s kept them; a batch's
record to corpus/round-1w/lm/batches-anthropic/. The key comes from the
environment and is never printed or written; the client is pointed at
Anthropic's own address whatever the environment's base URL says.
"""

import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

import anthropic
from anthropic.types.message_create_params import MessageCreateParamsNonStreaming
from anthropic.types.messages.batch_create_params import Request

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "corpus" / "round-1w" / "lm"
MAX_TOKENS = 32768
# list price a million tokens, input and output; a batch is charged half
PRICE = {"claude-sonnet-5-5": (2.00, 10.00), "claude-haiku-5-5": (0.10, 0.50)}


def now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def client() -> anthropic.Anthropic:
    return anthropic.Anthropic(api_key=os.environ["ANTHROPIC_API_KEY"].strip(), base_url="https://api.anthropic.com")


def records() -> list:
    return sorted((OUT / "batches-anthropic").glob("*.json"))


def submit(model: str, which: str, first: int | None) -> None:
    src = OUT / "prompts" / which
    dst = OUT / "llm" / model / which
    asked = set()
    for b in records():
        rec = json.load(open(b, encoding="utf-8"))
        if rec["model"] == model and rec["set"] == which:
            asked.update(s for s in rec["stems"] if s not in rec.get("failed stems", ()))
    todo = [p for p in sorted(src.glob("*.txt"))[:first]
            if p.stem not in asked and not ((dst / f"{p.stem}.raw").exists() and (dst / f"{p.stem}.raw").stat().st_size)]
    if not todo:
        print("nothing to ask")
        return
    batch = client().messages.batches.create(requests=[
        Request(custom_id=p.stem, params=MessageCreateParamsNonStreaming(
            model=model, max_tokens=MAX_TOKENS, messages=[{"role": "user", "content": p.read_text(encoding="utf-8")}]))
        for p in todo])
    (OUT / "batches-anthropic").mkdir(parents=True, exist_ok=True)
    rec = {"id": batch.id, "model": model, "set": which, "stems": [p.stem for p in todo], "temperature": None,
           "max tokens": MAX_TOKENS, "submitted": now(), "status": batch.processing_status}
    (OUT / "batches-anthropic" / f"{batch.id}.json").write_text(json.dumps(rec, indent=1) + "\n", encoding="utf-8")
    print(f"{model} {which}: {len(todo)} prompts in {batch.id} ({batch.processing_status})", flush=True)


def collect() -> None:
    c = client()
    for path in records():
        rec = json.load(open(path, encoding="utf-8"))
        if rec.get("collected"):
            continue
        b = c.messages.batches.retrieve(rec["id"])
        rec.update(status=b.processing_status, request_counts=b.request_counts.model_dump(), checked=now())
        if b.processing_status == "ended":
            dst = OUT / "llm" / rec["model"] / rec["set"]
            dst.mkdir(parents=True, exist_ok=True)
            failed, tokens = [], [0, 0]
            for r in c.messages.batches.results(rec["id"]):
                if r.result.type != "succeeded":
                    failed.append(r.custom_id)
                    continue
                msg = r.result.message
                usage = msg.usage.model_dump()
                text = "".join(x.text for x in msg.content if getattr(x, "type", "") == "text")
                (dst / f"{r.custom_id}.raw").write_text(text, encoding="utf-8")
                (dst / f"{r.custom_id}.json").write_text(json.dumps({
                    "prompt": f"{r.custom_id}.txt", "set": rec["set"], "asked": rec["model"],
                    "provider": "anthropic, batch", "batch": rec["id"], "finish": msg.stop_reason,
                    "answered by": msg.model, "usage": usage,
                    "input tokens": usage.get("input_tokens"), "output tokens": usage.get("output_tokens"),
                    "reasoning tokens": (usage.get("output_tokens_details") or {}).get("thinking_tokens"),
                    "cached tokens": usage.get("cache_read_input_tokens"), "temperature": None,
                    "max tokens": rec["max tokens"], "submitted": rec["submitted"], "collected": now()},
                    indent=1, default=str) + "\n", encoding="utf-8")
                tokens[0] += usage.get("input_tokens") or 0
                tokens[1] += usage.get("output_tokens") or 0
            pin, pout = PRICE[rec["model"]]
            rec.update({"failed stems": failed, "input tokens": tokens[0], "output tokens": tokens[1],
                        "cost at half the list price": (tokens[0] * pin + tokens[1] * pout) / 2e6, "collected": now()})
        path.write_text(json.dumps(rec, indent=1) + "\n", encoding="utf-8")
        print(f"{rec['model']:20} {rec['set']:11} {len(rec['stems']):4d} {rec['status']:12} {rec['request_counts']} "
              f"cost {rec.get('cost at half the list price')} failed {len(rec.get('failed stems') or [])}", flush=True)


def spent() -> None:
    total = 0.0
    for path in records():
        rec = json.load(open(path, encoding="utf-8"))
        cost = rec.get("cost at half the list price")
        total += cost or 0.0
        print(f"{rec['model']:20} {rec['set']:11} {len(rec['stems']):4d} prompts  in {rec.get('input tokens')} "
              f"out {rec.get('output tokens')}  {'$%.4f' % cost if cost is not None else rec['status']}")
    print(f"in all ${total:.4f}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd = sys.argv[1]
    if cmd == "submit":
        submit(sys.argv[2], sys.argv[3], int(sys.argv[4]) if len(sys.argv) > 4 else None)
    elif cmd == "collect":
        collect()
    else:
        spent()
