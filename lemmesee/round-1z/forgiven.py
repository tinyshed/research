"""Round 1z: every remote language model's saved replies under the forgiving reading.

    python -I forgiven.py <out.json>

The forgiving reading is score.py's `lenient`: round 1p's reader, and where
it fails, two faults of form forgiven. A reply that lacks its one closing
brace has it added; a target written with its column, `users(id)` or
`users.id`, is read as its table when what stands before the bracket or the
last dot is a table of the schema. It was written on 27 November 2026,
after the replies had been read under the protocol's strict reading, from
two local models' replies on plain A.

For lemmekey and for every model of the first group (7 to 9 November,
round 1s's and round 1t's replies) and of the second (14 November, round
1w's), a set at a time, both readings: P, R and F1 with 95 % intervals,
macro F1, lemmekey minus the model with its interval on the same resamples,
by the number of tables and at 40 tables or more; the external databases by
source as well; and how many targets were read without their column and how
many replies had a brace added. lemmekey's keys are not replies and are the
same under either reading.

The pair task (the paper's Table 12) is read too: a reply there names
`referencedTable` and `referencedColumn` apart, and it is counted how many
replies write a table with a column in it and what recall at one becomes
when such a table is read without it.
"""

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import score  # noqa: E402  (round 1w's lms and the earlier rounds come onto the path with it)

import numpy as np  # noqa: E402

import gemini  # noqa: E402
import kept  # noqa: E402
import lm  # noqa: E402
import lms  # noqa: E402
import whole  # noqa: E402

ROOT = whole.ROOT
SETS = {"plain-a-db": "plain A", "style3-db": "style 3", "db": "398 external databases",
        "heldout-db": "LiveSQLBench", "fresh-db": "fresh GitHub set"}
FIRST = {"db": ROOT / "round-1s" / "llm", "heldout-db": ROOT / "round-1s" / "llm", "fresh-db": ROOT / "round-1t" / "lm" / "llm"}
SECOND = ROOT / "round-1w" / "lm" / "llm"


def measures(per: np.ndarray, ours: np.ndarray, tables: np.ndarray) -> dict:
    """A model's measures, and lemmekey minus it on the same resamples."""
    m = whole.measure(per, tables)
    idx = np.random.default_rng(whole.SEED).integers(0, len(per), (1000, len(per)))
    a, b = whole.resampled(ours, idx), whole.resampled(per, idx)
    out = {"precision": m["precision"], "recall": m["recall"], "f1": m["f1"], "f1_95": m["f1_95"],
           "precision_95": whole.interval(b[:, 0]), "recall_95": whole.interval(b[:, 1]), "macro f1": m["macro f1"]}
    po, pm = whole.prf(*ours.sum(axis=0)), whole.prf(*per.sum(axis=0))
    for j, w in enumerate(("precision", "recall", "f1")):
        out[f"lemmekey minus it, {w}"] = float(po[j] - pm[j])
        out[f"lemmekey minus it, {w}_95"] = whole.interval(a[:, j] - b[:, j])
    macro_o = np.array([whole.prf(*c)[2] for c in ours])
    macro_m = np.array([whole.prf(*c)[2] for c in per])
    out["lemmekey minus it, macro f1"] = float(macro_o.mean() - macro_m.mean())
    out["lemmekey minus it, macro f1_95"] = whole.interval(macro_o[idx].mean(axis=1) - macro_m[idx].mean(axis=1))
    out["by tables"] = {k: {"schemas": v["databases"], "keys": v["keys"], "precision": v["precision"], "recall": v["recall"],
                            "f1": v["f1"]} for k, v in m["by tables"].items()}
    return out


def both(per: np.ndarray, ours: np.ndarray, tables: np.ndarray, sources) -> dict:
    out = measures(per, ours, tables)
    big = tables >= 40
    if big.sum() > 1:
        b = measures(per[big], ours[big], tables[big])
        out["40 tables or more"] = {k: v for k, v in b.items() if k != "by tables"}
        out["40 tables or more"]["schemas"] = int(big.sum())
    if sources is not None:
        out["by source"] = {}
        for src in ("spider", "bird", "ctu"):
            sel = sources == src
            b = measures(per[sel], ours[sel], tables[sel])
            out["by source"][src] = {k: v for k, v in b.items() if k != "by tables"}
    return out


