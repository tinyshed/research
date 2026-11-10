"""Round 1v's first question: F1 over every declared key by the number of
tables of a schema, for the frozen models on every set this work has read
and for the language models where their replies are kept.

    python -I sizes.py <out.json>

The frozen models' keys are dump.py's (their saved scores at their own
thresholds, one target a column), and for each what becomes of a declared
key by size: found, its table scored first but under the threshold, another
table first, no candidate, and the best F1 one threshold for that size
could reach. The language models' are their replies to
round 1p's whole-schema prompt under round 1s's protocol, read as round 1s
reads them: Spider, BIRD and CTU (round 1s), LiveSQLBench (round 1s) and the
fresh GitHub set (round 1t, 9 November). Nothing is asked here.
"""

import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "round-1p"))

import numpy as np  # noqa: E402

import gemini  # noqa: E402  (round 1p's reading of a reply)
import whole  # noqa: E402

FROZEN = ("1s", "seven-bounded", "nine-mended")
LMS = ("gemini-3-flash", "gemini-3.5-flash-lite", "claude-haiku-5-5", "gemini-3.8-flash-high")
REPLIES = {"spider": ("round-1s", "db"), "bird": ("round-1s", "db"), "ctu": ("round-1s", "db"),
           "livesqlbench (development)": ("round-1s", "heldout-db"), "github fresh": ("round-1t/lm", "fresh-db")}


def lm_counts(schemas: list, raws: Path) -> np.ndarray:
    per = np.zeros((len(schemas), 3), dtype=np.int64)
    for i, s in enumerate(schemas):
        truth = {tuple(k) for k in s["fk"]}
        raw = raws / f"{gemini.db_stem(s['id'])}.raw"
        drawn = set()
        if raw.exists():
            obj = gemini.last_object(raw.read_text(encoding="utf-8", errors="replace"), "keys") or {}
            names = {t["name"].lower(): t for t in s["tables"]}
            seen = set()
            for e in obj.get("keys") or []:
                if not isinstance(e, dict):
                    continue
                t = names.get(str(e.get("table", "")).lower())
                to = names.get(str(e.get("references", "")).lower())
                col = next((c["name"] for c in t["columns"] if c["name"].lower() == str(e.get("column", "")).lower()),
                           None) if t else None
                if not (t and to and col):
                    drawn.add(("?", json.dumps(e, sort_keys=True), ""))
                    continue
                if (t["name"], col) in seen:
                    continue
                seen.add((t["name"], col))
                drawn.add((t["name"], col, to["name"]))
        per[i] = [len(truth & drawn), len(drawn - truth), len(truth - drawn)]
    return per


def fates(sc) -> dict:
    """What becomes of a schema's declared keys under a frozen model, by the
    number of its tables: found; its table scored first of the column's
    targets but under the threshold; another table scored first; no
    candidate pair held it."""
    ck = (sc.schema.astype(np.int64) << 32) | (sc.table.astype(np.int64) << 16) | sc.column.astype(np.int64)
    order = np.lexsort((-sc.second, ck))
    first = np.ones(len(order), dtype=bool)
    first[1:] = ck[order][1:] != ck[order][:-1]
    top = np.zeros(len(ck), dtype=bool)
    top[order[first]] = True
    out = {}
    for b in whole.BINS:
        ids = np.flatnonzero((sc.tables >= b[0]) & (sc.tables < b[1]))
        if not len(ids):
            continue
        rows = np.isin(sc.schema, ids) & sc.label
        keys = sum(len(sc.truth[i]) for i in ids)
        if not keys:
            continue
        found = int((rows & top & (sc.second >= sc.threshold)).sum())
        below = int((rows & top & (sc.second < sc.threshold)).sum())
        other = int((rows & ~top).sum())
        pairs = int(np.isin(sc.schema, ids).sum())
        columns = len(np.unique(ck[np.isin(sc.schema, ids)]))
        # one threshold for the schemas of this size: the best F1 over every declared key it can reach
        mine = np.isin(sc.schema, ids) & top
        scores, labels = sc.second[mine], sc.label[mine]
        by = np.argsort(-scores)
        tp = np.cumsum(labels[by])
        drawn = np.arange(1, len(by) + 1)
        f1 = 2 * tp / (drawn + keys)
        j = int(np.argmax(f1))
        at = lambda t: 2 * int((labels & (scores >= t)).sum()) / (int((scores >= t).sum()) + keys)  # noqa: E731
        out[whole.bin_name(b)] = {"keys": int(keys), "found": found / keys, "first, under the threshold": below / keys,
                                  "another table first": other / keys,
                                  "no candidate": (keys - found - below - other) / keys,
                                  "its table scored first": (found + below) / keys,
                                  "targets a column, mean": pairs / max(columns, 1),
                                  "f1 at the model's threshold": at(sc.threshold),
                                  "f1 at the best threshold for this size": float(f1[j]),
                                  "the best threshold for this size": float(scores[by][j]),
                                  "precision there": float(tp[j] / drawn[j]),
                                  "precision at 0.5": int((labels & (scores >= 0.5)).sum()) / max(int((scores >= 0.5).sum()), 1)}
    return out


def main(out: Path) -> None:
    result = {}
    pooled = {}
    for name in whole.SETS:
        entry = {}
        tables = None
        for fm in FROZEN:
            if not (whole.OUT / "scores" / fm / f"{whole.slug(name)}.pkl").exists():
                continue
            sc = whole.Scored(fm, name)
            tables = sc.tables
            per = sc.counts(sc.baseline())
            entry[fm] = whole.measure(per, tables)
            entry[fm]["what becomes of a declared key"] = fates(sc)
            pooled.setdefault(fm, []).append((name, per, tables))
            schemas = sc.schemas
        if name in REPLIES and tables is not None:
            root, folder = REPLIES[name]
            for lm in LMS:
                raws = whole.ROOT / root / "llm" / lm / folder
                if raws.exists():
                    per = lm_counts(schemas, raws)
                    entry[lm] = whole.measure(per, tables)
                    pooled.setdefault(lm, []).append((name, per, tables))
        result[name] = entry
        print(f"== {name}")
        for n, m in entry.items():
            print(whole.line(n, m))
    # the sets the language models answered, pooled: every model on the same databases
    names = [n for n in REPLIES if n in result and all(lm in result[n] for lm in LMS)]
    print("== " + ", ".join(names) + ", pooled")
    result["pooled where the language models answered"] = {"sets": names}
    for n, parts in pooled.items():
        chosen = [(per, tables) for name, per, tables in parts if name in names]
        if len(chosen) != len(names):
            continue
        per = np.vstack([p for p, _ in chosen])
        tables = np.concatenate([t for _, t in chosen])
        m = whole.measure(per, tables)
        result["pooled where the language models answered"][n] = m
        print(whole.line(n, m))
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    main(Path(sys.argv[1]))
