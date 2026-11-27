"""Round 1x: the row methods of the README built from rows.py's profiles and
scored beside lemmekey (`eleven--wide`), through round 1w's `kept.against`.

    python -I score.py <out folder of rows.py> <folder for the report's data>

Writes main.json (every method on K and on U, lemmekey and the two hybrid
rules, over all declared keys and over those with a non-empty column, each
with lemmekey minus it on the same resamples), curve.json (Zhang et al.'s
top X %), rows.json and rows.csv (what the rows are like, a database a
line, with its times), cost.json and checks.json.
"""

import csv
import json
import pickle
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / "round-1w"))

import numpy as np  # noqa: E402
from sklearn.tree import DecisionTreeClassifier  # noqa: E402

import kept  # noqa: E402  (round 1w's scoring of kept scores)
import whole  # noqa: E402  (round 1v's)

MODEL = kept.MODEL
SOURCES = ("spider", "bird")
SETS = ("K", "U")
LEMMEKEY = "lemmekey"


# --- names, for HoPF's and Rostin's name features -----------------------------

TOKEN = re.compile(r"[A-Z]+(?![a-z])|[A-Z]?[a-z]+|\d+")


def tokens(*names: str) -> list:
    return [m.group(0).lower() for n in names for m in TOKEN.finditer(n)]


def levenshtein(a: str, b: str) -> int:
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1]


def fuzzy(left: list, right: list, freq: dict) -> float:
    """Chaudhuri et al.'s similarity as HoPF's paper gives it: each token of
    the referenced side to its most similar unmatched token of the
    dependent side, weighted by ln(1 / f)."""
    free = list(left)
    sims, weights = [], []
    for tok in right:
        best, at = 0.0, None
        for k, cand in enumerate(free):
            s = 1.0 - levenshtein(tok, cand) / max(len(tok), len(cand))
            if at is None or s > best:
                best, at = s, k
        if at is not None:
            free.pop(at)
        sims.append(best)
        weights.append(np.log(1.0 / freq[tok]))
    if not sims:
        return 0.0
    if sum(weights) <= 0:
        return float(np.mean(sims))
    return float(np.dot(sims, weights) / sum(weights))


def plain(name: str) -> str:
    return re.sub(r"[^a-z0-9]", "", name.lower())


def f6(dep: str, ref_table: str, ref: str) -> int:
    a, b, tb = plain(dep), plain(ref), plain(ref_table) + plain(ref)
    if a in (b, tb):
        return 2
    if a and b and (a in b or b in a or a in tb or tb in a):
        return 1
    return 0


# --- a database as the methods see it -----------------------------------------