def pair_task() -> dict:
    """Round 1s's pair replies: how many write a table with a column in it."""
    out = {}
    for which, folder in (("external", "pair"), ("heldout", "heldout-pair")):
        path = ROOT / "round-1s" / which / "bench.jsonl"
        if not path.exists():
            continue
        bench = [json.loads(line) for line in open(path, encoding="utf-8")]
        for model in lm.MODELS:
            raws = ROOT / "round-1s" / "llm" / model / folder
            n = hit = hit_f = with_column = 0
            for p in bench:
                if p.get("occurs_in_perm") or p.get("too_large"):
                    continue
                raw = raws / f"{gemini.pair_stem(p)}.raw"
                if not raw.exists():
                    continue
                obj = gemini.last_object(raw.read_text(encoding="utf-8", errors="replace"), "column") or {}
                got = [str(obj.get(k, "")).lower() for k in ("table", "column", "referencedTable", "referencedColumn")]
                want = [p["table"].lower(), p["column"].lower(), p["foreign_table"].lower(), p["referred_column"].lower()]
                n += 1
                hit += got == want
                soft = list(got)
                for j in (0, 2):
                    m = re.match(r"^(.+?)\s*\(.*\)$", got[j])
                    short = m.group(1) if m else (got[j].rsplit(".", 1)[0] if "." in got[j] else None)
                    if short is not None and short == want[j]:
                        soft[j] = short
                        with_column += 1
                hit_f += soft == want
            if n:
                out[f"{model}, {folder}"] = {"pairs": n, "recall at one, strict": hit / n, "recall at one, forgiving": hit_f / n,
                                             "tables written with a column": with_column}
    return out


def run(out: Path) -> None:
    result = {"what": "every remote language model's saved replies read twice: `strict` is the protocol's reading "
                      "(round 1p's reader), `forgiving` adds a missing closing brace and reads a target written "
                      "table(column) or table.column as its table when that table is in the schema. The forgiving "
                      "reading was introduced on 27 November 2026, after the replies had been read. Differences are "
                      "lemmekey minus the model, both arms on the same 1 000 resamples of schemas, seed 20261101.",
              "sets": {}}
    for folder, label in SETS.items():
        if folder == "fresh-db":
            sc = kept.scored(kept.MODEL, "github fresh")
            schemas, ours = sc.schemas, sc.counts(sc.baseline())
        else:
            schemas, pers = lms.a_set(folder)
            ours = pers[score.MODEL]
        ids = [s["id"] for s in schemas]
        truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas}
        tables = np.array([len(s["tables"]) for s in schemas])
        sources = np.array([s.get("source", "") for s in schemas]) if folder == "db" else None
        asked = [s for s in schemas if s["fk"]]
        entry = {"schemas": len(ids), "keys": int(sum(len(t) for t in truth.values())),
                 "development data for lemmekey": folder not in ("plain-a-db", "style3-db"),
                 "lemmekey": both(ours, ours, tables, sources), "models": {}}
        print(f"== {label}: {len(ids)} schemas, {entry['keys']} keys; lemmekey F1 {entry['lemmekey']['f1']:.3f}")
        groups = [("second group, 14 November", d) for d in sorted(SECOND.glob(f"*/{folder}"))]
        if folder in FIRST:
            groups = [("first group, 7 to 9 November", d) for d in sorted(FIRST[folder].glob(f"*/{folder}"))] + groups
        for group, d in groups:
            if not any(d.glob("*.raw")):
                continue
            strict_preds, info = lm.whole_schema(schemas, d)
            if info["unanswered"]:
                continue
            soft_preds, soft = score.lenient(asked, d)
            m = {"group": group,
                 "strict": {**both(lm.counts(truth, strict_preds, ids), ours, tables, sources), **info},
                 "forgiving": {**both(lm.counts(truth, soft_preds, ids), ours, tables, sources), **soft}}
            name = d.parent.name + (" (first group)" if group.startswith("first") else "")
            entry["models"][name] = m
            s, f = m["strict"], m["forgiving"]
            print(f"{name:40} strict {s['f1']:.3f}  forgiving P {f['precision']:.3f} R {f['recall']:.3f} F1 {f['f1']:.3f} "
                  f"[{f['f1_95'][0]:.3f}, {f['f1_95'][1]:.3f}] macro {f['macro f1']:.3f}  lemmekey minus it "
                  f"{f['lemmekey minus it, f1']:+.3f} [{f['lemmekey minus it, f1_95'][0]:+.3f}, "
                  f"{f['lemmekey minus it, f1_95'][1]:+.3f}]  targets without their column "
                  f"{f['targets read without their column']}, braces added {f['replies with a closing brace added']}, "
                  f"unreadable {f['unreadable']}, naming nothing {f['keys naming nothing']}")
        result["sets"][label] = entry
    result["pair task"] = pair_task()
    for k, v in result["pair task"].items():
        print("pair task", k, v)
    out.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    run(Path(sys.argv[1]))
