"""Round 1m's bench counted: each story database read by round 1's chain,
its pool run as round 1e's (thirty best by round 1c's ranker, both fives),
the look of round 1j taught on the 93 public databases scoring it, and every
five counted by what its spec planted — the stories a panel of the five
reads, the decoys it shows, the mirrors it shows twice.

    python -I bench.py see <bench dir>
    python -I bench.py pool <bench dir>
    python -I bench.py count <bench dir> <round 1l learn dir>

see runs round 1's see.sh on each built story_<key>, into corpus/see/story<key>-postgres;
pool writes <bench dir>/look/<fixture>/pool.json; count writes <bench dir>/count.json.
A story is found when a panel of the five reads what was planted — the
column whose values or money changed, or, for a rate that changed, the
table drawn over a time — and found over time when that panel is also drawn
over a time.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(ROOT / "round-1c"))
sys.path.insert(0, str(ROOT / "round-1k"))
sys.path.insert(0, str(HERE))
import learn  # noqa: E402
import local  # noqa: E402
import rank  # noqa: E402
import roles  # noqa: E402
from aq import generate, identity, load  # noqa: E402

SEE = ROOT / "corpus" / "see"
PG = "host=127.0.0.1 port=55432"


def fixtures(bench: Path) -> list[tuple[str, str]]:
    return [(p.parent.name, f"story{p.parent.name}-postgres") for p in sorted(bench.glob("m*/spec.json"))]


def see(bench: Path) -> None:
    for key, fe in fixtures(bench):
        about = json.loads((bench / key / "spec.json").read_text(encoding="utf-8")).get("about", "")
        db = f"story_{key}"
        p = subprocess.run(["sh", str(ROOT / "round-1" / "see.sh"), "postgres", f"story{key}",
                            f"{PG} user=lemmesee password=lemmesee dbname={db}",
                            f"{PG} user=postgres password=lemmesee-admin dbname={db}",
                            f"{about} A story fixture of round 1m: ninety days made from a spec."],
                           capture_output=True, text=True, encoding="utf-8", errors="replace")
        print(key, "ok" if p.returncode == 0 else f"failed: {p.stderr.strip()[-300:]}", flush=True)


def pool(bench: Path) -> None:
    look1d = local.module("bm_look1d", ROOT / "round-1d" / "look.py")
    fes = [fe for _, fe in fixtures(bench) if (SEE / fe / "snapshot.json").exists()]
    look1d.pool(ROOT / "corpus" / "round-1c" / "learn", ROOT / "corpus" / "round-1d", SEE, bench / "look", fes, 30)


def refs(schema, q) -> set[tuple[str, str]]:
    """The table and column pairs a question reads."""
    T = lambda t, c: (schema.tables[t].name, schema.tables[t].columns[c].name)
    out = {(schema.tables[q.base].name, "*")}
    for t, c in q.measure.cols:
        out.add(T(t, c))
    if q.time:
        out.add(T(q.time.t, q.time.c))
    if q.by:
        if q.by.c >= 0:
            out.add(T(q.by.t, q.by.c))
        else:
            tb = schema.tables[q.by.t]
            out.add((tb.name, "*"))
            if tb.label is not None:
                out.add(T(q.by.t, tb.label))
            if q.by.path:
                out.add(T(*q.by.path[-1]))
    return out


def look_scores(bench: Path, fe: str, panels: list[dict]):
    """round 1j's baseline look, taught on all 93 public databases by their
    teachers' mean, scoring a story database's panels: live.py's way."""
    sys.argv = [sys.argv[0]]
    joint = local.module("bm_joint", ROOT / "round-1j" / "joint.py")
    dg = local.module("bm_diagnose", ROOT / "round-1g" / "diagnose.py")
    look1d, look1e = dg.look1d, dg.look1e
    frozen = ROOT / "corpus" / "round-1j"
    meta = json.loads((frozen / "data.json").read_text(encoding="utf-8"))
    A = dict(np.load(frozen / "data.npz", allow_pickle=False))
    names = meta["numeric_names"]
    look1d.QUESTION[:] = names[len(look1d.RESULT) + 2:len(names) - len(look1e.MORE)]
    _, qfeat = look1e.questions(SEE, fe)
    for p in panels:
        p["question"] = qfeat.get(p["title"], {})
        p["more"] = look1e.more(p)
    X = np.vstack((A["base"], np.asarray([look1e.vector(p) for p in panels], dtype=np.float32)))
    y = np.r_[A["y"], np.full(len(panels), np.nan)]
    train = np.r_[np.ones(len(A["base"]), dtype=bool), np.zeros(len(panels), dtype=bool)]
    pred, _ = joint.fit_predict("baseline", X, y, train, ~train, joint.SEED)
    return pred, dg, look1e


