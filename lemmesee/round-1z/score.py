"""Round 1z: every model of the round read against lemmekey, a set at a time.

    python -I score.py <out.json>

Runs in round 1b's environment, through the earlier rounds' functions:
round 1w's `lms.a_set` gives a set's schemas and `eleven--wide`'s counts a
database (plain A and style 3 from round 1v's one reading, the other sets
from its kept scores), round 1s's `lm.whole_schema` reads a reply and
round 1v's `whole.measure` counts.

Beside lemmekey, a set at a time:

- the remote language models of round 1w, from their saved replies
  (corpus/round-1w/lm/llm/), counted again by the same code;
- the local language models of this round (corpus/round-1z/llm/<tag>/<set>/),
  each under its two settings of reasoning. A reply that did not end by
  itself, a prompt that did not fit its window and a reply that cannot be
  read count as no key predicted, and are counted by the number of tables.
  A model that answered only plain A's sample is measured on the sample,
  lemmekey beside it on the same schemas;
- the networks of this round and lemmekey's first stage alone, from the
  counts read.py kept (corpus/round-1z/read/<set>.json).
"""

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1w"))

import lms  # noqa: E402  (round 1w's; the earlier rounds come onto the path with it)

import numpy as np  # noqa: E402

import gemini  # noqa: E402
import kept  # noqa: E402
import lm  # noqa: E402
import whole  # noqa: E402

OUT = whole.ROOT / "round-1z"
LOCAL = OUT / "llm"
MODEL = kept.MODEL
SETS = {"plain-a-db": "plain-a", "style3-db": "style3", "db": "external", "heldout-db": "livesqlbench"}
READ = {"plain-a-db": ("plain-a",), "style3-db": ("style3",), "db": ("spider", "bird", "ctu"),
        "heldout-db": ("livesqlbench-development",)}


def against(per: np.ndarray, ours: np.ndarray, tables: np.ndarray) -> dict:
    """A model's measures and lemmekey minus it, both arms on the same resamples."""
    m = whole.measure(per, tables)
    d = whole.measure(per, tables, ours)
    m["minus lemmekey, f1"], m["minus lemmekey, f1_95"] = d["minus base, f1"], d["minus base, f1_95"]
    m["minus lemmekey, macro f1"], m["minus lemmekey, macro f1_95"] = d["minus base, macro f1"], d["minus base, macro f1_95"]
    idx = np.random.default_rng(whole.SEED).integers(0, len(per), (1000, len(per)))
    a, b = whole.resampled(per, idx), whole.resampled(ours, idx)
    for j, w in enumerate(("precision", "recall")):
        m[f"{w}_95"] = whole.interval(a[:, j])
        m[f"minus lemmekey, {w}"] = float(whole.prf(*per.sum(axis=0))[j] - whole.prf(*ours.sum(axis=0))[j])
        m[f"minus lemmekey, {w}_95"] = whole.interval(a[:, j] - b[:, j])
    big = tables >= 40
    if big.any():
        bm = whole.measure(per[big], tables[big], ours[big])
        m["40 tables or more"] = {k: v for k, v in bm.items() if k != "by tables" and not k.startswith("minus base, macro")}
    return m


STRICT = gemini.last_object


def mended(text: str, must: str):
    """round 1p's reading, and where it finds nothing, once more with the one closing brace a
    reply may lack: `{"keys": [...]` and no `}`. A diagnosis beside the protocol's reading."""
    obj = STRICT(text, must)
    if obj is None:
        body = text.strip()
        if body.endswith("```"):
            body = body[:-3].rstrip()
        if body.endswith("]"):
            obj = STRICT(body + "}", must)
    return obj


def lenient(schemas: list, d: Path) -> tuple:
    """lm.whole_schema with two faults of form forgiven: the closing brace (`mended`), and a
    target written with its column, `users(id)` or `users.id`, where what stands before the
    bracket or the last dot is a table of the schema. Everything else as the protocol reads it."""
    import re
    preds, info = {}, {"unanswered": 0, "unreadable": 0, "keys naming nothing": 0, "targets read without their column": 0,
                       "replies with a closing brace added": 0}
    for s in schemas:
        raw = d / f"{gemini.db_stem(s['id'])}.raw"
        if not raw.exists():
            info["unanswered"] += 1
            preds[s["id"]] = set()
            continue
        text = raw.read_text(encoding="utf-8", errors="replace")
        obj = mended(text, "keys")
        info["replies with a closing brace added"] += obj is not None and STRICT(text, "keys") is None
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
            ref = str(e.get("references", "")).lower()
            to = names.get(ref)
            if to is None:
                m = re.match(r"^(.+?)\s*\(.*\)$", ref)
                short = m.group(1) if m else (ref.rsplit(".", 1)[0] if "." in ref else None)
                if short in names:
                    to = names[short]
                    info["targets read without their column"] += 1
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


