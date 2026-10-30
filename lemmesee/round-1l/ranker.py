"""Round 1c's ranker taught the product's way: its features, labels, folds
and LightGBM settings unchanged, fitted in one environment by each set of
teachers — round 1c's (Haiku, Gemini), the product's writers (Luna,
DeepSeek), Jev scoring the generator's questions (jevrank.py), the product's
three, and all five — and the fits compared.

    python -I ranker.py build <round 1c teacher dir> <round 1l teacher dir> <out>
    python -I ranker.py cv <out>
    python -I ranker.py fit <out>
    python -I ranker.py judged <out> <corpus/see> <round 1j frozen dir>
    python -I ranker.py pool <out> <corpus/see> <round 1j frozen dir> <look dir> [<set>...]
    python -I ranker.py count <look dir> <round 1j frozen dir> <judge>...
    python -I ranker.py live <out> <see dir> <fixture> [<title words>...]

build writes <out>/data.pkl, every schema's questions labelled by each
teacher apart. cv fits each set of teachers over round 1c's five folds by
repository and measures its five against every teacher it did not see:
<out>/cv.json. fit writes <out>/<set>/ranker.txt and words.json, which round
1d's pool reads in place of round 1c's. judged scores the 93 public
databases' questions with each fit and counts its five by the judges that
saw their panels — the first (Sonnet, Gemini) and the product's (pmsonnet,
pmgemini) — where they saw them: <out>/judged.json. pool runs every
database's ten best and five by each fit on the local copies and writes them
as round 1d's pool, which round 1d's pack, gemini.py cut and round 1k's
product.py take to judges; count counts each fit's five and ten by the
judges named, where all of them saw every panel. live prints, for a
database whose data stays on this machine, each fit's ten best and where the
questions whose titles hold the given words fall; nothing is written.
"""

from __future__ import annotations

import json
import math
import pickle
import random
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT / "round-1c"))
import learn  # noqa: E402
import rank  # noqa: E402
from aq import generate, identity, load  # noqa: E402

SETS = {"first teachers": ("haiku", "gemini"), "luna and deepseek": ("luna", "deepseek"), "jev": ("jev",),
        "luna, deepseek and jev": ("luna", "deepseek", "jev"), "all five": ("haiku", "gemini", "luna", "deepseek", "jev"),
        "votes of the three": ("votes3",), "votes of the five": ("votes5",)}
# a set's label is its teachers' largest; votes are the teachers' labels summed, so a question
# two teachers put first outranks one a single teacher did: 0 none, 1 to 2, 3 to 5, 6 and more
VOTES = {"votes3": ("luna", "deepseek", "jev"), "votes5": ("haiku", "gemini", "luna", "deepseek", "jev")}


def votes(r: dict, teachers: tuple) -> list[int]:
    have = [np.array(r["teachers"][t]["labels"]) for t in teachers if t in r["teachers"]]
    s = np.sum(have, axis=0)
    return np.select([s >= 6, s >= 3, s >= 1], [3, 2, 1], 0).tolist()
JUDGES = {"first judges": ("sonnet", "gemini"), "product judges": ("pmsonnet", "pmgemini")}


def only(rows, teachers):
    """The rows a set of teachers answered, with their labels alone."""
    out = []
    for r in rows:
        t = {k: v for k, v in r["teachers"].items() if k in teachers}
        if t:
            out.append({**r, "teachers": t})
    return out


def jev_labels(ids: list, titles: list[str], chance: dict[str, float]) -> tuple[list[int], list]:
    """Jev's chances as a teacher's ten: its first five by chance are a
    teacher's first five, its next five the rest of its ten, and the labels
    follow round 1c's labels_of."""
    by_title = {t: ids[i] for i, t in reversed(list(enumerate(titles)))}
    asked = []
    for t, _ in sorted(chance.items(), key=lambda x: -x[1]):
        if t in by_title and by_title[t] not in asked:
            asked.append(by_title[t])
        if len(asked) == 10:
            break
    exact = {g: 3 if r < 5 else 2 for r, g in enumerate(asked)}
    partial_m = {(g[0], g[1]) for g in asked}
    partial_d = {(g[0], g[2], g[3]) for g in asked}
    lab = [exact.get(i, 1 if (i[0], i[1]) in partial_m or (i[0], i[2], i[3]) in partial_d else 0) for i in ids]
    return lab, asked


