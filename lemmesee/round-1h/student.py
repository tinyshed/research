"""Round 1h's student of names: what a panel's title is worth, learned from
the three teachers' keep by title on SchemaPile's schemas, folds by
repository; then fitted on all and run on the judged databases' pools, its
score a number the look reads.

    python -I student.py measure <out> <round 1g out>
    python -I student.py fit <out> <round 1g out> <corpus/see> <look dir>...

measure writes <out>/student.json — AUC within a schema against the
teachers, out of fold, for each set of what the student reads; fit writes
<out>/names.json, each judged panel's score by id, and <out>/student.bin,
the student packed.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "round-1c"))
import vectors  # noqa: E402

SEED = 20261105
FOLDS = 5
SLOTS = ("base", "measure", "time", "by")
PROPS = ("id", "money", "count", "sum_ok", "audit_time", "event_time", "good_group", "cryptic", "importance")
SMOOTH = 10.0


def words_of(slots: dict, s: str) -> list[str]:
    return [w.split(":", 1)[-1] for w in slots.get(s, []) if not w.startswith("#")]


class Reader:
    """What the student reads of a panel besides round 1c's numbers: the
    lexicon's, the vectors' and, once a table is set, each word's keep rate."""

    def __init__(self, g_out: Path, h_out: Path):
        self.lex = json.loads((g_out / "lexicon" / "lexicon.json").read_text(encoding="utf-8"))["words"]
        self.vec = vectors.load(h_out)
        self.rates: dict | None = None

    def lexicon(self, slots: dict) -> list[float]:
        out = []
        for s in SLOTS:
            known = [self.lex[w] for w in words_of(slots, s) if w in self.lex]
            out += [max(v[i] for v in known) if known else math.nan for i in range(len(PROPS))]
        return out

    def vectors(self, slots: dict) -> list[float]:
        out = []
        for s in SLOTS:
            vs = [self.vec[w] for w in words_of(slots, s) if w in self.vec]
            out += list(np.mean(vs, axis=0)) if vs else [math.nan] * vectors.DIMS
        return out

    def kept(self, slots: dict, own: dict | None = None) -> list[float]:
        out = []
        prior = self.rates["prior"]
        for s in SLOTS:
            r = []
            for w in set(slots.get(s, [])):
                c = self.rates["slots"][s].get(w)
                if c is None:
                    continue
                pos, n = c
                if own and w in own[s]:
                    pos, n = pos - own[s][w][0], n - own[s][w][1]
                if n > 0:
                    r.append((pos + SMOOTH * prior) / (n + SMOOTH))
            out += [max(r), min(r), float(np.mean(r))] if r else [math.nan] * 3
        return out


def rate_table(panels: list[dict], y: dict[str, float]) -> tuple[dict, dict]:
    """Each word's keep, by where it stands, over these panels; and the same
    by repository, to take a repository's own out of its rows."""
    total = {s: {} for s in SLOTS}
    by_repo: dict = {}
    ys = []
    for p in panels:
        t = y.get(p["id"])
        if t is None:
            continue
        ys.append(t)
        own = by_repo.setdefault(p["repo"], {s: {} for s in SLOTS})
        for s in SLOTS:
            for w in set(p["slots"].get(s, [])):
                for d in (total[s], own[s]):
                    c = d.setdefault(w, [0.0, 0.0])
                    c[0] += t
                    c[1] += 1.0
    return {"prior": float(np.mean(ys)), "slots": total}, by_repo


def targets(out: Path) -> dict[str, float]:
    lab = {j: json.loads((out / "labels" / f"{j}.json").read_text(encoding="utf-8"))
           for j in ("jev", "deepseek", "glm") if (out / "labels" / f"{j}.json").exists()}
    ids = set().union(*lab.values())
    y = {}
    for i in ids:
        got = [float(lab[j][i]) for j in lab if i in lab[j]]
        y[i] = float(np.mean(got))
    return y


SETS = {
    "round 1c's numbers": ("numbers",),
    "and the lexicon": ("numbers", "lexicon"),
    "and the vectors": ("numbers", "vectors"),
    "and the words' keep": ("numbers", "kept"),
    "all": ("numbers", "lexicon", "vectors", "kept"),
}


def matrix(panels, reader, names, parts, own_by_repo=None):
    X = []
    for p in panels:
        row = []
        if "numbers" in parts:
            row += [p["features"].get(n, math.nan) for n in names] + [p["score"], float(p["rank"])]
        if "lexicon" in parts:
            row += reader.lexicon(p["slots"])
        if "vectors" in parts:
            row += reader.vectors(p["slots"])
        if "kept" in parts:
            row += reader.kept(p["slots"], own_by_repo.get(p["repo"]) if own_by_repo else None)
        X.append(row)
    return np.array(X, dtype=np.float32)


def train(X, y, Xv=None, yv=None):
    import lightgbm as lgb
    params = dict(objective="cross_entropy", learning_rate=0.05, num_leaves=31, min_data_in_leaf=40,
                  feature_fraction=0.5, bagging_fraction=0.8, bagging_freq=1, lambda_l2=1.0, verbose=-1, seed=SEED)
    ds = lgb.Dataset(X, y)
    if Xv is None:
        return lgb.train(params, ds, num_boost_round=600)
    dv = lgb.Dataset(Xv, yv, reference=ds)
    return lgb.train(params, ds, num_boost_round=3000, valid_sets=[dv],
                     callbacks=[lgb.early_stopping(100, verbose=False)])


