"""Round 1y: the blind judge's sample on plain A or style 3, and what its
answers count (README.md, "The measures", 2 and 3).

    python -I judging.py pack <set>     packets and the hidden key, corpus/round-1y/judge-<set>/
    python -I judging.py count <set>    after the judges wrote their answers

The compared models are read from choice.json: the frozen model, the rule's
highest candidate and its twin, the best candidate of the other family, the
fit weighted by a schema's own label frequency, and the frozen model's
matched control for the highest candidate. The pool is every pair no
declaration names that a compared model draws (`drawn`) or that a language
model of round 1w named on the set and no compared model draws (`named`).
Strata: schemas under 40 tables and of 40 or more; for `named`, one language
model or several. Round 1v's packets otherwise: 60 declared keys and 60
random pairs as controls, every pair once under a number, shuffled, a schema
shown as the tables its pairs name with no declared key.
"""

import json
import random
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / "round-1v"))
sys.path.insert(0, str(HERE.parent / "round-1s"))
sys.path.insert(0, str(HERE.parent / "round-1p"))

import numpy as np  # noqa: E402

import whole  # noqa: E402

import pu  # noqa: E402

V, OUT = pu.V, pu.OUT
LLM = whole.ROOT / "round-1w" / "lm" / "llm"
NL = chr(10)
SEED = 20261110
PER_PACKET = 65
KEYED = ("int", "text", "uuid", "decimal", "longtext")
TAKE = {"plain-a": {"drawn": 250, "named": 60}, "style3": {"drawn": 120, "named": 30}}
RESAMPLES = 10000


def compared(which: str) -> list:
    c = json.load(open(OUT / "choice.json", encoding="utf-8"))
    best = c["the highest candidate"]
    first, rest = best.split("--", 1)
    twin = first + "-seed2--" + rest
    names = ["eleven--wide", best, twin, c["the best candidate of the other family"],
             c["read beside, a schema's own label frequency"], best + " | matched"]
    if which != "style3":
        # added before plain A was packed (README.md): the twin's own control, so that both seeds are held
        # against a threshold change; style 3 had been packed and judged without it
        names.append(twin + " | matched")
    return list(dict.fromkeys(names))


def language_models(which: str, schemas: list) -> dict:
    """{model: {database id: its keys}} from round 1w's saved replies; nothing is asked."""
    import lm  # noqa: E402  (round 1s's reading of a reply)
    out = {}
    for d in sorted(LLM.glob(f"*/{which}-db")):
        if any(d.glob("*.raw")):
            preds, info = lm.whole_schema(schemas, d)
            if not info["unanswered"]:
                out[d.parent.name] = preds
    return out


def pool_of(which: str) -> tuple:
    schemas = [json.loads(line) for line in open(V / f"heldout-github-{which}" / "hidden.jsonl", encoding="utf-8")]
    drawn = json.load(open(OUT / f"heldout-{which}-drawn.json", encoding="utf-8"))
    truth = [{tuple(k) for k in s["fk"]} for s in schemas]
    models = compared(which)
    by_pair = defaultdict(list)
    for m in models:
        for si, ks in enumerate(drawn[m]):
            for t, c, g in ks:
                if (t, c, g) not in truth[si]:
                    by_pair[(si, t, c, g)].append(m)
    named = defaultdict(list)
    where = {s["id"]: i for i, s in enumerate(schemas)}
    lms = language_models(which, schemas)
    for name, preds in lms.items():
        for sid, ks in preds.items():
            si = where[sid]
            for t, c, g in ks:
                if t != "?" and (t, c, g) not in truth[si] and (si, t, c, g) not in by_pair:
                    named[(si, t, c, g)].append(name)
    return schemas, truth, models, by_pair, named, sorted(lms)


def stratum(schemas, p, kind: str, names: list) -> str:
    size = "40 tables or more" if len(schemas[p[0]]["tables"]) >= 40 else "under 40 tables"
    if kind == "drawn":
        return f"drawn, {size}"
    return f"named by {'one language model' if len(names) == 1 else 'several language models'}, {size}"


