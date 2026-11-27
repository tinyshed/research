"""Round 1x, after the scores: three readings the README did not set, written
once score.py's figures were seen, to say why they are what they are. None
of them changes a figure of score.py.

    python -I after.py broken <out folder of rows.py> <folder for the report's data>
    python -I after.py sizes  <out folder of rows.py> <folder for the report's data>

`broken` reads the databases again: for each declared key the data do not
keep, how many of its column's distinct values and rows have no match in
the referenced column (broken.json). `sizes` reads only the profiles:
lemmekey and the row methods on K by the rows of a database, and the
promoted rule with a floor under the candidate's score (sizes.json,
floor.json).
"""

import json
import sqlite3
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import numpy as np  # noqa: E402

import rows as rows_  # noqa: E402
import score  # noqa: E402
from score import kept  # noqa: E402

BINS = [(0, 100), (100, 10_000), (10_000, 10 ** 12)]
FLOORS = [0.0, 0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]


def load(root: Path):
    data, lk, sets = {}, {}, {}
    for src in score.SOURCES:
        sc, keys = score.lemmekey(src)
        sets[src] = [s for s in sc.schemas if s["fk"]]
        lk[src] = (sc, keys)
        data[src] = [score.Rows(s, json.load(open(root / "profile" / src / f"{s['id'].split('/', 1)[1]}.json",
                                                  encoding="utf-8"))) for s in sets[src]]
    return data, lk, sets


def broken(root: Path, out: Path) -> None:
    data, _, _ = load(root)
    members = rows_.bird_members()
    res = {}
    for src in score.SOURCES:
        entries = []
        for r in data[src]:
            bad = [d for d in r.prof["declared"] if d.get("state") == "fails"]
            if not bad:
                continue
            db = r.schema["id"].split("/", 1)[1]
            path, extracted, _ = rows_.locate(src, db, members, root / "tmp")
            try:
                con = sqlite3.connect(path.resolve().as_uri() + "?immutable=1", uri=True)
                con.execute("PRAGMA temp_store=MEMORY")
                q = rows_.q
                for d in bad:
                    t, a, g, b = q(d["table"]), q(d["column"]), q(d["target"]), q(d["referenced"])
                    n, nd = con.execute(f"SELECT COUNT(*), COUNT(DISTINCT {a}) FROM {t} WHERE {a} IS NOT NULL").fetchone()
                    m, md = con.execute(f"SELECT COUNT(*), COUNT(DISTINCT dep.{a}) FROM {t} AS dep WHERE dep.{a} IS NOT NULL "
                                        f"AND dep.{a} NOT IN (SELECT {b} FROM {g} WHERE {b} IS NOT NULL)").fetchone()
                    target_rows = con.execute(f"SELECT COUNT(*) FROM {g}").fetchone()[0]
                    entries.append({"database": r.schema["id"], "key": f"{d['table']}.{d['column']} -> {d['target']}.{d['referenced']}",
                                    "rows": n, "distinct values": nd, "rows with no match": m, "distinct values with no match": md,
                                    "rows of the referenced table": target_rows})
                con.close()
            finally:
                if extracted:
                    path.unlink(missing_ok=True)
            print(r.schema["id"], len(bad), flush=True)
        share = np.array([e["distinct values with no match"] / e["distinct values"] for e in entries])
        res[src] = {"declared keys the data do not keep": len(entries),
                    "databases with one": len({e["database"] for e in entries}),
                    "the referenced table is empty": int(sum(e["rows of the referenced table"] == 0 for e in entries)),
                    "under 1 % of the distinct values unmatched": int((share < 0.01).sum()),
                    "1 % to under 10 %": int(((share >= 0.01) & (share < 0.1)).sum()),
                    "10 % to under 50 %": int(((share >= 0.1) & (share < 0.5)).sum()),
                    "50 % or more": int((share >= 0.5).sum()),
                    "every value unmatched": int((share == 1.0).sum()),
                    "median share of distinct values unmatched": float(np.median(share)) if len(share) else None,
                    "keys": entries}
        print(src, {k: v for k, v in res[src].items() if k != "keys"}, flush=True)
    (out / "broken.json").write_text(json.dumps(res, indent=1) + "\n", encoding="utf-8")


def sizes(root: Path, out: Path) -> None:
    data, lk, sets = load(root)
    by_size, floor = {}, {}
    for src in score.SOURCES:
        other = next(s for s in score.SOURCES if s != src)
        rows, schemas = data[src], sets[src]
        ours = {score.LEMMEKEY: lk[src][1],
                "ind [K]": {r.schema["id"]: r.ind("K") for r in rows},
                "hopf [K]": {r.schema["id"]: r.hopf("K", True) for r in rows},
                "rostin [K]": score.rostin(data[other], rows, "K", True)[0]}
        by_size[src] = {}
        for lo, hi in BINS:
            sel = [s for s, r in zip(schemas, rows) if lo <= r.prof["rows"] < hi]
            if not sel:
                continue
            ids = {s["id"] for s in sel}
            res = kept.against(sel, {n: {i: p for i, p in v.items() if i in ids} for n, v in ours.items()}, {}, ("all",))
            by_size[src][f"{lo} to {hi - 1} rows" if hi < 10 ** 12 else f"{lo} rows or more"] = score.brief(res, score.LEMMEKEY)
        # the promoted rule with a floor under the best candidate's score; chosen on nothing, read on the answers
        thr = lk[src][0].threshold
        floor[src] = {}
        for fl in FLOORS:
            preds = {}
            for r in rows:
                drawn = lk[src][1][r.schema["id"]]
                have = {(t, c) for t, c, _ in drawn}
                t = r.tables
                add = {(t[ti]["name"], t[ti]["columns"][ci]["name"], t[tj]["name"])
                       for ti, ci, tj, kj, s, state in r.prof["lemmekey's best candidates"]
                       if state == "holds" and fl <= s < thr and (t[ti]["name"], t[ti]["columns"][ci]["name"]) not in have}
                preds[r.schema["id"]] = drawn | add
            res = kept.against(schemas, {score.LEMMEKEY: lk[src][1], "promoted": preds}, {}, ("all",))
            floor[src][str(fl)] = score.brief(res, score.LEMMEKEY)["promoted"]
        print(f"== {src}")
        for b, res in by_size[src].items():
            print(b, {n: (e["databases"], e["keys"], round(e["precision"], 3), round(e["recall"], 3), round(e["f1"], 3))
                      for n, e in res.items()})
        for fl, e in floor[src].items():
            d = e["lemmekey minus it"]["f1"]
            print(f"floor {fl}: P {e['precision']:.3f} R {e['recall']:.3f} F1 {e['f1']:.3f}  lemmekey minus it "
                  f"{d['point']:+.3f} [{d['95'][0]:+.3f}, {d['95'][1]:+.3f}]")
    (out / "sizes.json").write_text(json.dumps(by_size, indent=1) + "\n", encoding="utf-8")
    (out / "floor.json").write_text(json.dumps(floor, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    {"broken": broken, "sizes": sizes}[sys.argv[1]](Path(sys.argv[2]), Path(sys.argv[3]))