class Rows:
    def __init__(self, schema: dict, prof: dict):
        self.schema, self.prof = schema, prof
        self.tables = prof["tables"]
        self.feat = {(a, b, c, d): f for a, b, c, d, f in prof["features"]}
        self.inds = {"K": [tuple(x) for x in prof["inclusions into K"]],
                     "U": [tuple(x) for x in prof["inclusions into U"]]}
        cols = [tokens(t["name"], c["name"]) for t in self.tables for c in t["columns"]]
        self.freq = {}
        for toks in cols:
            for tok in set(toks):
                self.freq[tok] = self.freq.get(tok, 0) + 1 / len(cols)
        self.declared_columns = {}
        for d in prof["declared"]:
            if "state" in d and d["state"] in ("holds", "fails", "empty"):
                self.declared_columns[(d["table"].lower(), d["column"].lower(), d["target"].lower())] = d
        self.nonnull = {(t["name"], c["name"]): c["nonnull"] for t in self.tables for c in t["columns"]}

    def name(self, ti, ci, tj):
        return self.tables[ti]["name"], self.tables[ti]["columns"][ci]["name"], self.tables[tj]["name"]

    def tie(self, ind):
        ti, ci, tj, cj = ind
        return (self.feat[ind]["referenced distinct"], tj, cj)

    def by_column(self, which):
        out = {}
        for ind in self.inds[which]:
            out.setdefault(ind[:2], []).append(ind)
        return out

    def ind(self, which) -> set:
        return {self.name(ti, ci, min(c, key=self.tie)[2]) for (ti, ci), c in self.by_column(which).items()}

    def reachable(self, which) -> set:
        """Every (column, table) some inclusion of the set allows: the ceiling's predictions."""
        return {self.name(ti, ci, tj) for ti, ci, tj, _ in self.inds[which]}

    def zhang(self, which) -> list:
        """One target a column by the least distance, the columns in increasing distance."""
        out = []
        for (ti, ci), c in self.by_column(which).items():
            best = min(c, key=lambda i: (self.feat[i]["emd"], *self.tie(i)))
            out.append((self.feat[best]["emd"], ti, ci, self.name(ti, ci, best[2])))
        return [x[3] for x in sorted(out, key=lambda x: x[:3])]

    def hopf(self, which, names: bool) -> set:
        scored = []
        for ind in self.inds[which]:
            ti, ci, tj, cj = ind
            s = self.feat[ind]["bhattacharyya"]
            if names:
                left = tokens(self.tables[ti]["name"], self.tables[ti]["columns"][ci]["name"])
                right = tokens(self.tables[tj]["name"], self.tables[tj]["columns"][cj]["name"])
                s = (s + fuzzy(left, right, self.freq)) / 2
            scored.append((-s, *self.tie(ind), ti, ci, ind))
        scored.sort(key=lambda x: x[:6])
        n = len(self.tables)
        edges = {i: set() for i in range(n)}
        taken, out = set(), set()

        def reaches(a, b):
            seen, stack = set(), [a]
            while stack:
                x = stack.pop()
                if x == b:
                    return True
                if x not in seen:
                    seen.add(x)
                    stack.extend(edges[x])
            return False

        def connected():
            und = {i: set() for i in range(n)}
            for a, bs in edges.items():
                for b in bs:
                    und[a].add(b)
                    und[b].add(a)
            seen, stack = set(), [0]
            while stack:
                x = stack.pop()
                if x not in seen:
                    seen.add(x)
                    stack.extend(und[x])
            return len(seen) == n

        for *_, ind in scored:
            ti, ci, tj, cj = ind
            if (ti, ci) in taken:
                continue
            if ti == tj or reaches(tj, ti):
                continue
            t = self.schema["tables"][ti]
            if t["columns"][ci]["primary"] and len(t["pk"]) == 1 and self.feat[ind]["consecutive"]:
                continue
            taken.add((ti, ci))
            out.add(self.name(ti, ci, tj))
            edges[ti].add(tj)
            if connected():
                break
        return out

    def rostin_rows(self, which):
        """Each inclusion of the set with Rostin et al.'s ten features and its label."""
        inds = self.inds[which]
        as_dep, as_ref = {}, {}
        for ti, ci, tj, cj in inds:
            as_dep[(ti, ci)] = as_dep.get((ti, ci), 0) + 1
            as_ref[(tj, cj)] = as_ref.get((tj, cj), 0) + 1
        x, y = [], []
        for ind in inds:
            ti, ci, tj, cj = ind
            f = self.feat[ind]
            dep, ref = self.tables[ti]["columns"][ci]["name"], self.tables[tj]["columns"][cj]["name"]
            x.append([f["dependent distinct"], f["dependent distinct"] / f["referenced distinct"],
                      as_ref.get((ti, ci), 0), as_dep[(ti, ci)], as_ref[(tj, cj)],
                      f6(dep, self.tables[tj]["name"], ref), f["f7 length difference"], f["f8 out of range"],
                      int(plain(dep).endswith(("id", "key", "nr"))),
                      self.tables[ti]["rows"] / max(self.tables[tj]["rows"], 1)])
            d = self.declared_columns.get((self.tables[ti]["name"].lower(), dep.lower(), self.tables[tj]["name"].lower()))
            y.append(int(bool(d) and str(d.get("referenced", "")).lower() == ref.lower()))
        return inds, np.array(x, dtype=np.float64).reshape(len(inds), 10), np.array(y, dtype=np.int64)


NO_NAMES = [0, 1, 2, 3, 4, 6, 7, 9]


def rostin(train: list, test: list, which: str, names: bool) -> tuple[dict, dict]:
    cols = list(range(10)) if names else NO_NAMES
    xs, ys = zip(*[(x, y) for _, x, y in (r.rostin_rows(which) for r in train)])
    x, y = np.vstack(xs)[:, cols], np.concatenate(ys)
    tree = DecisionTreeClassifier(criterion="entropy", min_samples_leaf=2, random_state=0).fit(x, y)
    out = {}
    for r in test:
        inds, xt, _ = r.rostin_rows(which)
        preds = set()
        if inds:
            p = tree.predict_proba(xt[:, cols])[:, list(tree.classes_).index(1)] if 1 in tree.classes_ else np.zeros(len(inds))
            best = {}
            for ind, pi in zip(inds, p):
                if pi > 0.5:
                    k = (-pi, *r.tie(ind))
                    if ind[:2] not in best or k < best[ind[:2]][0]:
                        best[ind[:2]] = (k, ind)
            preds = {r.name(ind[0], ind[1], ind[2]) for _, ind in best.values()}
        out[r.schema["id"]] = preds
    info = {"training inclusions": int(len(y)), "of them declared keys": int(y.sum()),
            "leaves": int(tree.get_n_leaves()), "depth": int(tree.get_depth())}
    return out, info