def pack(which: str) -> None:
    d = OUT / f"judge-{which}"
    rng = random.Random(SEED)
    schemas, truth, models, by_pair, named, lms = pool_of(which)
    strata = defaultdict(list)
    for p in sorted(by_pair):
        strata[stratum(schemas, p, "drawn", [])].append(p)
    for p in sorted(named):
        strata[stratum(schemas, p, "named", named[p])].append(p)
    pairs = {}
    for name in sorted(strata):
        take = TAKE[which]["drawn" if name.startswith("drawn") else "named"]
        for p in rng.sample(strata[name], min(take, len(strata[name]))):
            pairs[p] = {"kind": "undeclared", "stratum": name, "models": by_pair.get(p, []),
                        "language models": named.get(p, [])}
    declared = sorted((si, a, c, g) for si, s in enumerate(schemas) for a, c, g in s["fk"])
    for p in rng.sample(declared, min(60, len(declared))):
        pairs.setdefault(p, {"kind": "control: declared", "stratum": "", "models": [], "language models": []})
    taken = {p[:3] for p in declared} | {p[:3] for p in by_pair} | {p[:3] for p in named}
    free = [(si, t["name"], c["name"]) for si, s in enumerate(schemas) for t in s["tables"] for c in t["columns"]
            if c["family"] in KEYED and not c["primary"] and (si, t["name"], c["name"]) not in taken]
    for si, t, c in rng.sample(free, min(60, len(free))):
        g = schemas[si]["tables"][rng.randrange(len(schemas[si]["tables"]))]["name"]
        pairs.setdefault((si, t, c, g), {"kind": "control: not a key", "stratum": "", "models": [], "language models": []})
    by_db = defaultdict(list)
    for p in pairs:
        by_db[p[0]].append(p)
    dbs = sorted(by_db)
    rng.shuffle(dbs)
    packets, cur = [], []
    for db in dbs:
        cur.append(db)
        if sum(len(by_db[x]) for x in cur) >= PER_PACKET:
            packets.append(cur)
            cur = []
    if cur:
        packets.append(cur)
    (d / "packets").mkdir(parents=True, exist_ok=True)
    (d / "answers").mkdir(parents=True, exist_ok=True)
    key = {}
    for k, group in enumerate(packets, 1):
        items = [p for x in group for p in by_db[x]]
        rng.shuffle(items)
        lines = [f"# Packet {k:02d}", ""]
        for j, x in enumerate(group, 1):
            s = schemas[x]
            shown = {p[1] for p in items if p[0] == x} | {p[3] for p in items if p[0] == x}
            lines.append(f"## Database {k:02d}-{j}")
            lines.append("")
            for t in s["tables"]:
                if t["name"] in shown:
                    cols = ", ".join(f"{c['name']} {c['type'] or c['family']}" for c in t["columns"])
                    pk = ", ".join(t["pk"]) or "none"
                    lines.append(f"- table `{t['name']}` (primary key: {pk}): {cols}")
            lines.append("")
        lines.append("## Pairs")
        lines.append("")
        for i, p in enumerate(items, 1):
            pid = f"{k:02d}-{i:03d}"
            j = group.index(p[0]) + 1
            lines.append(f"{pid}. database {k:02d}-{j}: does `{p[1]}.{p[2]}` hold references to the rows of `{p[3]}`?")
            key[pid] = {"pair": [schemas[p[0]]["id"], *p[1:]], "tables": len(schemas[p[0]]["tables"]), **pairs[p]}
        (d / "packets" / f"packet-{k:02d}.md").write_text(NL.join(lines) + NL, encoding="utf-8")
    (d / "key.json").write_text(json.dumps(key, indent=1) + NL, encoding="utf-8")
    totals = {"compared models": models, "language models": lms,
              "strata": {n: len(v) for n, v in sorted(strata.items())},
              "undeclared keys drawn, every one": {m: sum(1 for v in by_pair.values() if m in v) for m in models}}
    (d / "totals.json").write_text(json.dumps(totals, indent=1) + NL, encoding="utf-8")
    print(json.dumps({"folder": d.name, "packets": len(packets), "pairs": len(key),
                      "kinds": Counter(v["kind"] for v in key.values()),
                      "sampled by stratum": Counter(v["stratum"] for v in key.values() if v["stratum"]), **totals}, indent=1))


