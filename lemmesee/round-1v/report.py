"""Round 1v: the report's raw output gathered beside it.

    python -I report.py <reports/data/keys-size-...>

Writes, from the ignored corpus, what the report's figures were read from:

    fits.json                every first and second stage's options, seed, threshold and validation figures
    development.json         every scoring's measures on every development set, the frozen models' beside them
    development-per-database.json   the per-database counts of the models frozen for the clean sets
    choice.json, guard.json  the rule of choice and its guard, as choose.py and guard.py read them
    teachers.json            the teachers' requests: asked, answered, tokens, seconds, and their answers against the declarations
    teachers-<model>.jsonl.gz   every reply as it came, with its record
    taught.json, taught-more.json, declared-otherwise.json   what the fits were taught from, by index
    judge-<name>-count.json, judge-<name>-pairs.json         the blind judge's packets' key with its answers
    frozen.json, heldout-<set>.json, <set>-summary.json, <set>-manifest.json, <set>-sha256.txt   the clean sets and their one reading
    questions.json           see's questions through the keys (questions.py)

sizes.py and thin.py write sizes.json and thinning.json themselves. No
schema's text is written: a set is its manifest of repositories, commits and
paths, and its hashes.
"""

import gzip
import json
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "round-1p"))

import numpy as np  # noqa: E402

import gemini  # noqa: E402
import teach  # noqa: E402
import whole  # noqa: E402

OUT = whole.OUT


def fits() -> dict:
    out = {"first": {}, "second": {}}
    for p in sorted((OUT / "first").glob("*/first.json")):
        if not p.parent.name.endswith("-small"):
            out["first"][p.parent.name] = json.load(open(p, encoding="utf-8"))
    for p in sorted((OUT / "second").glob("*/second.json")):
        if "-small" in p.parent.name:
            continue
        r = json.load(open(p, encoding="utf-8"))
        r["importance, the ten largest shares"] = {
            k: round(v / max(sum(r["importance"].values()), 1), 4) for k, v in list(r["importance"].items())[:10]}
        r.pop("importance")
        r.pop("features")
        out["second"][p.parent.name] = r
    return out


def development(frozen: list) -> tuple:
    dev, per = {}, {}
    for p in sorted((OUT / "scored").glob("*.json")):
        r = json.load(open(p, encoding="utf-8"))
        name = p.stem
        dev[name] = {"threshold": r["threshold"], "sets": {s: e["measures"] for s, e in r["sets"].items()}}
        for s, e in r["sets"].items():
            for fm in ("1s", "seven-bounded", "nine-mended"):
                if fm in e:
                    dev.setdefault(fm, {"sets": {}})["sets"].setdefault(s, e[fm])
        if name in frozen:
            per[name] = {s: e["per database"] for s, e in r["sets"].items()}
    return dev, per