def local(d: Path, schemas: list, mend: bool = False) -> tuple:
    """A local model's keys a schema, and what became of each prompt."""
    gemini.last_object = mended if mend else STRICT
    recs = {p.stem: json.load(open(p, encoding="utf-8")) for p in d.glob("db-*.json")}
    asked = [s for s in schemas if s["fk"] and gemini.db_stem(s["id"]) in recs]
    preds, info = lenient(asked, d) if mend else lm.whole_schema(asked, d)
    fate = {}
    bins = {whole.bin_name(b): {"schemas": 0, "cut": 0, "does not fit": 0, "unreadable": 0} for b in whole.BINS}
    for s in asked:
        r = recs[gemini.db_stem(s["id"])]
        b = next(whole.bin_name(b) for b in whole.BINS if b[0] <= len(s["tables"]) < b[1])
        bins[b]["schemas"] += 1
        if r["finish"] == "does not fit":
            what = "does not fit"
        elif r["finish"] != "stop":
            what = "cut"
        elif (mended if mend else STRICT)((d / f"{gemini.db_stem(s['id'])}.raw").read_text(encoding="utf-8", errors="replace"),
                                "keys") is None:
            what = "unreadable"
        else:
            what = "read"
        fate[s["id"]] = what
        if what != "read":
            preds[s["id"]] = set()
            bins[b][what] += 1
    answered = [r for r in recs.values() if "answer" in r]
    cpu = set()
    if (OUT / "cpu-replies.json").exists() and "gemma4" in d.parent.name and d.name == "plain-a-db":
        cpu = {r["stem"] + ".txt" for r in json.load(open(OUT / "cpu-replies.json", encoding="utf-8"))["replies"]}
    by_run = {}
    for label, lo, hi in (("26 November, 250 W", "", "2026-11-26T19:00"), ("after the reboot, 187 W from 22:24 UTC", "2026-11-26T19:00", "9")):
        rs = [r for r in answered if lo <= r["asked at"] < hi and r["prompt"] not in cpu]
        if rs:
            sec = np.array([r["seconds"] - r.get("seconds, load", 0) for r in rs])
            rate = [r["output tokens"] / r["seconds, reply"] for r in rs if r.get("seconds, reply") and (r.get("output tokens") or 0) >= 50]
            by_run[label] = {"replies": len(rs), "seconds, a schema's median": float(np.median(sec)),
                             "seconds, a schema's mean": float(sec.mean()), "seconds, 90th percentile": float(np.percentile(sec, 90)),
                             "seconds, the longest": float(sec.max()), "seconds, total": float(sec.sum()),
                             "output tokens a second, a reply's median": float(np.median(rate)) if rate else None}
    secs = np.array([r["seconds"] for r in answered]) if answered else np.zeros(1)
    runs = [json.load(open(p, encoding="utf-8")) for p in sorted(d.glob("run-*.json"))]
    use = {"prompts": len(recs), "answered by the runtime": len(answered),
           "cut": sum(v == "cut" for v in fate.values()), "does not fit": sum(v == "does not fit" for v in fate.values()),
           "unreadable": sum(v == "unreadable" for v in fate.values()),
           "cut at 32 768 output tokens": sum(1 for r in answered if r["finish"] != "stop" and (r.get("output tokens") or 0) >= 32768),
           "keys naming nothing": info["keys naming nothing"],
           "targets read without their column": info.get("targets read without their column"),
           "by tables": {k: v for k, v in bins.items() if v["schemas"]},
           "input tokens": int(sum(r.get("input tokens") or 0 for r in answered)),
           "output tokens": int(sum(r.get("output tokens") or 0 for r in answered)),
           "input tokens, a schema's median": float(np.median([r.get("input tokens") or 0 for r in answered])) if answered else 0,
           "output tokens, a schema's median": float(np.median([r.get("output tokens") or 0 for r in answered])) if answered else 0,
           "by run": by_run, "replies on the processor, their timings left out": len(cpu),
           "seconds, total": float(secs.sum()), "seconds, a schema's median": float(np.median(secs)),
           "seconds, a schema's mean": float(secs.mean()), "seconds, 90th percentile": float(np.percentile(secs, 90)),
           "seconds, the longest": float(secs.max()),
           "windows": {str(w): sum(1 for r in recs.values() if r.get("window") == w) for w in (16384, 32768, 65536)},
           "runs": [{k: r.get(k) for k in ("started", "ended", "asked", "card MiB before", "card MiB at most",
                                           "runtime resident bytes at most", "loaded, by window")} for r in runs],
           "card MiB at most": max((r["card MiB at most"] for r in runs), default=None),
           "card MiB before": min((r["card MiB before"] for r in runs), default=None),
           "runtime resident bytes at most": max((r["runtime resident bytes at most"] for r in runs), default=None),
           "loaded, by window": {k: v for r in runs for k, v in r["loaded, by window"].items()},
           "asked from": min((r["started"] for r in runs), default=None), "asked until": max((r["ended"] for r in runs), default=None),
           "tag": runs[0]["tag"] if runs else None, "digest": runs[0]["digest"] if runs else None,
           "bytes": runs[0]["bytes"] if runs else None, "reasoning": runs[0]["reasoning"] if runs else None}
    return [s["id"] for s in asked], preds, use


