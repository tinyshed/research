"""Round 1s: the language models' answers measured, beside the refit `both`.

    python -I lm.py <out> <round 1p out> <round 1q out>

reads <out>/llm/<model>/<set>/ as ask.py wrote it, round 1p's databases and
pairs from <out>/external/, the refit's keys from
<out>/external/open-predictions-both.jsonl, and the earlier rounds' replies
for the same prompts, and writes <out>/lm-summary.json: over every declared
key P / R / F1 with intervals and the refit minus each model; the macro F1;
the pair task; each cascade; tokens, latency and price a schema.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1p"))
sys.path.insert(0, str(HERE))

import gemini  # noqa: E402  (round 1p's reader of replies)
from refit import EXTERNAL_SEED, interval, resampled  # noqa: E402

MODELS = ["gemini-3.5-flash-lite", "gemini-3-flash", "gemini-3.8-flash-high", "claude-haiku-5-5"]
# USD a million tokens, input and output (thinking included), standard paid
# tier, read on 7 November 2026: Google AI Studio's pricing page (Gemini 3
# Flash as `gemini-3-flash-preview`, Gemini 3.8 Flash's price until 31
# December 2026) and Anthropic's (Claude Haiku 5.5 up to 100 000 prompt
# tokens, 0.50 / 2.50 beyond)
PRICE = {"gemini-3.5-flash-lite": (0.30, 2.50), "gemini-3-flash": (0.50, 3.00),
         "gemini-3.8-flash-high": (0.75, 3.75), "claude-haiku-5-5": (0.10, 0.50)}
LONG_PROMPT = {"claude-haiku-5-5": (100_000, 0.50, 2.50)}
SOURCES = ("spider", "bird", "ctu")


def prf(tp, fp, fn):
    return gemini.prf(int(tp), int(fp), int(fn))


def whole_schema(schemas, raws: Path) -> tuple[dict, dict]:
    """Each database's keys as a reply names them, round 1p's reading:
    names matched case-insensitively, a key naming nothing of the schema a
    false one, one key a column."""
    preds, info = {}, {"unanswered": 0, "unreadable": 0, "keys naming nothing": 0}
    for s in schemas:
        if not s["fk"]:
            continue
        raw = raws / f"{gemini.db_stem(s['id'])}.raw"
        if not raw.exists():
            info["unanswered"] += 1
            preds[s["id"]] = set()
            continue
        obj = gemini.last_object(raw.read_text(encoding="utf-8", errors="replace"), "keys")
        if obj is None:
            info["unreadable"] += 1
            preds[s["id"]] = set()
            continue
        names = {t["name"].lower(): t for t in s["tables"]}
        out, seen = set(), set()
        for e in obj.get("keys") or []:
            if not isinstance(e, dict):
                continue
            t = names.get(str(e.get("table", "")).lower())
            to = names.get(str(e.get("references", "")).lower())
            col = next((c["name"] for c in t["columns"] if c["name"].lower() == str(e.get("column", "")).lower()),
                       None) if t else None
            if not (t and to and col):
                info["keys naming nothing"] += 1
                out.add(("?", json.dumps(e, sort_keys=True), ""))
                continue
            if (t["name"], col) in seen:
                continue
            seen.add((t["name"], col))
            out.add((t["name"], col, to["name"]))
        preds[s["id"]] = out
    return preds, info


def counts(truth, preds, ids):
    return np.array([[len(truth[i] & preds.get(i, set())), len(preds.get(i, set()) - truth[i]),
                      len(truth[i] - preds.get(i, set()))] for i in ids], dtype=np.int64)


def pair_task(bench, raws: Path):
    rows, unanswered, unreadable = [], 0, 0
    for p in bench:
        if p["occurs_in_perm"] or p["too_large"]:
            continue
        raw = raws / f"{gemini.pair_stem(p)}.raw"
        obj = None
        if raw.exists():
            obj = gemini.last_object(raw.read_text(encoding="utf-8", errors="replace"), "column")
            unreadable += obj is None
        else:
            unanswered += 1
        got = [str((obj or {}).get(k, "")).lower() for k in ("table", "column", "referencedTable", "referencedColumn")]
        want = [p["table"].lower(), p["column"].lower(), p["foreign_table"].lower(), p["referred_column"].lower()]
        rows.append((p["source"], p["db"], got == want))
    out = {"unanswered": unanswered, "unreadable": unreadable}
    for src in SOURCES:
        r = [x for x in rows if x[0] == src]
        dbs = sorted({x[1] for x in r})
        at = {d: k for k, d in enumerate(dbs)}
        per = np.zeros((len(dbs), 2))
        for _, d, hit in r:
            per[at[d]] += [hit, 1]
        idx = np.random.default_rng(EXTERNAL_SEED).integers(0, len(dbs), (1000, len(dbs)))
        s = per[idx].sum(axis=1)
        out[src] = {"pairs": len(r), "recall_at_1": float(per[:, 0].sum() / per[:, 1].sum()),
                    "95": interval(s[:, 0] / s[:, 1])}
    return out


def cascade(d: Path, raws: Path):
    sure, questions = defaultdict(set), defaultdict(list)
    for line in open(d / "sure.jsonl", encoding="utf-8"):
        sid, t, c, r, _ = json.loads(line)
        sure[sid].add((t, c, r))
    for line in open(d / "questions.jsonl", encoding="utf-8"):
        n, sid, t, c, r, _ = json.loads(line)
        questions[sid].append((n, (t, c, r)))
    both = {k: set(v) for k, v in sure.items()}
    info = {"databases asked": len(questions), "questions": sum(len(v) for v in questions.values()),
            "unanswered": 0, "unreadable": 0, "yes": 0}
    for sid, qs in questions.items():
        raw = raws / f"{gemini.db_stem(sid)}.raw"
        if not raw.exists():
            info["unanswered"] += 1
            continue
        obj = gemini.last_object(raw.read_text(encoding="utf-8", errors="replace"), "answers")
        if obj is None:
            info["unreadable"] += 1
            continue
        answers = {str(k): v for k, v in (obj.get("answers") or {}).items()}
        for n, key in qs:
            if answers.get(str(n)) is True:
                both.setdefault(sid, set()).add(key)
                info["yes"] += 1
    return dict(sure), both, info, set(questions)


def usage(model: str, d: Path, stems=None):
    recs = []
    for f in sorted(d.glob("*.json")):
        if f.name.endswith(".failed.json") or (stems is not None and f.stem not in stems):
            continue
        recs.append(json.load(open(f, encoding="utf-8")))
    if not recs:
        return None
    pin, pout = PRICE[model]

    def price(r):
        i, o = r["input tokens"] or 0, r["output tokens"] or 0
        if model in LONG_PROMPT and i > LONG_PROMPT[model][0]:
            return (i * LONG_PROMPT[model][1] + o * LONG_PROMPT[model][2]) / 1e6
        return (i * pin + o * pout) / 1e6

    sec = np.array([r["seconds"] for r in recs])
    ins = np.array([r["input tokens"] or 0 for r in recs])
    outs = np.array([r["output tokens"] or 0 for r in recs])
    reas = np.array([r["reasoning tokens"] or 0 for r in recs])
    prices = np.array([price(r) for r in recs])
    return {"answered": len(recs), "models named": sorted({r["answered by"] for r in recs}),
            "input tokens": int(ins.sum()), "output tokens": int(outs.sum()), "reasoning tokens": int(reas.sum()),
            "requests with reasoning tokens": int((reas > 0).sum()),
            "median input tokens": float(np.median(ins)), "median output tokens": float(np.median(outs)),
            "median reasoning tokens": float(np.median(reas)),
            "seconds, median": float(np.median(sec)), "seconds, 90th": float(np.percentile(sec, 90)),
            "seconds, largest": float(sec.max()), "seconds, total": float(sec.sum()),
            "price, total": float(prices.sum()), "price, median a request": float(np.median(prices)),
            "attempts beyond the first": int(sum(r["attempts"] - 1 for r in recs)),
            "finish reasons": dict(sorted({str(r["finish"]): sum(1 for x in recs if str(x["finish"]) == str(r["finish"]))
                                           for r in recs}.items())),
            "in flight": sorted({r["in flight"] for r in recs})}


def main(out: Path, round_1p: Path, round_1q: Path) -> None:
    ext = out / "external"
    schemas = [json.loads(line) for line in open(ext / "hidden.jsonl", encoding="utf-8")]
    bench = [json.loads(line) for line in open(ext / "bench.jsonl", encoding="utf-8")]
    truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas if s["fk"]}
    source = {s["id"]: s["source"] for s in schemas}
    novel = {s["id"]: s["novel"] for s in schemas}
    ids = sorted(truth)
    refit = defaultdict(set)
    for line in open(ext / "open-predictions-both.jsonl", encoding="utf-8"):
        sid, t, c, r, _ = json.loads(line)
        refit[sid].add((t, c, r))
    rng = np.random.default_rng(EXTERNAL_SEED)
    subsets = {}
    for which in SOURCES + ("all",):
        for sub in ("all", "novel"):
            sel = [i for i in ids if which in ("all", source[i]) and (sub == "all" or novel[i])]
            subsets[f"{which}, {sub}"] = (sel, rng.integers(0, len(sel), (1000, len(sel))))

    def measure(preds, against=None):
        e = {}
        for k, (sel, idx) in subsets.items():
            per = counts(truth, preds, sel)
            p, r, f = prf(*per.sum(axis=0))
            boot = resampled(per, idx)
            e[k] = {"precision": p, "recall": r, "f1": f, "f1_95": interval(boot[:, 2]),
                    "macro f1 over databases": float(np.mean([prf(*c)[2] for c in per]))}
            if against is not None:
                pa = counts(truth, against, sel)
                da = resampled(pa, idx)
                point = np.array(prf(*pa.sum(axis=0))) - np.array([p, r, f])
                e[k]["refit minus this"] = {w: {"point": float(point[j]), "95": interval(da[:, j] - boot[:, j])}
                                            for j, w in enumerate(("precision", "recall", "f1"))}
        return e

    summary = {"refit both": measure(refit), "models": {}, "earlier runs": {}}
    for m in MODELS:
        base = out / "llm" / m
        if not base.exists():
            continue
        entry = {}
        preds, info = whole_schema(schemas, base / "db")
        entry["whole schema"] = {"replies": info, **measure(preds, refit)}
        entry["the paper's task"] = pair_task(bench, base / "pair")
        for cs in ("cascade-roles", "cascade-both"):
            if not (base / cs).exists():
                continue
            sure, both, cinfo, asked = cascade(out / "prompts" / cs, base / cs)
            alone = measure(sure)
            with_lm = measure(both)
            # The cascade minus its first pass, and whole-schema minus cascade
            diffs = {}
            for k, (sel, idx) in subsets.items():
                a, b, w = counts(truth, both, sel), counts(truth, sure, sel), counts(truth, preds, sel)
                ra, rb, rw = resampled(a, idx), resampled(b, idx), resampled(w, idx)
                diffs[k] = {"cascade minus its first pass": {"point": float(prf(*a.sum(0))[2] - prf(*b.sum(0))[2]),
                                                             "95": interval(ra[:, 2] - rb[:, 2])},
                            "whole schema minus cascade": {"point": float(prf(*w.sum(0))[2] - prf(*a.sum(0))[2]),
                                                           "95": interval(rw[:, 2] - ra[:, 2])}}
            stems = {gemini.db_stem(s) for s in asked}
            entry[cs] = {"replies": cinfo, "first pass alone": alone, "cascade": with_lm, "differences": diffs,
                         "usage": usage(m, base / cs),
                         "whole-schema usage on the same databases": usage(m, base / "db", stems)}
        entry["usage"] = {s: usage(m, base / s) for s in ("db", "pair") if (base / s).exists()}
        entry["usage, whole schema by source"] = {
            src: usage(m, base / "db", {gemini.db_stem(i) for i in ids if source[i] == src}) for src in SOURCES}
        summary["models"][m] = entry
    # The earlier rounds' replies to the same prompts, read the same way
    earlier = {"gemini-3.8-flash-high, round 1p": round_1p / "gemini",
               "gemini-3.5-flash-lite, round 1q": round_1q / "llm" / "gemini-3.5-flash-lite",
               "gemini-3-flash, round 1q": round_1q / "llm" / "gemini-3-flash",
               "Haiku as agents, round 1q": round_1q / "llm" / "haiku"}
    for name, d in earlier.items():
        if d.exists():
            preds, info = whole_schema(schemas, d)
            summary["earlier runs"][name] = {"whole schema": {"replies": info, **measure(preds, refit)},
                                             "the paper's task": pair_task(bench, d)}
    (out / "lm-summary.json").write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    for m, e in summary["models"].items():
        a = e["whole schema"]["all, all"]
        print(f"{m:24} P {a['precision']:.3f} R {a['recall']:.3f} F1 {a['f1']:.3f} {e['whole schema']['replies']}")
    for m, e in summary["earlier runs"].items():
        a = e["whole schema"]["all, all"]
        print(f"{m:40} F1 {a['f1']:.3f}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]))