def auc_within(panels, s, y) -> float:
    by: dict = {}
    for p, v in zip(panels, s):
        by.setdefault(p["schema"], []).append((y[p["id"]] >= 0.5, v))
    vals = []
    for rows in by.values():
        pos = [v for k, v in rows if k]
        neg = [v for k, v in rows if not k]
        if pos and neg:
            vals.append(sum((a > b) + 0.5 * (a == b) for a in pos for b in neg) / (len(pos) * len(neg)))
    return float(np.mean(vals))


def load_panels(out: Path) -> list[dict]:
    with (out / "panels.jsonl").open(encoding="utf-8") as f:
        return [json.loads(line) for line in f]


def measure(out: Path, g_out: Path) -> None:
    panels = load_panels(out)
    y = targets(out)
    panels = [p for p in panels if p["id"] in y]
    names = sorted({k for p in panels[:2000] for k in p["features"]})
    reader = Reader(g_out, out)
    repos = sorted({p["repo"] for p in panels})
    rng = np.random.default_rng(SEED)
    rng.shuffle(repos)
    fold = {r: i % FOLDS for i, r in enumerate(repos)}
    result = {"schemas": len({p["schema"] for p in panels}), "panels": len(panels),
              "kept by the teachers' mean": round(float(np.mean([y[p["id"]] >= 0.5 for p in panels])), 3)}
    result["round 1c's ranker alone, AUC"] = round(auc_within(panels, [p["score"] for p in panels], y), 3)
    for label, parts in SETS.items():
        oof = np.zeros(len(panels))
        for k in range(FOLDS):
            tr = [p for p in panels if fold[p["repo"]] != k]
            te_i = [i for i, p in enumerate(panels) if fold[p["repo"]] == k]
            # a fifth of the taught repositories stop the trees
            stop = {r for r in repos if fold[r] != k and hash(r) % 5 == 0}
            inner = [p for p in tr if p["repo"] not in stop]
            held = [p for p in tr if p["repo"] in stop]
            if "kept" in parts:
                reader.rates, own = rate_table(inner, y)
            else:
                own = None
            model = train(matrix(inner, reader, names, parts, own), np.array([y[p["id"]] for p in inner]),
                          matrix(held, reader, names, parts), np.array([y[p["id"]] for p in held]))
            oof[te_i] = model.predict(matrix([panels[i] for i in te_i], reader, names, parts))
        result[f"{label}, AUC"] = round(auc_within(panels, oof, y), 3)
        print(label, result[f"{label}, AUC"], flush=True)
    (out / "student.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(json.dumps(result, indent=1))


def fit(out: Path, g_out: Path, see_dir: Path, look_dirs: list[Path], parts: tuple = SETS["all"]) -> None:
    """The student on every schema, and its score of every judged panel,
    the judged questions read at tier S as the student learned them."""
    import pack
    from aq import generate, load
    import rank
    y = targets(out)
    panels = [p for p in load_panels(out) if p["id"] in y]
    names = sorted({k for p in panels[:2000] for k in p["features"]})
    reader = Reader(g_out, out)
    reader.rates, own = rate_table(panels, y)
    model = train(matrix(panels, reader, names, parts, own), np.array([y[p["id"]] for p in panels]))
    scores = {}
    for d in look_dirs:
        for pool in d.glob("*/pool.json"):
            fe = pool.parent.name
            snap = json.loads((see_dir / fe / "snapshot.json").read_text(encoding="utf-8"))
            schema = load(snap, False)
            by_title = {}
            for q in generate(schema):
                by_title.setdefault(q.title(schema), q)
            # a question only the row counts let the generator ask is read at tier S all the same
            full = load(snap, True)
            for q in generate(full):
                by_title.setdefault(q.title(full), q)
            ps = []
            for p in json.loads(pool.read_text(encoding="utf-8"))["panels"]:
                q = by_title.get(p["title"])
                if q is None:
                    continue
                ps.append({"id": p["id"], "repo": fe, "score": p.get("score", math.nan), "rank": p.get("rank", 99),
                           "slots": rank.slots(schema, q), "features": rank.features(schema, q)})
            if ps:
                for p, s in zip(ps, model.predict(matrix(ps, reader, names, parts))):
                    scores[p["id"]] = round(float(s), 5)
    (out / "names.json").write_text(json.dumps(scores), encoding="utf-8")
    data = pack.pack(model)
    (out / "student.bin").write_bytes(data)
    words = sum(len(v) for v in reader.rates["slots"].values())
    print(f"{len(scores)} judged panels scored; the student {len(data)} bytes packed, {model.num_trees()} trees; "
          f"its word table {words} entries")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "measure":
        measure(Path(sys.argv[2]), Path(sys.argv[3]))
    else:
        fit(Path(sys.argv[2]), Path(sys.argv[3]), Path(sys.argv[4]), [Path(x) for x in sys.argv[5:]])
