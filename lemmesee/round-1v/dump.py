"""Round 1v: every candidate pair of a set as a frozen model scores it, saved
once, so that what is read over a whole schema can be tried without building
the pairs again.

    python -I dump.py sets                      the sets, as pickles of their schemas
    python -I dump.py score <model> [<set> ...]  a model's two stages on each set

Paths are round 1t's, under the repository's ignored corpus; the output goes
to corpus/round-1v/sets/<set>.pkl and corpus/round-1v/scores/<model>/<set>.pkl.

A set's pickle is the list of its schemas as the models read them. A score's
pickle holds, a row a pair in the order `cues.build_all` wrote them (a
schema's pairs together): the schema, table, column and target by their
index in the set's list, the label, the first stage's probability and the
second stage's raw score, and the name of the target's key column the pair
was read through.

The sets: SchemaPile's validation split and its renamed copies as
`nine-mended` was fitted beside them (restyle.py with the dictionary's
shortenings and a key word first, round 1t's seed), its test split and the
renamed test as round 1t scored them, Spider, BIRD, CTU and LiveSQLBench as
round 1s read them, and the four GitHub sets as the fixed collector writes
them. All of them were read before this round; none is clean.
"""

import json
import pickle
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1t"))

import fit  # noqa: E402  (round 1t's: the earlier rounds on the path, the trigrams sorted)

import numpy as np  # noqa: E402

import cues  # noqa: E402
import graph  # noqa: E402
import keys  # noqa: E402
import letters  # noqa: E402
import restyle  # noqa: E402
import stack  # noqa: E402
import widen  # noqa: E402

OUT = fit.ROOT / "round-1v"
GITHUB = {"github 2025": "heldout-github-typed", "github 2025, rest": "heldout-github-rest-typed",
          "github style": "heldout-github-style-typed", "github fresh": "heldout-github-style-fresh"}


def slug(name: str) -> str:
    return name.replace(",", "").replace("'", "").replace("(", "").replace(")", "").replace(" ", "-")


def all_sets() -> dict:
    """name: (schemas, split, guesses)."""
    sp, sp_guesses = keys.load(fit.ROOT / "round-1b")
    val = [s for s in sp if s["split"] == "validation" and s["fk"]]
    restyle.DICTIONARY, restyle.KEY_FIRST = True, True
    renamed = [r for r in (fit.restyled(s, fit.RESTYLE_SEED, "~restyled") for s in val) if r]
    restyle.DICTIONARY, restyle.KEY_FIRST = False, False
    out = {"schemapile validation": (val, "validation", sp_guesses),
           "schemapile validation, renamed": (renamed, "validation", {})}
    del sp
    out.update(fit.sets())
    out.pop("teachers' test domains", None)
    for name, d in GITHUB.items():
        schemas = [json.loads(line) for line in open(fit.OUT / d / "hidden.jsonl", encoding="utf-8")]
        out[name] = (schemas, "external", {})
    return out


def write_sets() -> None:
    (OUT / "sets").mkdir(parents=True, exist_ok=True)
    for name, (schemas, split, guesses) in all_sets().items():
        chosen = [s for s in schemas if s["split"] == split and s["fk"]]
        by = {(g[0], g[1], g[2]): v.get("fk") for g, v in guesses.items()} if guesses else {}
        ids = {s["id"] for s in chosen}
        rules = {k: v for k, v in by.items() if k[0] in ids and v}
        pickle.dump({"name": name, "split": split, "schemas": chosen, "rules": rules},
                    open(OUT / "sets" / f"{slug(name)}.pkl", "wb"), protocol=4)
        print(f"{name:34} {len(chosen):5d} schemas {sum(len(s['fk']) for s in chosen):6d} keys", flush=True)


def score(model: str, names: list[str]) -> None:
    assert letters.is_sorted()
    d, variant = fit.MODELS[model]
    first, second = widen.load_models(d)
    final = json.load(open(d / "graph-final.json", encoding="utf-8"))
    gf = stack.of(final)
    (OUT / "scores" / model).mkdir(parents=True, exist_ok=True)
    names = names or [p.stem for p in sorted((OUT / "sets").glob("*.pkl"))]
    with letters.pool(graph.WORKERS) as pool:
        for name in names:
            t0 = time.time()
            data = pickle.load(open(OUT / "sets" / f"{slug(name)}.pkl", "rb"))
            schemas, split = data["schemas"], data["split"]
            guesses = {k: {"fk": v} for k, v in data["rules"].items()}
            x, meta = cues.build_all(pool, schemas, split, guesses, variant)
            p1 = first.predict_proba(x)[:, 1].astype(np.float32)
            p2 = second.predict(gf(meta, p1)).astype(np.float32)
            where = {s["id"]: i for i, s in enumerate(schemas)}
            tables = [{t["name"]: j for j, t in enumerate(s["tables"])} for s in schemas]
            columns = [[{c["name"]: k for k, c in enumerate(t["columns"])} for t in s["tables"]] for s in schemas]
            n = len(meta)
            rec = {"model": model, "variant": variant, "set": data["name"], "threshold": final["threshold"],
                   "schema": np.zeros(n, np.int32), "table": np.zeros(n, np.int32), "column": np.zeros(n, np.int32),
                   "target": np.zeros(n, np.int32), "key": np.zeros(n, np.int32),
                   "label": np.zeros(n, bool), "rules": np.zeros(n, bool), "first": p1, "second": p2}
            for i, m in enumerate(meta):
                si = where[m[graph.SID]]
                ti, gi = tables[si][m[graph.TABLE]], tables[si][m[graph.TARGET]]
                rec["schema"][i], rec["table"][i], rec["target"][i] = si, ti, gi
                rec["column"][i] = columns[si][ti][m[graph.COLUMN]]
                rec["key"][i] = columns[si][gi][m[widen.KEY]]
                rec["label"][i], rec["rules"][i] = m[graph.LABEL], m[graph.RULES]
            pickle.dump(rec, open(OUT / "scores" / model / f"{slug(name)}.pkl", "wb"), protocol=4)
            kept = keys.one_target(p2.astype(np.float64), [(m[graph.SID], m[graph.TABLE], m[graph.COLUMN]) for m in meta])
            drawn = kept >= final["threshold"]
            tp = int((drawn & rec["label"]).sum())
            total = sum(len(s["fk"]) for s in schemas)
            p = tp / max(int(drawn.sum()), 1)
            r = tp / max(total, 1)
            print(f"{model:16} {name:34} pairs {n:8d} P {p:.3f} R {r:.3f} F1 {2 * p * r / max(p + r, 1e-9):.3f}"
                  f"  ({time.time() - t0:.0f} s)", flush=True)
            del x, meta


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "sets":
        write_sets()
    else:
        score(sys.argv[2], sys.argv[3:])
