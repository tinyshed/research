"""Round 1q: every arm on the same databases — the model alone, with value
keys, with the catalog's drop, with both, the cascade with Haiku and the
cascade with both, beside the language models given the whole schema —
over every declared one-column key, per source, on the databases whose
statistics exist.

    .venv/Scripts/python -I arms.py <round 1p out> <cascade out> <stats> <llm replies root> <out> <label>

<llm replies root> holds one folder of round 1p's whole-schema replies a
model (gemini-3.8-flash-high's are round 1p's own gemini/ folder, named so
by a link or copy). Writes <out>/arms-<label>.json.
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

ROUND_1P = Path(__file__).resolve().parent.parent / "round-1p"
sys.path.insert(0, str(ROUND_1P))
sys.path.insert(0, str(Path(__file__).resolve().parent))

import gemini  # noqa: E402
import widen  # noqa: E402

from values import value_keys  # noqa: E402
from whose import evidence  # noqa: E402

Q, H, DROP = 0.5, 0.9, 0.5


def llm_keys(schema: dict, raw: Path) -> set | None:
    """A whole-schema reply's keys, as round 1p's gemini.py reads them."""
    if not raw.exists():
        return None
    obj = gemini.last_object(raw.read_text(encoding="utf-8", errors="replace"), "keys")
    if obj is None:
        return set()
    names = {t["name"].lower(): t for t in schema["tables"]}
    out, seen = set(), set()
    for e in obj.get("keys") or []:
        t = names.get(str(e.get("table", "")).lower())
        to = names.get(str(e.get("references", "")).lower())
        col = next((c["name"] for c in t["columns"] if c["name"].lower() == str(e.get("column", "")).lower()),
                   None) if t else None
        if not (t and to and col):
            out.add(("?", str(e), ""))
            continue
        if (t["name"], col) not in seen:
            seen.add((t["name"], col))
            out.add((t["name"], col, to["name"]))
    return out


def main(data: Path, cascade: Path, stats_dir: Path, llm_root: Path, out: Path, label: str) -> None:
    schemas = {s["id"]: s for s in map(json.loads, open(data / "hidden.jsonl", encoding="utf-8"))}
    stats = {}
    for f in stats_dir.glob("*/*.json"):
        sid = f"{f.parent.name}/{f.stem}"
        if sid in schemas and schemas[sid]["fk"]:
            stats[sid] = json.load(open(f, encoding="utf-8"))
    tables = {i: {t["name"]: t for t in schemas[i]["tables"]} for i in stats}
    sure, band = defaultdict(set), defaultdict(list)
    for line in open(cascade / "sure.jsonl", encoding="utf-8"):
        sid, t, c, r, _ = json.loads(line)
        if sid in stats:
            sure[sid].add((t, c, r))
    for line in open(cascade / "questions.jsonl", encoding="utf-8"):
        n, sid, t, c, r, _ = json.loads(line)
        if sid in stats:
            band[sid].append((n, (t, c, r)))
    haiku = defaultdict(set)
    for sid, qs in band.items():
        raw = cascade / "replies" / "haiku" / f"{gemini.db_stem(sid)}.raw"
        if raw.exists():
            answers = json.loads(raw.read_text(encoding="utf-8")).get("answers", {})
            haiku[sid] = {k for n, k in qs if answers.get(str(n)) is True}

    def drop(sid, keys):
        kept = set()
        for t, c, r in keys:
            k, _ = widen.key_of(tables[sid][r], True)
            e = evidence(stats[sid], t, c, r, k["name"]) if k else None
            if e is None or e["inside"] >= DROP:
                kept.add((t, c, r))
        return kept

    def drop_any(sid, keys):
        """The drop read again after BIRD: a key is kept when its column's
        sample lies within any unique column of its target — the declared
        key, a declared unique column, or one every sampled value of which
        is distinct."""
        kept = set()
        for t, c, r in keys:
            tab = tables[sid][r]
            unique = set(tab["pk"][:1] if len(tab["pk"]) == 1 else []) | {x["name"] for x in tab["columns"] if x["unique"]}
            unique |= {n for n, v in stats[sid].get(r, {}).get("columns", {}).items() if v["n_distinct"] == -1}
            seen = [e for u in unique if (e := evidence(stats[sid], t, c, r, u)) is not None]
            if not seen or max(e["inside"] for e in seen) >= DROP:
                kept.add((t, c, r))
        return kept

    def with_values(sid, keys):
        taken = {(t, c) for t, c, _ in keys}
        return keys | {(t, c, r) for t, c, r in value_keys(stats[sid], schemas[sid], Q, H) if (t, c) not in taken}

    arms = defaultdict(dict)
    for sid in stats:
        s = sure[sid]
        cas = s | haiku[sid]
        arms["model alone"][sid] = s
        arms["model, value keys"][sid] = with_values(sid, s)
        arms["model, catalog drop"][sid] = drop(sid, s)
        arms["model, both"][sid] = with_values(sid, drop(sid, s))
        arms["cascade with Haiku"][sid] = cas
        arms["cascade with Haiku, both"][sid] = with_values(sid, drop(sid, cas))
        arms["model, drop against any unique key"][sid] = drop_any(sid, s)
    llms = {}
    for d in sorted(p for p in llm_root.iterdir() if p.is_dir()):
        got = {}
        for sid in stats:
            k = llm_keys(schemas[sid], d / f"{gemini.db_stem(sid)}.raw")
            if k is not None:
                got[sid] = k
        llms[d.name] = got
        if len(got) == len(stats):
            # the same catalog checks on the language model's own keys
            llms[f"{d.name}, catalog drop"] = {i: drop(i, {k for k in got[i] if k[0] != "?"}) |
                                               {k for k in got[i] if k[0] == "?"} for i in got}
            llms[f"{d.name}, both"] = {i: with_values(i, llms[f"{d.name}, catalog drop"][i]) for i in got}
            llms[f"{d.name}, drop against any unique key"] = {i: drop_any(i, {k for k in got[i] if k[0] != "?"}) |
                                                               {k for k in got[i] if k[0] == "?"} for i in got}
    truth = {i: {tuple(k) for k in schemas[i]["fk"]} for i in stats}
    result = {"databases with statistics": {src: sum(1 for i in stats if schemas[i]["source"] == src)
                                           for src in ("spider", "bird", "ctu")}}
    for src in ("spider", "bird", "ctu"):
        ids = [i for i in stats if schemas[i]["source"] == src]
        if not ids:
            continue
        entry = {"databases": len(ids), "keys declared": sum(len(truth[i]) for i in ids)}
        for name, preds in list(arms.items()) + [(f"{m} (whole schema)", p) for m, p in llms.items()]:
            have = [i for i in ids if i in preds]
            if len(have) < len(ids):
                entry[name] = {"databases answered": len(have)}
                continue
            entry[name] = dict(zip(("precision", "recall", "f1"), gemini.prf(*gemini.open_counts(truth, preds, ids))))
        result[src] = entry
    # 95 % intervals of F1 differences, 1 000 resamples of databases
    import random
    pairs = [("model, value keys", "model alone"), ("model, catalog drop", "model alone"),
             ("model, drop against any unique key", "model alone"), ("model, both", "model alone"),
             ("cascade with Haiku", "model alone"), ("haiku (whole schema)", "cascade with Haiku")]
    for m in llms:
        if not m.endswith(("catalog drop", ", both", "any unique key")):
            for v in ("catalog drop", "drop against any unique key"):
                if f"{m}, {v}" in llms:
                    pairs.append((f"{m}, {v} (whole schema)", f"{m} (whole schema)"))
    every = {**arms, **{f"{m} (whole schema)": p for m, p in llms.items()}}
    every.update({f"{m.rsplit(', ', 1)[0]}, {m.rsplit(', ', 1)[1]} (whole schema)": p for m, p in llms.items() if ", " in m})
    rng = random.Random(20261101)
    for src in ("spider", "bird", "ctu"):
        ids = [i for i in stats if schemas[i]["source"] == src]
        diffs = {}
        for a_, b_ in pairs:
            if a_ not in every or b_ not in every or any(i not in every[a_] or i not in every[b_] for i in ids):
                continue
            f = lambda arm, sel: gemini.prf(*gemini.open_counts(truth, every[arm], sel))[2]  # noqa: E731
            boot = sorted(f(a_, pick) - f(b_, pick) for pick in ([rng.choice(ids) for _ in ids] for _ in range(1000)))
            diffs[f"{a_} minus {b_}"] = {"difference": f(a_, ids) - f(b_, ids), "95": [boot[24], boot[974]]}
        if src in result:
            result[src]["differences"] = diffs
    out.mkdir(parents=True, exist_ok=True)
    (out / f"arms-{label}.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    for src in ("spider", "bird", "ctu"):
        if src not in result:
            continue
        print(f"== {src}: {result[src]['databases']} databases, {result[src]['keys declared']} keys")
        for k, v in result[src].items():
            if isinstance(v, dict) and "f1" in v:
                print(f"  {k:42} P {v['precision']:.3f} R {v['recall']:.3f} F1 {v['f1']:.3f}")
            elif isinstance(v, dict):
                print(f"  {k:42} {v}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    a = sys.argv
    main(Path(a[1]), Path(a[2]), Path(a[3]), Path(a[4]), Path(a[5]), a[6])