def teachers(data: Path) -> dict:
    out = {}
    for model in teach.TEACHERS:
        with gzip.open(data / f"teachers-{model}.jsonl.gz", "wt", encoding="utf-8", compresslevel=9) as z:
            for which in teach.SETS:
                d = teach.OUT / "llm" / model / which
                if not d.exists():
                    continue
                index = json.load(open(teach.OUT / "prompts" / f"{which}.index.json", encoding="utf-8"))
                records, failed = [], 0
                for stem, sid in sorted(index.items()):
                    raw, rec = d / f"{stem}.raw", d / f"{stem}.json"
                    if not raw.exists():
                        failed += (d / f"{stem}.failed.json").exists()
                        continue
                    r = json.load(open(rec, encoding="utf-8"))
                    records.append(r)
                    z.write(json.dumps({"set": which, "schema": sid, "reply": raw.read_text(encoding="utf-8"),
                                        **{k: r.get(k) for k in ("answered by", "input tokens", "output tokens",
                                                                 "reasoning tokens", "seconds", "attempts", "errors",
                                                                 "started", "ended", "temperature", "max tokens",
                                                                 "in flight")}}, ensure_ascii=False) + "\n")
                if not records and not failed:
                    continue
                seconds = sorted(r["seconds"] for r in records)
                e = {"prompts": len(index), "answered": len(records), "refused by the proxy's limit": failed,
                     "input tokens": sum(r.get("input tokens") or 0 for r in records),
                     "output tokens": sum(r.get("output tokens") or 0 for r in records),
                     "reasoning tokens": sum(r.get("reasoning tokens") or 0 for r in records),
                     "seconds, median": seconds[len(seconds) // 2] if seconds else None,
                     "seconds, 90th percentile": seconds[int(len(seconds) * 0.9)] if seconds else None,
                     "started": min((r["started"] for r in records), default=None),
                     "ended": max((r["ended"] for r in records), default=None)}
                path = teach.OUT / f"{model}--{which}.jsonl"
                if path.exists():
                    taught = {}
                    for line in open(path, encoding="utf-8"):
                        r = json.loads(line)
                        taught[r["id"]] = {tuple(k) for k in r["keys"]}
                    tp = fp = fn = 0
                    for s in teach.chosen(which):
                        if s["id"] in taught:
                            truth = {tuple(k) for k in s["fk"]}
                            tp += len(truth & taught[s["id"]])
                            fp += len(taught[s["id"]] - truth)
                            fn += len(truth - taught[s["id"]])
                    e["against the declarations"] = {"schemas": len(taught), "precision": tp / max(tp + fp, 1),
                                                     "recall": tp / max(tp + fn, 1), "declared keys": tp + fn,
                                                     "keys no declaration names": fp}
                out.setdefault(model, {})[which] = e
    return out


def judge(name: str, data: Path) -> None:
    d = OUT / f"judge-{name}"
    if not (d / "count.json").exists():
        return
    shutil.copyfile(d / "count.json", data / f"judge-{name}-count.json")
    key = json.load(open(d / "key.json", encoding="utf-8"))
    answers = {}
    for f in sorted((d / "answers").glob("packet-*.json")):
        answers.update({k: str(v).strip().lower() for k, v in json.load(open(f, encoding="utf-8")).items()})
    for pid, v in key.items():
        v["answer"] = answers.get(pid)
    (data / f"judge-{name}-pairs.json").write_text(json.dumps(key, indent=0, ensure_ascii=False) + "\n", encoding="utf-8")


def main(data: Path) -> None:
    import choose  # noqa: E402
    import guard  # noqa: E402
    data.mkdir(parents=True, exist_ok=True)
    frozen = json.load(open(OUT / "frozen.json", encoding="utf-8")) if (OUT / "frozen.json").exists() else {"models": {}}
    (data / "fits.json").write_text(json.dumps(fits(), indent=1) + "\n", encoding="utf-8")
    dev, per = development(list(frozen["models"]))
    (data / "development.json").write_text(json.dumps(dev, indent=1) + "\n", encoding="utf-8")
    (data / "development-per-database.json").write_text(json.dumps(per) + "\n", encoding="utf-8")
    names = [p.stem for p in sorted((OUT / "scored").glob("*.json"))]
    out, choice = choose.main([n for n in names if "+" not in n])
    blends, _ = choose.main([n for n in names if "+" in n]) if any("+" in n for n in names) else ({}, None)
    (data / "choice.json").write_text(json.dumps({"the rule's choice among single models": choice, "models": out,
                                                  "blends, beside": {k: v for k, v in blends.items() if "+" in k}},
                                                 indent=1) + "\n", encoding="utf-8")
    (data / "guard.json").write_text(json.dumps(guard.main(names), indent=1) + "\n", encoding="utf-8")
    (data / "teachers.json").write_text(json.dumps(teachers(data), indent=1) + "\n", encoding="utf-8")
    for f in ("taught.json", "taught-more.json", "declared-otherwise.json"):
        shutil.copyfile(teach.OUT / f, data / f)
    for d in sorted(OUT.glob("judge-*")):
        judge(d.name[len("judge-"):], data)
    for p in sorted(OUT.glob("questions-*.json")):
        shutil.copyfile(p, data / "questions.json")
    if (OUT / "frozen.json").exists():
        shutil.copyfile(OUT / "frozen.json", data / "frozen.json")
    for d in sorted(OUT.glob("heldout-github-*")):
        which = d.name[len("heldout-github-"):]
        shutil.copyfile(d / "summary.json", data / f"{which}-summary.json")
        # a set is published once it is read: one kept clean keeps its repositories to itself, its schemas' hash aside
        if (OUT / f"heldout-{which}.json").exists():
            shutil.copyfile(d / "sha256.txt", data / f"{which}-sha256.txt")
            shutil.copyfile(d / "manifest.json", data / f"{which}-manifest.json")
            shutil.copyfile(OUT / f"heldout-{which}.json", data / f"heldout-{which}.json")
        else:
            first = open(d / "sha256.txt", encoding="utf-8").readline()
            (data / f"{which}-sha256.txt").write_text(first, encoding="utf-8")
    print(sorted(p.name for p in data.iterdir()))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
