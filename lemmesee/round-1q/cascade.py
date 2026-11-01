"""Round 1q's cascade: the key model draws the keys it is sure of and asks a
language model only about the candidates it is unsure of.

    .venv/Scripts/python -I cascade.py cut <models> <round 1p out> <variant> <out>
    CLIPROXY_KEY=... .venv/Scripts/python -I cascade.py ask <out> <endpoint, …/v1> <model> [<at once>]
    .venv/Scripts/python -I cascade.py measure <round 1p out> <out> <model>

`cut` scores round 1p's databases, keeps each column's best target, and
writes into <out>: `sure.jsonl`, the keys at or above the model's threshold;
`questions.jsonl`, the keys in [LOW, threshold); and `prompts/<db>.txt`, one
prompt a database with questions, its schema written as round 1p's
whole-schema prompt writes it. `ask` keeps each reply in `replies/<model>/`.
`measure` writes cascade-<model>.json into <out>: F1 over every one-column key
declared, the questions asked, and the prompt characters against round 1p's
whole-schema prompts.
"""

import json
import time
import os
import sys
from collections import defaultdict
from pathlib import Path

ROUND_1P = Path(__file__).resolve().parent.parent / "round-1p"
sys.path.insert(0, str(ROUND_1P))

import numpy as np  # noqa: E402

import gemini  # noqa: E402

LOW = 0.2  # below it a candidate is neither drawn nor asked, fixed before the run

ASK = (
    "Below is a relational database's schema with its foreign key declarations removed: each table with its "
    "columns, their types, NOT NULL and UNIQUE where declared, and its primary key.\n\n{schema}\n\n"
    "For each numbered question, say whether the column refers to rows of that table, as a foreign key would. "
    'Reply with only one JSON object and no other text: {{"answers": {{"1": true or false, ...}}}}, every '
    "number answered.\n\n{questions}"
)


def cut(models: Path, data: Path, variant: str, out: Path) -> None:
    import external  # noqa: E402
    import keys  # noqa: E402
    import widen  # noqa: E402
    run, graph = external.scorer(models, lambda job: widen.build((*job, variant)))
    threshold = json.load(open(models / "graph-final.json", encoding="utf-8"))["threshold"]
    schemas = [json.loads(line) for line in open(data / "hidden.jsonl", encoding="utf-8")]
    by_id = {s["id"]: s for s in schemas}
    scores, meta = run(schemas, {})
    kept = keys.one_target(scores, [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
    (out / "prompts").mkdir(parents=True, exist_ok=True)
    asked = defaultdict(list)
    with open(out / "sure.jsonl", "w", encoding="utf-8") as sure:
        for i in np.flatnonzero(kept >= LOW):
            m = meta[i]
            row = [m[graph.SID], m[graph.TABLE], m[graph.COLUMN], m[graph.TARGET], float(kept[i])]
            if kept[i] >= threshold:
                sure.write(json.dumps(row, ensure_ascii=False) + "\n")
            else:
                asked[m[graph.SID]].append(row)
    with open(out / "questions.jsonl", "w", encoding="utf-8") as q:
        for sid, rows in asked.items():
            lines = [f'{n}. Does "{t}"."{c}" refer to rows of "{r}"?' for n, (_, t, c, r, _) in enumerate(rows, 1)]
            text = ASK.format(schema=gemini.render(by_id[sid]), questions="\n".join(lines))
            (out / "prompts" / f"{gemini.db_stem(sid)}.txt").write_text(text, encoding="utf-8")
            for n, row in enumerate(rows, 1):
                q.write(json.dumps([n] + row, ensure_ascii=False) + "\n")
    print(f"threshold {threshold:.3f}; sure {int((kept >= threshold).sum())}; "
          f"questions {sum(len(v) for v in asked.values())} in {len(asked)} databases")


def ask(out: Path, url: str, model: str, workers: int = 8) -> None:
    import urllib.request
    from concurrent.futures import ThreadPoolExecutor
    key = os.environ["CLIPROXY_KEY"]
    replies = out / "replies" / model
    replies.mkdir(parents=True, exist_ok=True)
    todo = [p for p in sorted((out / "prompts").glob("*.txt"))
            if not ((replies / f"{p.stem}.raw").exists() and (replies / f"{p.stem}.raw").stat().st_size > 0)]

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
            if text:
                (replies / f"{p.stem}.raw").write_text(text, encoding="utf-8")
                return f"{p.name} ok"
            err = "an empty reply"
        return f"{p.name} failed: {err}"

    with ThreadPoolExecutor(workers) as pool:
        for line in pool.map(one, todo):
            print(line, flush=True)


def measure(data: Path, out: Path, model: str) -> None:
    schemas = [json.loads(line) for line in open(data / "hidden.jsonl", encoding="utf-8")]
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["fk"]}
    source = {s["id"]: s["source"] for s in schemas}
    sure, cascade = defaultdict(set), defaultdict(set)
    for line in open(out / "sure.jsonl", encoding="utf-8"):
        sid, t, c, r, _ = json.loads(line)
        sure[sid].add((t, c, r))
        cascade[sid].add((t, c, r))
    questions = defaultdict(list)
    for line in open(out / "questions.jsonl", encoding="utf-8"):
        n, sid, t, c, r, _ = json.loads(line)
        questions[sid].append((n, (t, c, r)))
    unread, yes = 0, 0
    for sid, qs in questions.items():
        raw = out / "replies" / model / f"{gemini.db_stem(sid)}.raw"
        obj = gemini.last_object(raw.read_text(encoding="utf-8", errors="replace"), "answers") if raw.exists() else None
        if obj is None:
            unread += 1
            continue
        answers = {str(k): v for k, v in (obj.get("answers") or {}).items()}
        for n, key in qs:
            if answers.get(str(n)) is True:
                cascade[sid].add(key)
                yes += 1
    prompt_chars = sum(len(p.read_text(encoding="utf-8")) for p in (out / "prompts").glob("*.txt"))
    whole_chars = sum(len(p.read_text(encoding="utf-8")) for p in (data / "gemini").glob("db-*.txt"))
    result = {"questions": sum(len(v) for v in questions.values()), "answered yes": yes,
              "databases asked": len(questions), "replies unreadable": unread,
              "prompt characters": prompt_chars, "round 1p's whole-schema prompt characters": whole_chars}
    for name in ("spider", "bird", "ctu", "all"):
        ids = [i for i in truth if name in ("all", source[i])]
        for how, preds in (("model alone", sure), ("cascade", cascade)):
            result[f"{name}, {how}"] = dict(zip(("precision", "recall", "f1"),
                                               gemini.prf(*gemini.open_counts(truth, preds, ids))))
    (out / f"cascade-{model}.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "cut":
        cut(Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4], Path(sys.argv[5]))
    elif sys.argv[1] == "ask":
        ask(Path(sys.argv[2]), sys.argv[3], sys.argv[4], int(sys.argv[5]) if len(sys.argv) > 5 else 8)
    else:
        measure(Path(sys.argv[2]), Path(sys.argv[3]), sys.argv[4])