def count(bench: Path, learn_new: Path) -> None:
    import lightgbm as lgb
    sets = local.module("bm_sets", ROOT / "round-1g" / "sets.py")
    mw = roles.money_words()
    rankers = {"round 1c": ROOT / "corpus" / "round-1c" / "learn",
               "luna and deepseek": learn_new / "luna-and-deepseek",
               "luna, deepseek and jev": learn_new / "luna-deepseek-and-jev"}
    models = {n: lgb.Booster(model_file=str(d / "ranker.txt")) for n, d in rankers.items()}
    per: dict[str, dict] = {}
    for key, fe in fixtures(bench):
        pool_p = bench / "look" / fe / "pool.json"
        if not pool_p.exists():
            continue
        spec = json.loads((bench / key / "spec.json").read_text(encoding="utf-8"))
        snap = json.loads((SEE / fe / "snapshot.json").read_text(encoding="utf-8"))
        schema = load(snap, True)
        qs = [q for q in generate(schema) if rank.veto(schema, q) is None]
        by_title = {}
        for q in qs:
            by_title.setdefault(q.title(schema), q)
        # the ranker's own fives, over every question, with and without roles
        row = {"features": [rank.features(schema, q) for q in qs], "slots": [rank.slots(schema, q) for q in qs]}
        points = np.array([rank.stats_points(schema, q) for q in qs])
        ids = [identity(schema, q) for q in qs]
        qroles = [roles.role(i, sl, mw) for i, sl in zip(ids, row["slots"])]
        fives = {}
        for name, m in models.items():
            X, _, _ = learn.matrix_free(row, m.feature_name(), None)
            s = m.predict(X) + points
            fives[name] = [qs[i] for i in rank.choose(schema, qs, list(s))]
            fives[f"{name}, roles"] = [qs[i] for i in role_five_free(schema, qs, ids, s, qroles)]
            fives[f"{name}, screen"] = [qs[i] for i in roles.screen(schema, qs, ids, s, row["slots"], mw)]
        # the look over the pool: round 1d's ten, and round 1e's thirty
        panels = [p for p in json.loads(pool_p.read_text(encoding="utf-8"))["panels"] if "round 1" not in p["in"]]
        panels = [p for p in panels if p["title"] in by_title]
        pred, dg, look1e = look_scores(bench, fe, panels)
        pids = [identity(schema, by_title[p["title"]]) for p in panels]
        proles = [roles.role(i, rank.slots(schema, by_title[p["title"]]), mw) for i, p in zip(pids, panels)]
        # round 1d's pool as the owner's page read it: the ranker's ten best questions and its five
        ten, seen = [], set()
        for k in sorted(range(len(panels)), key=lambda k: -panels[k].get("score", -1e9)):
            if pids[k] not in seen and len(seen) < 10:
                seen.add(pids[k])
                ten.append(k)
        ten = sorted(set(ten) | {k for k, p in enumerate(panels) if "round 1c" in p["in"]})
        for depth, idx in (("ten", ten), ("thirty", list(range(len(panels))))):
            ch = look1e.Chooser([pids[k] for k in idx])
            s = pred[idx]
            fives[f"look over {depth}"] = [by_title[panels[idx[i]]["title"]] for i in ch.choose(s, sets.AXIS_ONCE)]
            pick = roles.choose(ch, s, [proles[k] for k in idx], sets.AXIS_ONCE, look1e.choose.SHARED)
            fives[f"look over {depth}, roles"] = [by_title[panels[idx[i]]["title"]] for i in pick]
        per[key] = {name: judge(schema, spec, five) for name, five in fives.items()}
        per[key]["about"] = spec.get("about", "")
        print(key, {n: f"{r['found']}/{r['stories']}" for n, r in per[key].items() if isinstance(r, dict)}, flush=True)
    systems = [n for n in next(iter(per.values())) if n != "about"]
    total = {}
    for n in systems:
        rs = [per[k][n] for k in per]
        total[n] = {"stories found": round(sum(r["found"] for r in rs) / max(1, sum(r["stories"] for r in rs)), 3),
                    "found over time": round(sum(r["found over time"] for r in rs) / max(1, sum(r["stories"] for r in rs)), 3),
                    "decoys shown a five": round(float(np.mean([r["decoys"] for r in rs])), 2),
                    "mirrors twice a five": round(float(np.mean([r["mirrors"] for r in rs])), 2),
                    "databases": len(rs)}
    (bench / "count.json").write_text(json.dumps({"total": total, "per database": per}, indent=1, ensure_ascii=False),
                                      encoding="utf-8")
    print(f"\n{'five':32} {'found':>6} {'over time':>10} {'decoys':>7} {'mirrors':>8}")
    for n, r in total.items():
        print(f"{n:32} {r['stories found']:6.2f} {r['found over time']:10.2f} {r['decoys shown a five']:7.2f} "
              f"{r['mirrors twice a five']:8.2f}")