# --- lemmekey and the hybrid rules ---------------------------------------------

def lemmekey(name: str):
    sc = whole.Scored(MODEL, name)
    d = whole.OUT / "second" / MODEL
    raw = np.load(d / "sets" / f"{whole.slug(name)}.npy").astype(np.float64)
    assert np.array_equal(np.asarray(sc.second, dtype=np.float64), raw), "the dump is not the kept second stage"
    assert sc.threshold == json.load(open(d / "second.json", encoding="utf-8"))["threshold"]
    return sc, kept.keys_of(sc)


def hybrids(rows: Rows, drawn: set, threshold: float) -> dict:
    t = rows.tables
    best = {}
    for ti, ci, tj, kj, score, state in rows.prof["lemmekey's best candidates"]:
        best[(t[ti]["name"], t[ti]["columns"][ci]["name"])] = (t[tj]["name"], score, state)
    filtered, dropped = set(), 0
    for (tn, cn, g) in drawn:
        target, score, state = best[(tn, cn)]
        assert target == g and score >= threshold, "a drawn key is not its column's best candidate"
        if state == "fails":
            dropped += 1
        else:
            filtered.add((tn, cn, g))
    have = {(tn, cn) for tn, cn, _ in drawn}
    promoted = {(tn, cn, target) for (tn, cn), (target, score, state) in best.items()
                if (tn, cn) not in have and state == "holds"}
    states = {}
    for (tn, cn, g) in drawn:
        s = best[(tn, cn)][2]
        states[s] = states.get(s, 0) + 1
    return {"filtered": filtered, "promoted": drawn | promoted, "both": filtered | promoted,
            "drawn keys by what the data say": states, "promoted keys": len(promoted)}


# --- the run --------------------------------------------------------------------

def brief(res: dict, base: str) -> dict:
    """kept.against's result as the report's lines: each method, and lemmekey minus it."""
    out = {}
    for n, by in res.items():
        e = by["all"]
        line = {k: e[k] for k in ("databases", "keys", "precision", "recall", "f1", "precision_95", "recall_95",
                                  "f1_95", "macro f1 over databases")}
        if n != base:
            line[f"{base} minus it"] = res[base]["all"][f"minus {n}"]
        out[n] = line
    return out