def build(first: Path, product: Path, out: Path) -> None:
    where = {"haiku": first, "gemini": first, "luna": product, "deepseek": product}

    def answers(_tdir):
        got = {}
        for t, d in where.items():
            got[t] = {}
            for p in sorted((d / f"answers-{t}").glob("*.json")):
                try:
                    a = json.loads(p.read_text(encoding="utf-8"))
                except json.JSONDecodeError:
                    continue
                got[t] |= {k: qs for k, qs in a.items() if isinstance(qs, list) and qs}
        return got

    learn.TEACHERS = tuple(where)
    learn.teacher_answers = answers
    learn.build(first, out)
    rows = pickle.load((out / "data.pkl").open("rb"))
    n = 0
    for r in rows:
        p = product / "answers-jev" / f"{r['key']}.json"
        if p.exists():
            lab, asked = jev_labels(r["ids"], r["titles"], json.loads(p.read_text(encoding="utf-8")))
            r["teachers"]["jev"] = {"labels": lab, "asked": asked}
            n += 1
        for name, teachers in VOTES.items():
            r["teachers"][name] = {"labels": votes(r, teachers), "asked": []}
    with (out / "data.pkl").open("wb") as f:
        pickle.dump(rows, f)
    print(f"jev: {n} schemas")


def cv(out: Path, folds: int = 5, rounds: int = 600) -> None:
    rows = pickle.load((out / "data.pkl").open("rb"))
    names = learn.all_names(rows)
    repos = sorted({r["repo"] for r in rows})
    random.Random(20261028).shuffle(repos)
    fold_of = {repo: i % folds for i, repo in enumerate(repos)}
    for k in range(folds):
        test = [r for r in rows if fold_of[r["repo"]] == k]
        for name, teachers in SETS.items():
            train = only([r for r in rows if fold_of[r["repo"]] != k], teachers)
            model = learn.fit_one(train, names, rounds, learn.word_table(train))
            for r in test:
                X, _, _ = learn.matrix_free(r, names, None)
                r[f"_{name}"] = model.predict(X)
        for r in test:
            r["_prior"] = [rank.prior(f) for f in r["features"]]
        print(f"fold {k} done", flush=True)
    result = {"schemas": len(rows), "answered by": {t: sum(1 for r in rows if t in r["teachers"])
                                                    for ts in SETS.values() for t in ts}}
    for against, teachers in (("against luna and deepseek", SETS["luna and deepseek"]), ("against jev", ("jev",)),
                              ("against the first teachers", SETS["first teachers"])):
        rs = only(rows, teachers)
        result[against] = {"prior": learn.measure(rs, lambda r: r["_prior"])}
        for name in SETS:
            result[against][name] = learn.measure(rs, lambda r, n=name: r[f"_{n}"])
    agree = {}
    for a in SETS["all five"]:
        for b in SETS["all five"]:
            if a == b:
                continue
            both = [r for r in rows if a in r["teachers"] and b in r["teachers"]]
            agree[f"{a}'s five in {b}'s ten"] = float(np.mean([
                len({x for x in r["teachers"][a]["asked"][:5] if x} & {x for x in r["teachers"][b]["asked"] if x}) / 5
                for r in both])) if both else math.nan
    result["teachers agree"] = agree
    (out / "cv.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(json.dumps(result, indent=1))


def fit(out: Path, rounds: int = 600) -> None:
    rows = pickle.load((out / "data.pkl").open("rb"))
    names = learn.all_names(rows)
    for name, teachers in SETS.items():
        rs = only(rows, teachers)
        table = learn.word_table(rs)
        model = learn.fit_one(rs, names, rounds, table)
        d = out / name.replace(",", "").replace(" ", "-")
        d.mkdir(parents=True, exist_ok=True)
        model.save_model(str(d / "ranker.txt"))
        words = {"prior": table["prior"],
                 "slots": {s: {w: [round(c[0]), round(c[1])] for w, c in t.items()} for s, t in table["slots"].items()}}
        (d / "words.json").write_text(json.dumps(words, separators=(",", ":")), encoding="utf-8")
        imp = sorted(zip(names, model.feature_importance("gain")), key=lambda x: -x[1])
        print(f"{name}: {len(rs)} schemas, {model.num_trees()} trees; first by gain:",
              ", ".join(n for n, _ in imp[:8]))


def models(out: Path) -> dict:
    import lightgbm as lgb
    got = {"round 1c as shipped": ROOT / "corpus" / "round-1c" / "learn"}
    got |= {name: out / name.replace(",", "").replace(" ", "-") for name in SETS}
    return {name: lgb.Booster(model_file=str(d / "ranker.txt")) for name, d in got.items()}


def scored(snap: dict, ms: dict):
    """A database's questions as round 1d's pool scores them, by each fit."""
    schema = load(snap, True)
    qs = [q for q in generate(schema) if rank.veto(schema, q) is None]
    row = {"features": [rank.features(schema, q) for q in qs], "slots": [rank.slots(schema, q) for q in qs]}
    points = np.array([rank.stats_points(schema, q) for q in qs])
    out = {}
    for name, m in ms.items():
        X, _, _ = learn.matrix_free(row, m.feature_name(), None)
        out[name] = m.predict(X) + points
    return schema, qs, out


def auc(s: np.ndarray, y: np.ndarray) -> float:
    pos, neg = s[y > 0.5], s[y < 0.5]
    if not len(pos) or not len(neg):
        return math.nan
    return float(np.mean((pos[:, None] > neg[None, :]) + 0.5 * (pos[:, None] == neg[None, :])))


def judged(out: Path, see_dir: Path, frozen: Path) -> None:
    sys.path.insert(0, str(ROOT / "round-1k"))
    import local
    sys.argv = [sys.argv[0]]
    joint = local.module("rl_joint", ROOT / "round-1j" / "joint.py")
    look1d = local.module("rl_look1d", ROOT / "round-1d" / "look.py")
    meta = json.loads((frozen / "data.json").read_text(encoding="utf-8"))
    A = np.load(frozen / "data.npz")
    family = {str(db): str(f) for db, f in zip(A["db"], A["family"])}
    names = {j for js in JUDGES.values() for j in js}
    ans = {j: {} for j in names}
    for d in [ROOT / "corpus" / f"round-1{x}" / "look" for x in "def"]:
        for j in ans:
            ans[j] |= look1d.labels(d, j)
    by_db = {}
    for p in meta["panels"]:
        by_db.setdefault(p["db"], []).append(p)
    ms = models(out)
    per = {(name, jn, m): {} for name in ms for jn in JUDGES for m in ("kept of judged", "judged of five", "auc")}
    for db, panels in sorted(by_db.items()):
        snap = json.loads((see_dir / db / "snapshot.json").read_text(encoding="utf-8"))
        schema, qs, scores = scored(snap, ms)
        title_ix = {}
        for i, q in enumerate(qs):
            title_ix.setdefault(q.title(schema), i)
        for jn, js in JUDGES.items():
            y_of = {}
            for p in panels:
                v = [float(bool(ans[j][p["id"]]["keep"])) for j in js if p["id"] in ans[j]]
                if len(v) == len(js) and p["title"] in title_ix:
                    y_of[title_ix[p["title"]]] = float(np.mean(v))
            if not y_of:
                continue
            ix = np.array(sorted(y_of))
            y = np.array([y_of[i] for i in ix])
            for name, s in scores.items():
                ten, five = ten_and_five(schema, qs, s)
                seen = [y_of[i] for i in five if i in y_of]
                per[(name, jn, "judged of five")][db] = len(seen) / 5
                if seen:
                    per[(name, jn, "kept of judged")][db] = float(np.mean(seen))
                seen = [y_of[i] for i in ten if i in y_of]
                per.setdefault((name, jn, "judged of ten"), {})[db] = len(seen) / len(ten)
                if seen:
                    per.setdefault((name, jn, "kept in ten"), {})[db] = float(np.mean(seen))
                a = auc(s[ix], y)
                if not math.isnan(a):
                    per[(name, jn, "auc")][db] = a
        print(db, len(qs), "questions", flush=True)
    result = {}
    for (name, jn, m), v in per.items():
        row = {"mean": round(float(np.mean(list(v.values()))), 3), "databases": len(v)}
        ref = per[("first teachers", jn, m)]
        if name != "first teachers":
            common = {db: x - ref[db] for db, x in v.items() if db in ref}
            row["minus first teachers"] = joint.paired_interval(common, family)
        result[f"{name} / {jn} / {m}"] = row
    (out / "judged.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(f"{'ranker taught by':22} {'judges':15} {'measure':15} {'mean':>6}   minus first teachers [95%]")
    for k, r in result.items():
        name, jn, m = k.split(" / ")
        d = r.get("minus first teachers")
        print(f"{name:22} {jn:15} {m:15} {r['mean']:6.3f}   " +
              (f"{d['mean']:+.3f} [{d['95%'][0]:+.3f}, {d['95%'][1]:+.3f}]" if d else ""))


def ten_and_five(schema, qs, s, n: int = 10) -> tuple[list[int], list[int]]:
    order = list(np.argsort(-s, kind="stable"))
    ten, seen = [], set()
    for i in order:
        ident = identity(schema, qs[i])
        if ident not in seen:
            seen.add(ident)
            ten.append(int(i))
        if len(ten) == n:
            break
    return ten, rank.choose(schema, qs, list(s))


def pool(out: Path, see_dir: Path, frozen: Path, look: Path, sets: list[str]) -> None:
    """Every database's ten best and five by each fit named (every fit when
    none is), run on the local copies of the 93 databases and written as
    round 1d's pool, to be packed for the judges that count: all of them, so
    one pair sees every five compared."""
    sys.path.insert(0, str(ROOT / "round-1k"))
    import local
    look1d = local.module("rl_look1d", ROOT / "round-1d" / "look.py")
    meta = json.loads((frozen / "data.json").read_text(encoding="utf-8"))
    ms = {name: m for name, m in models(out).items() if name in (sets or SETS)}
    for db in sorted({p["db"] for p in meta["panels"]}):
        snap = json.loads((see_dir / db / "snapshot.json").read_text(encoding="utf-8"))
        schema, qs, scores = scored(snap, ms)
        members: dict[int, list[str]] = {}
        for name, s in scores.items():
            ten, five = ten_and_five(schema, qs, s)
            for i in ten:
                members.setdefault(i, []).append(f"{name}, ten")
            for i in five:
                members.setdefault(i, []).append(f"{name}, five")
        panels = []
        for k, i in enumerate(sorted(members)):
            p = look1d.see.panel_of(schema, qs[i], snap["engine"], snap["database"], f"{db}:l{k}")
            p["in"] = members[i]
            p["result"] = look1d.result_features(p)
            panels.append(p)
        note = json.loads((see_dir / db / "runs.json").read_text(encoding="utf-8")).get("note", "")
        (look / db).mkdir(parents=True, exist_ok=True)
        (look / db / "pool.json").write_text(json.dumps({"fixture": db, "note": note, "panels": panels},
                                                        ensure_ascii=False), encoding="utf-8")
        print(db, len(panels), "panels,", sum(1 for p in panels if p.get("error")), "errors", flush=True)


def count(look: Path, frozen: Path, judges: list[str]) -> None:
    """Each fit's five and the best five of its ten, by the judges that saw
    every panel of the pool, their keep averaged; differences from the first
    teachers' fit bootstrapped over families."""
    sys.path.insert(0, str(ROOT / "round-1k"))
    import local
    sys.argv = [sys.argv[0]]
    joint = local.module("rl_joint", ROOT / "round-1j" / "joint.py")
    look1d = local.module("rl_look1d", ROOT / "round-1d" / "look.py")
    A = np.load(frozen / "data.npz")
    family = {str(db): str(f) for db, f in zip(A["db"], A["family"])}
    ans = {j: look1d.labels(look, j) for j in judges}
    per: dict[tuple, dict] = {}
    for f in sorted(look.glob("*/pool.json")):
        db = f.parent.name
        panels = json.loads(f.read_text(encoding="utf-8"))["panels"]
        y = {}
        for p in panels:
            v = [float(bool(ans[j][p["id"]]["keep"])) for j in judges if p["id"] in ans[j]]
            if len(v) == len(judges):
                y[p["id"]] = float(np.mean(v))
        for name in {s.rsplit(", ", 1)[0] for p in panels for s in p["in"]}:
            five = [p["id"] for p in panels if f"{name}, five" in p["in"]]
            ten = [p for p in panels if f"{name}, ten" in p["in"]]
            if five and all(i in y for i in five):
                per.setdefault((name, "kept of its five"), {})[db] = sum(y[i] for i in five)
            if ten and all(p["id"] in y for p in ten):
                per.setdefault((name, "kept in its ten"), {})[db] = sum(y[p["id"]] for p in ten)
    result = {}
    for (name, m), v in per.items():
        row = {"mean": round(float(np.mean(list(v.values()))), 3), "databases": len(v)}
        ref = per.get(("first teachers", m), {})
        if name != "first teachers":
            row["minus first teachers"] = joint.paired_interval({db: x - ref[db] for db, x in v.items() if db in ref},
                                                                family)
        result[f"{name} / {m}"] = row
    (look / f"count-{'-'.join(judges)}.json").write_text(json.dumps(result, indent=1), encoding="utf-8")
    print(f"judges: {', '.join(judges)}")
    for k, r in result.items():
        d = r.get("minus first teachers")
        print(f"  {k:40} {r['mean']:6.2f} ({r['databases']} dbs)  " +
              (f"{d['mean']:+.2f} [{d['95%'][0]:+.2f}, {d['95%'][1]:+.2f}]" if d else ""))


def live(out: Path, see_dir: Path, fixture: str, words: list[str]) -> None:
    snap = json.loads((see_dir / fixture / "snapshot.json").read_text(encoding="utf-8"))
    ms = models(out)
    schema, qs, scores = scored(snap, ms)
    for name, s in scores.items():
        order = list(np.argsort(-s, kind="stable"))
        top, seen = [], set()
        for i in order:
            ident = identity(schema, qs[i])
            if ident not in seen:
                seen.add(ident)
                top.append(int(i))
        print(f"\n== {name}: {len(qs)} questions")
        for r, i in enumerate(top[:10], 1):
            print(f"  {r:3d}. {qs[i].title(schema)}")
        for w in words:
            hits = [(r, qs[i].title(schema)) for r, i in enumerate(top, 1) if w.lower() in qs[i].title(schema).lower()]
            print(f"  '{w}': " + "; ".join(f"{r}. {t}" for r, t in hits[:4]))
        five = rank.choose(schema, qs, list(s))
        print("  five: " + " | ".join(qs[i].title(schema) for i in five))
        sys.path.insert(0, str(ROOT / "round-1m"))
        import roles
        ids = [identity(schema, q) for q in qs]
        shown = roles.screen(schema, qs, ids, s, [rank.slots(schema, q) for q in qs], roles.money_words())
        print("  screen (round 1m): " + " | ".join(qs[i].title(schema) for i in shown))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, *a = sys.argv[1:]
    if cmd == "build":
        build(Path(a[0]), Path(a[1]), Path(a[2]))
    elif cmd == "cv":
        cv(Path(a[0]))
    elif cmd == "fit":
        fit(Path(a[0]))
    elif cmd == "judged":
        judged(Path(a[0]), Path(a[1]), Path(a[2]))
    elif cmd == "pool":
        pool(Path(a[0]), Path(a[1]), Path(a[2]), Path(a[3]), a[4:])
    elif cmd == "count":
        count(Path(a[0]), Path(a[1]), a[2:])
    elif cmd == "live":
        live(Path(a[0]), Path(a[1]), a[2], a[3:])
