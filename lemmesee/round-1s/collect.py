"""Round 1s: the raw output the report cites, copied beside it.

    python -I collect.py <out> <report data>

copies the measures' JSON from <out> into <report data>, and writes each
model's replies to each prompt set as one JSON line a prompt — the prompt's
file name, the reply as it came, its record — as `replies-<model>-<set>.jsonl`.
Nothing it writes holds a path of this machine.
"""

import json
import shutil
import sys
from pathlib import Path


def main(out: Path, dst: Path) -> None:
    dst.mkdir(parents=True, exist_ok=True)
    plain = {
        "check.json": out / "check.json",
        "summary.json": out / "summary.json",
        "lm-summary.json": out / "lm-summary.json",
        "frozen.json": out / "frozen.json",
        "parity.json": out / "parity" / "parity.json",
        "prompt-hashes.txt": out / "prompts" / "hashes.txt",
        "heldout-summary.json": out / "heldout" / "summary.json",
        "heldout-result.json": out / "heldout" / "frozen-result.json",
        "heldout-score.json": out / "heldout" / "score-frozen.json",
        "heldout-schema.sha256": out / "heldout" / "schema.sha256",
        "heldout-revision.txt": out / "heldout" / "revision.txt",
        "heldout-lm.json": out / "heldout" / "lm.json",
        "heldout-prompt-hashes.txt": out / "prompts" / "heldout-hashes.txt",
    }
    for name, src in plain.items():
        if src.exists():
            shutil.copyfile(src, dst / name)
    for d in sorted((out / "models").iterdir()):
        for f in ("graph-final.json", "pack.json"):
            if (d / f).exists():
                shutil.copyfile(d / f, dst / f"fit-{d.name}-{f}")
    for f in sorted((out / "scored").glob("*.json")):
        shutil.copyfile(f, dst / f"scored-{f.name}")
    for f in sorted((out / "external").glob("score-*.json")):
        shutil.copyfile(f, dst / f"external-{f.name}")
    for f in sorted((out / "bench").glob("*")):
        if f.name.startswith(("bench-", "memory-", "environment")):
            shutil.copyfile(f, dst / f"rust-{f.name}")
    for model in sorted((out / "llm").iterdir()):
        for s in sorted(model.iterdir()):
            rows = []
            for raw in sorted(s.glob("*.raw")):
                rec = s / f"{raw.stem}.json"
                rows.append({"prompt": f"{raw.stem}.txt", "reply": raw.read_text(encoding="utf-8"),
                             "record": json.load(open(rec, encoding="utf-8")) if rec.exists() else None})
            failed = [json.load(open(f, encoding="utf-8")) for f in sorted(s.glob("*.failed.json"))
                      if not (s / f"{f.name.split('.')[0]}.raw").exists()]
            with open(dst / f"replies-{model.name}-{s.name}.jsonl", "w", encoding="utf-8") as w:
                for r in rows:
                    w.write(json.dumps(r, ensure_ascii=False) + "\n")
                for r in failed:
                    w.write(json.dumps({"prompt": r["prompt"], "reply": None, "unanswered": r}, ensure_ascii=False)
                            + "\n")
    print(f"{len(list(dst.iterdir()))} files in {dst.name}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]))