def run(root: Path, out: Path) -> None:
    out.mkdir(parents=True, exist_ok=True)
    frozen = kept.check_frozen()
    data, lk, sets = {}, {}, {}
    for src in SOURCES:
        sc, keys = lemmekey(src)
        schemas = [s for s in sc.schemas if s["fk"]]
        lk[src] = (sc, keys)
        sets[src] = schemas
        data[src] = [Rows(s, json.load(open(root / "profile" / src / f"{s['id'].split('/', 1)[1]}.json", encoding="utf-8")))
                     for s in schemas]

    main, curve, checks, per_db, cost = {}, {}, {}, [], {}
    # the check the README sets, before anything new is scored
    for src in SOURCES:
        res = kept.against(sets[src], {LEMMEKEY: lk[src][1]}, {}, ("all",))
        f = res[LEMMEKEY]["all"]["f1"]
        print(f"lemmekey on {src}: F1 {f:.3f}")
        checks[f"lemmekey on {src}, f1"] = f
    assert round(checks["lemmekey on spider, f1"], 3) == 0.885 and round(checks["lemmekey on bird, f1"], 3) == 0.829

    for src in SOURCES:
        other = next(s for s in SOURCES if s != src)
        schemas, rows = sets[src], data[src]
        ours = {LEMMEKEY: lk[src][1]}
        notes = {}
        for which in SETS:
            tag = f" [{which}]"
            ours["ind" + tag] = {r.schema["id"]: r.ind(which) for r in rows}
            ours["zhang" + tag] = {r.schema["id"]: set(r.zhang(which)) for r in rows}
            ours["hopf" + tag] = {r.schema["id"]: r.hopf(which, True) for r in rows}
            ours["hopf, no names" + tag] = {r.schema["id"]: r.hopf(which, False) for r in rows}
            for names in (True, False):
                n = ("rostin" if names else "rostin, no names") + tag
                ours[n], notes[n] = rostin(data[other], rows, which, names)
        hy = {r.schema["id"]: hybrids(r, lk[src][1][r.schema["id"]], lk[src][0].threshold) for r in rows}
        for rule in ("filtered", "promoted", "both"):
            ours[f"lemmekey, {rule}"] = {i: h[rule] for i, h in hy.items()}
        states = {}
        for h in hy.values():
            for k, v in h["drawn keys by what the data say"].items():
                states[k] = states.get(k, 0) + v
        notes["lemmekey's drawn keys by what the data say"] = states
        notes["keys promoted"] = sum(h["promoted keys"] for h in hy.values())

        # over all declared keys
        res = {"all declared keys": brief(kept.against(schemas, ours, {}, ("all",)), LEMMEKEY)}
        # over the declared keys whose column is not empty; a prediction for an empty column not counted
        sub_schemas, sub = [], {n: {} for n in ours}
        for s, r in zip(schemas, rows):
            fk = [k for k in s["fk"] if r.nonnull[(k[0], k[1])] > 0]
            sub_schemas.append({**s, "fk": fk})
            for n in ours:
                sub[n][s["id"]] = {p for p in ours[n][s["id"]] if r.nonnull.get((p[0], p[1]), 0) > 0}
        res["declared keys with a non-empty column"] = brief(kept.against(sub_schemas, sub, {}, ("all",)), LEMMEKEY)

        # the ceiling of the inclusions, and the keys the data keep
        truth = {s["id"]: {tuple(k) for k in s["fk"]} for s in schemas}
        ceiling = {}
        for which in SETS:
            hit = sum(len(truth[r.schema["id"]] & r.reachable(which)) for r in rows)
            total = sum(len(t) for t in truth.values())
            nonempty = sum(len(s["fk"]) for s in sub_schemas)
            ceiling[which] = {"declared keys with an inclusion into the declared table": hit,
                              "over all declared keys": hit / total, "over those with a non-empty column": hit / nonempty,
                              "inclusions": sum(len(r.inds[which]) for r in rows),
                              "columns with an inclusion": sum(len(r.by_column(which)) for r in rows)}
        res["ceiling"] = ceiling
        res["notes"] = notes
        main[src] = res

        # Zhang et al.'s top X %, a database at a time
        curve[src] = {}
        for which in SETS:
            pts = {}
            for x in range(10, 101, 10):
                preds = {}
                for r in rows:
                    ranked = r.zhang(which)
                    preds[r.schema["id"]] = set(ranked[:int(np.ceil(len(ranked) * x / 100))])
                e = kept.against(schemas, {"z": preds}, {}, ("all",))["z"]["all"]
                pts[x] = {k: e[k] for k in ("precision", "recall", "f1", "macro f1 over databases")}
            best = max(pts, key=lambda x: pts[x]["f1"])
            curve[src][which] = {"top X %": pts, "best X, chosen on the answers": best}

        # what the rows are like, and what reading them cost
        missing, selfs = 0, []
        for s, r in zip(schemas, rows):
            p = r.prof
            keys_ = [tuple(k) for k in s["fk"]]
            dec = [r.declared_columns.get((k[0].lower(), k[1].lower(), k[2].lower())) for k in keys_]
            missing += sum(d is None for d in dec)
            for d in dec:
                if d and d["table"] == d["target"] and d["column"] == d.get("referenced"):
                    selfs.append(f"{s['id']}: {d['table']}.{d['column']}")  # a column declared to reference itself: no inclusion by the README's definition
                elif d and d["state"] == "holds" and d.get("referenced in K"):
                    assert (d["table"], d["column"], d["target"]) in r.reachable("K"), "a kept key is not among K's inclusions"
            sec = p["seconds"]
            a = sec["statistics and inclusions into K"]
            b = sec["unique columns and inclusions into U"]
            c = sec["values for the distributions and features"]
            per_db.append({
                "source": src, "database": s["id"].split("/", 1)[1], "bytes": p["bytes"], "tables": len(r.tables),
                "empty tables": sum(t["rows"] == 0 for t in r.tables),
                "tables under ten rows": sum(t["rows"] < 10 for t in r.tables),
                "rows": p["rows"], "cells": p["cells"], "columns": sum(len(t["columns"]) for t in r.tables),
                "empty columns": sum(c_["nonnull"] == 0 for t in r.tables for c_ in t["columns"]),
                "declared keys": len(keys_),
                "with a non-empty column": sum(r.nonnull[(k[0], k[1])] > 0 for k in keys_),
                "kept by the data": sum(bool(d) and d["state"] == "holds" for d in dec),
                "broken by the data": sum(bool(d) and d["state"] == "fails" for d in dec),
                "referenced column in K": sum(bool(d) and bool(d.get("referenced in K")) for d in dec),
                "inclusions into K": len(r.inds["K"]), "inclusions into U": len(r.inds["U"]),
                "columns in K": sum(c_["K"] for t in r.tables for c_ in t["columns"]),
                "columns in U": sum(c_["U"] for t in r.tables for c_ in t["columns"]),
                "seconds, ind on K": a, "seconds, ind on U as well": a + b, "seconds, with the values": a + b + c,
                "seconds, the hybrid rules' own tests": sec["tests of lemmekey's best candidates"],
                "tests run": p["counts"]["tests run"], "tests pruned by range": p["counts"]["tests pruned by range"],
                "started": p["started"]})
        checks[f"{src}: declared keys of the kept set the file's pragma does not give"] = missing
        checks[f"{src}: declared keys from a column to itself, which no inclusion can give"] = selfs
        checks[f"{src}: every other declared key the data keep into a column of K is among K's inclusions"] = True

        mine = [d for d in per_db if d["source"] == src]

        def dist(k):
            v = np.array([d[k] for d in mine], dtype=np.float64)
            return {"median": float(np.median(v)), "90th percentile": float(np.percentile(v, 90)),
                    "largest": float(v.max()), "sum": float(v.sum())}

        cost[src] = {k: dist(k) for k in ("rows", "cells", "bytes", "seconds, ind on K", "seconds, ind on U as well",
                                         "seconds, with the values", "seconds, the hybrid rules' own tests")}
        cost[src]["databases"] = len(mine)
        tot = {k: sum(d[k] for d in mine) for k in ("tables", "empty tables", "tables under ten rows", "declared keys",
                                                    "with a non-empty column", "kept by the data", "broken by the data",
                                                    "referenced column in K", "columns", "empty columns",
                                                    "inclusions into K", "inclusions into U", "columns in K", "columns in U")}
        tot["databases with an empty table"] = sum(d["empty tables"] > 0 for d in mine)
        tot["databases with no row at all"] = sum(d["rows"] == 0 for d in mine)
        tot["databases with a declared key on an empty column"] = sum(d["declared keys"] > d["with a non-empty column"] for d in mine)
        tot["median rows of a database"] = float(np.median([d["rows"] for d in mine]))
        tot["median rows of a table"] = float(np.median([t["rows"] for r in rows for t in r.tables]))
        main[src]["the rows"] = tot

        print(f"== {src}")
        for block in ("all declared keys", "declared keys with a non-empty column"):
            print(f"-- {block}")
            for n, e in res[block].items():
                d = e.get(f"{LEMMEKEY} minus it")
                tail = f"  lemmekey minus it {d['f1']['point']:+.3f} [{d['f1']['95'][0]:+.3f}, {d['f1']['95'][1]:+.3f}]" if d else ""
                print(f"{n:24} {e['keys']:4d} keys P {e['precision']:.3f} R {e['recall']:.3f} F1 {e['f1']:.3f} "
                      f"[{e['f1_95'][0]:.3f}, {e['f1_95'][1]:.3f}] macro {e['macro f1 over databases']:.3f}{tail}")
        print(json.dumps(ceiling, indent=1))
        print(json.dumps(notes, indent=1))
        print(json.dumps(tot, indent=1))
        print(json.dumps(cost[src], indent=1))
        for which in SETS:
            print("zhang top X %", which, {x: round(v["f1"], 3) for x, v in curve[src][which]["top X %"].items()})
        sys.stdout.flush()

    head = {"model": MODEL, "threshold": lk["spider"][0].threshold, "frozen": frozen}
    (out / "main.json").write_text(json.dumps({**head, **main}, indent=1) + "\n", encoding="utf-8")
    (out / "curve.json").write_text(json.dumps(curve, indent=1) + "\n", encoding="utf-8")
    (out / "cost.json").write_text(json.dumps(cost, indent=1) + "\n", encoding="utf-8")
    (out / "checks.json").write_text(json.dumps(checks, indent=1) + "\n", encoding="utf-8")
    (out / "rows.json").write_text(json.dumps(per_db, indent=1) + "\n", encoding="utf-8")
    with open(out / "rows.csv", "w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=list(per_db[0]))
        w.writeheader()
        w.writerows(per_db)


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    run(Path(sys.argv[1]), Path(sys.argv[2]))