def run(out: Path) -> None:
    frozen = kept.check_frozen()
    result = {"lemmekey": {"name": MODEL, "frozen": frozen}}
    for folder, label in SETS.items():
        schemas, pers = lms.a_set(folder)
        ids = [s["id"] for s in schemas]
        at = {i: k for k, i in enumerate(ids)}
        truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas}
        tables = np.array([len(s["tables"]) for s in schemas])
        ours = pers[MODEL]
        entry = {"databases": len(ids), "keys": int(sum(len(t) for t in truth.values())), "models": {}}
        entry["models"]["lemmekey"] = against(ours, ours, tables)
        print(f"== {label}: {len(ids)} databases, {entry['keys']} keys")
        print(whole.line("lemmekey", entry["models"]["lemmekey"]))
        # the networks and the first stage alone, as read.py kept them
        pers_read = []
        for which in READ[folder]:
            p = OUT / "read" / f"{which}.json"
            if p.exists():
                pers_read.append(json.load(open(p, encoding="utf-8")))
        if len(pers_read) == len(READ[folder]):
            got = [i for r in pers_read for i in r["databases"]]
            assert got == ids, "read.py's databases are not in this order"
            for n in pers_read[0]["models"]:
                per = np.vstack([np.array(r["models"][n]["per database"]) for r in pers_read])
                name = "lemmekey, scored again" if n == MODEL else n
                entry["models"][name] = against(per, ours, tables)
                entry["models"][name]["kind"] = "tree" if n.startswith("eleven") else "network"
                if n == MODEL:
                    entry["models"][name]["the same counts as the kept reading"] = bool((per == ours).all())
                print(whole.line(name, entry["models"][name]))
        # the remote language models' saved replies, as round 1w read them
        for d in sorted(lms.LLM.glob(f"*/{folder}")):
            if not any(d.glob("*.raw")):
                continue
            preds, info = lm.whole_schema(schemas, d)
            if info["unanswered"]:
                continue
            m = against(lm.counts(truth, preds, ids), ours, tables)
            m["kind"], m["replies"] = "remote", info
            preds_m, info_m = lenient([x for x in schemas if x["fk"]], d)
            mm = against(lm.counts(truth, preds_m, ids), ours, tables)
            m["form mended"] = {**{k: mm[k] for k in ("precision", "recall", "f1", "f1_95", "minus lemmekey, f1",
                                                      "minus lemmekey, f1_95")}, **info_m}
            entry["models"][d.parent.name] = m
            print(whole.line(d.parent.name, m))
        # the local language models
        for d in sorted(LOCAL.glob(f"*/{folder}")):
            asked, preds, use = local(d, schemas)
            if not asked:
                continue
            every = [i for i in ids if truth[i]]
            rows = np.array([at[i] for i in asked])
            per = lm.counts(truth, preds, asked)
            m = against(per, ours[rows], tables[rows])
            m["kind"], m["usage"] = "local", use
            m["schemas asked"], m["of"] = len(asked), len(every)
            if len(asked) < len(every):
                m["lemmekey on the same schemas"] = against(ours[rows], ours[rows], tables[rows])
            # the same replies with two faults of form forgiven: beside the protocol's reading, never in its place
            _, preds_m, use_m = local(d, schemas, mend=True)
            gemini.last_object = STRICT
            mm = against(lm.counts(truth, preds_m, asked), ours[rows], tables[rows])
            m["form mended"] = {k: mm[k] for k in ("precision", "recall", "f1", "f1_95", "macro f1", "by tables",
                                                             "minus lemmekey, f1", "minus lemmekey, f1_95", "40 tables or more")
                                         if k in mm}
            m["form mended"].update({k: use_m[k] for k in ("unreadable", "keys naming nothing",
                                                           "targets read without their column")})
            entry["models"][d.parent.name] = m
            print(whole.line(f"{d.parent.name} ({len(asked)}/{len(every)})", m))
            print(f"    form mended: P {mm['precision']:.3f} R {mm['recall']:.3f} F1 {mm['f1']:.3f} "
                  f"minus lemmekey {mm['minus lemmekey, f1']:+.3f} {[round(v, 3) for v in mm['minus lemmekey, f1_95']]} "
                  f"unreadable {use_m['unreadable']} targets without their column {use_m['targets read without their column']}")
            median = use["seconds, a schema's median"]
            print(f"    minus lemmekey {m['minus lemmekey, f1']:+.3f} {[round(v, 3) for v in m['minus lemmekey, f1_95']]}  "
                  f"cut {use['cut']} does not fit {use['does not fit']} unreadable {use['unreadable']}  "
                  f"median {median:.1f} s")
        result[label] = entry
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    run(Path(sys.argv[1]))