def reach(bench: Path) -> None:
    """For every story, the questions that would show it over time: whether
    the generator makes one, and where round 1c's ranker places the best."""
    import lightgbm as lgb
    model = lgb.Booster(model_file=str(ROOT / "corpus" / "round-1c" / "learn" / "ranker.txt"))
    places, kinds = [], {}
    for key, fe in fixtures(bench):
        spec = json.loads((bench / key / "spec.json").read_text(encoding="utf-8"))
        schema = load(json.loads((SEE / fe / "snapshot.json").read_text(encoding="utf-8")), True)
        qs = [q for q in generate(schema) if rank.veto(schema, q) is None]
        row = {"features": [rank.features(schema, q) for q in qs], "slots": [rank.slots(schema, q) for q in qs]}
        X, _, _ = learn.matrix_free(row, model.feature_name(), None)
        s = model.predict(X) + np.array([rank.stats_points(schema, q) for q in qs])
        order, seen, place = np.argsort(-s, kind="stable"), set(), {}
        for i in order:
            ident = identity(schema, qs[i])
            if ident not in seen:
                seen.add(ident)
                place[int(i)] = len(seen)
        for st in spec.get("stories", []):
            ch = st.get("change") or {}
            kind = "values" if "values" in ch else "money" if "times" in ch else "rate"
            shows = [i for i, q in enumerate(qs) if q.time is not None and i in place and
                     judge(schema, {"stories": [st]}, [q])["found over time"]]
            best = min((place[i] for i in shows), default=None)
            places.append(best)
            kinds.setdefault(kind, []).append(best)
            bt = qs[min(shows, key=lambda i: place[i])].title(schema) if shows else "none made"
            print(f"{key} {st.get('id'):24} {kind:6} best place {best}: {bt}")
    made = [p for p in places if p is not None]
    print(f"\n{len(made)} of {len(places)} stories have a question that shows them over time; "
          f"its place: median {np.median(made):.0f}, in the ten {sum(p <= 10 for p in made)}, "
          f"in the thirty {sum(p <= 30 for p in made)}")
    for k, v in kinds.items():
        m = [p for p in v if p is not None]
        print(f"  {k}: {len(m)} of {len(v)} made, median place {np.median(m) if m else float('nan'):.0f}")


def role_five_free(schema, qs, ids, s, qroles, k: int = 5, depth: int = 30) -> list[int]:
    """Roles over the ranker's order, with rank.choose's limits: no time
    column or breakdown twice, at most two from a table, one question once."""
    order = [int(i) for i in np.argsort(-s, kind="stable")]
    chosen, tables, times, bys, asked = [], {}, set(), set(), set()

    def fits(i):
        q = qs[i]
        t = (q.time.t, q.time.c) if q.time else None
        return not (ids[i] in asked or tables.get(q.base, 0) >= 2 or (t and t in times) or (ids[i][3] and ids[i][3] in bys))

    def take(i):
        q = qs[i]
        chosen.append(i)
        asked.add(ids[i])
        tables[q.base] = tables.get(q.base, 0) + 1
        if q.time:
            times.add((q.time.t, q.time.c))
        if ids[i][3]:
            bys.add(ids[i][3])

    for r in roles.ORDER:
        i = next((i for i in order[:depth] if qroles[i] == r and fits(i)), None)
        if i is not None:
            take(i)
    for i in order:
        if len(chosen) >= k:
            break
        if fits(i) and not (qroles[i] == "pulse" and any(qroles[j] == "pulse" for j in chosen)):
            take(i)
    return sorted(chosen[:k], key=lambda i: -s[i])


def judge(schema, spec: dict, five: list) -> dict:
    stories = spec.get("stories", [])
    found = over_time = 0
    for st in stories:
        # what was planted says what shows it: the changed column read, or a timeline of the table that grew
        ch = st.get("change") or {}
        if ch.get("column"):
            want = {(st.get("table"), ch["column"])}
            hits = [q for q in five if refs(schema, q) & want]
        elif "rate" in ch:
            hits = [q for q in five if schema.tables[q.base].name == st.get("table") and q.time is not None]
        else:
            want = {(e.get("table"), e.get("column")) for e in st.get("seen_in", [])}
            hits = [q for q in five if refs(schema, q) & want]
        found += bool(hits)
        over_time += any(q.time is not None for q in hits)
    bases = [schema.tables[q.base].name for q in five]
    decoys = mirrors = 0
    for d in spec.get("decoys", []):
        tabs = set(d.get("tables", []))
        if d.get("kind") == "mirror":
            timed = {schema.tables[q.base].name for q in five if q.time is not None and schema.tables[q.base].name in tabs}
            mirrors += len(timed) >= 2
        else:
            decoys += sum(1 for b in bases if b in tabs)
    return {"stories": len(stories), "found": found, "found over time": over_time, "decoys": decoys,
            "mirrors": mirrors, "five": [q.title(schema) for q in five]}


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    cmd, *a = sys.argv[1:]
    if cmd == "see":
        see(Path(a[0]))
    elif cmd == "pool":
        pool(Path(a[0]))
    elif cmd == "count":
        count(Path(a[0]), Path(a[1]))
    elif cmd == "reach":
        reach(Path(a[0]))