def count(which: str) -> None:
    d = OUT / f"judge-{which}"
    key = json.load(open(d / "key.json", encoding="utf-8"))
    totals = json.load(open(d / "totals.json", encoding="utf-8"))
    read = json.load(open(OUT / f"heldout-{which}.json", encoding="utf-8"))
    models = totals["compared models"]
    tables = np.array(read["tables"])
    big = tables >= 40
    answers = {}
    for f in sorted((d / "answers").glob("packet-*.json")):
        for pid, a in json.load(open(f, encoding="utf-8")).items():
            answers[pid] = str(a).strip().lower()
    by_kind = defaultdict(Counter)
    for pid, v in key.items():
        by_kind[v["stratum"] or v["kind"]][answers.get(pid, "missing")] += 1
    res = {"answered": len(answers), "of": len(key), "answers by kind": {k: dict(c) for k, c in sorted(by_kind.items())},
           "strata": totals["strata"], "compared models": models, "language models": totals["language models"]}
    # a stratum's judged pairs: yes or not, and which compared models draw each
    strata = {}
    for name, n_all in totals["strata"].items():
        rows = [(answers[pid] == "yes", [m in v["models"] for m in models])
                for pid, v in key.items() if v["stratum"] == name and answers.get(pid) in ("yes", "no", "unsure")]
        yes = np.array([r[0] for r in rows], dtype=float)
        member = np.array([r[1] for r in rows], dtype=float).reshape(len(rows), len(models))
        strata[name] = (n_all, yes, member)
    per = {m: np.array(read["models"][m]["per database"]) for m in models}
    rng = np.random.default_rng(whole.SEED)
    draws = {name: rng.multinomial(len(yes), np.full(len(yes), 1 / len(yes)), size=RESAMPLES) if len(yes) else None
             for name, (n_all, yes, member) in strata.items()}

    def estimate(scope: str) -> dict:
        """Point estimates and resamples: scope `all`, or `40 tables or more`."""
        use = [n for n in strata if scope == "all" or n.endswith("40 tables or more")]
        rows = big if scope != "all" else np.ones(len(tables), dtype=bool)
        declared = float(sum(per[models[0]][rows][:, [0, 2]].sum(axis=1)))
        real = np.full(RESAMPLES + 1, declared)                       # index 0 is the point estimate
        found = np.zeros((RESAMPLES + 1, len(models)))
        for n in use:
            n_all, yes, member = strata[n]
            if not len(yes):
                continue
            scale = n_all / len(yes)
            c = np.vstack([np.ones(len(yes)), draws[n]])
            real += scale * (c @ yes)
            found += scale * (c @ (member * yes[:, None]))
        out = {"declared keys": declared, "real keys, declared and judged-real of the pool": float(real[0]),
               "its 95 % interval": whole.interval(real[1:]), "models": {}}
        est = {}
        for j, m in enumerate(models):
            tp = float(per[m][rows][:, 0].sum())
            drawn = float(per[m][rows][:, :2].sum())
            p = (tp + found[:, j]) / max(drawn, 1)
            r = (tp + found[:, j]) / real
            est[m] = (p, r)
            sampled = sum(int(strata[n][2][:, j].sum()) for n in use if n.startswith("drawn"))
            said_yes = sum(int((strata[n][2][:, j] * strata[n][1]).sum()) for n in use if n.startswith("drawn"))
            out["models"][m] = {
                "drawn": drawn, "declared keys found": tp, "keys no declaration names": drawn - tp,
                "sampled": sampled, "called keys": said_yes, "share called keys": said_yes / max(sampled, 1),
                "judged real among them, estimated": float(found[0, j]),
                "precision, declarations": tp / max(drawn, 1), "recall, declarations": tp / max(declared, 1),
                "precision, with the judge": float(p[0]), "precision, with the judge, 95": whole.interval(p[1:]),
                "recall, declared and judged-real": float(r[0]), "recall, declared and judged-real, 95": whole.interval(r[1:]),
                "f1, with the judge": float(2 * p[0] * r[0] / (p[0] + r[0]))}
        for j, m in enumerate(models):
            for base in ("eleven--wide", m + " | matched", models[1] + " | matched"):
                if base in est and base != m and f"minus {base}" not in out["models"][m]:
                    dp, dr = est[m][0] - est[base][0], est[m][1] - est[base][1]
                    out["models"][m][f"minus {base}"] = {
                        "precision, with the judge": float(dp[0]), "precision, with the judge, 95": whole.interval(dp[1:]),
                        "recall, declared and judged-real": float(dr[0]),
                        "recall, declared and judged-real, 95": whole.interval(dr[1:])}
        return out

    res["all"] = estimate("all")
    res["40 tables or more"] = estimate("40 tables or more")
    (d / "count.json").write_text(json.dumps(res, indent=1) + NL, encoding="utf-8")
    print(json.dumps({k: v for k, v in res.items() if k not in ("all", "40 tables or more")}, indent=1))
    for scope in ("all", "40 tables or more"):
        e = res[scope]
        print(f"== {which}, {scope}: declared {e['declared keys']:.0f}, real {e['real keys, declared and judged-real of the pool']:.0f} "
              f"{[round(x) for x in e['its 95 % interval']]}")
        for m, r in e["models"].items():
            print(f"  {m:44} drawn {r['drawn']:6.0f} undeclared {r['keys no declaration names']:5.0f} sampled {r['sampled']:3d} "
                  f"yes {r['share called keys']:.3f} | P declared {r['precision, declarations']:.3f} judge "
                  f"{r['precision, with the judge']:.3f} {[round(x, 3) for x in r['precision, with the judge, 95']]} | "
                  f"R declared {r['recall, declarations']:.3f} real {r['recall, declared and judged-real']:.3f} "
                  f"{[round(x, 3) for x in r['recall, declared and judged-real, 95']]}")
            for k, v in r.items():
                if k.startswith("minus "):
                    print(f"      {k}: P {v['precision, with the judge']:+.3f} {[round(x, 3) for x in v['precision, with the judge, 95']]}"
                          f"  R {v['recall, declared and judged-real']:+.3f} {[round(x, 3) for x in v['recall, declared and judged-real, 95']]}")


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    {"pack": pack, "count": count}[sys.argv[1]](sys.argv[2])
