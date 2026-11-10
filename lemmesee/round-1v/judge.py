"""Round 1v: a blind judge of what the teachers add to the declarations.

    python -I judge.py pack     packets and the hidden key, corpus/round-1v/judge-taught/
    python -I judge.py count    after the judges wrote their answers
    python -I judge.py pack-drawn <set> <first>--<second> [...]   the keys models draw on a development set
    python -I judge.py count-drawn <set>
    python -I judge.py pack-clean <clean set> [<pairs>]   a sample of what the frozen models drew on a clean set
    python -I judge.py count-clean <clean set>

The two teachers (teach.py) name, on SchemaPile's training and validation
schemas, columns that no declaration names. Before a model learns from them,
a judge of another family, which taught nothing here, says how many are
keys: round 1t's protocol and prompt unchanged (round-1t/judge-prompt.md).
Sampled with seed 20261110 from the schemas of 20 tables or more of the
training split and of 10 or more of the validation split:

- 220 columns both teachers point at the same table (`taught by both`);
- 80 columns one teacher names, or both with different tables, with the
  table a naming teacher gave (`named by one`);
- as controls, 70 declared keys and 70 pairs of a column neither teacher
  names and no declaration names with a table of the schema drawn at random.

Each pair is shown once under a number, shuffled with the others; a packet
holds a few schemas, each as the tables its pairs name, with columns, types
and primary keys and no declared key. The key from numbers to pairs and
kinds is kept apart and read only by `count`.
"""

import json
import random
import sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import teach  # noqa: E402

D = teach.ROOT / "round-1v" / "judge-taught"
NL = chr(10)
SEED = 20261110
SETS = ("train-declared-20", "validation-declared-10")
TAKE = {"taught by both": 220, "named by one": 80, "control: declared": 70, "control: not a key": 70}
PER_PACKET = 65
KEYED = ("int", "text", "uuid", "decimal", "longtext")


def pack() -> None:
    rng = random.Random(SEED)
    pools = defaultdict(list)
    schemas = {}
    for which in SETS:
        answers = []
        for m in teach.TEACHERS:
            got = {}
            for line in open(teach.OUT / f"{m}--{which}.jsonl", encoding="utf-8"):
                r = json.loads(line)
                got[r["id"]] = {(a, c): g for a, c, g in r["keys"]}
            answers.append(got)
        for s in teach.chosen(which):
            if any(s["id"] not in a for a in answers):
                continue
            schemas[s["id"]] = s
            a, b = (x[s["id"]] for x in answers)
            declared = {(t, c): g for t, c, g in s["fk"]}
            for k in sorted(set(a) | set(b)):
                if k in declared:
                    continue
                if a.get(k) is not None and a.get(k) == b.get(k):
                    pools["taught by both"].append((s["id"], k[0], k[1], a[k]))
                else:
                    pools["named by one"].append((s["id"], k[0], k[1], a.get(k) or b.get(k)))
            for (t, c), g in declared.items():
                pools["control: declared"].append((s["id"], t, c, g))
            named = set(a) | set(b) | set(declared)
            free = [(t["name"], c["name"]) for t in s["tables"] for c in t["columns"]
                    if (t["name"], c["name"]) not in named and c["family"] in KEYED and not c["primary"]]
            if free:
                t, c = free[rng.randrange(len(free))]
                g = s["tables"][rng.randrange(len(s["tables"]))]["name"]
                pools["control: not a key"].append((s["id"], t, c, g))
    pairs = {}
    for kind, n in TAKE.items():
        pool = sorted(set(pools[kind]))
        for p in rng.sample(pool, min(n, len(pool))):
            pairs.setdefault(p, kind)
    by_db = defaultdict(list)
    for p in pairs:
        by_db[p[0]].append(p)
    dbs = sorted(by_db)
    rng.shuffle(dbs)
    packets, cur = [], []
    for db in dbs:
        cur.append(db)
        if sum(len(by_db[d]) for d in cur) >= PER_PACKET:
            packets.append(cur)
            cur = []
    if cur:
        packets.append(cur)
    (D / "packets").mkdir(parents=True, exist_ok=True)
    (D / "answers").mkdir(parents=True, exist_ok=True)
    key = {}
    for k, group in enumerate(packets, 1):
        items = [p for d in group for p in by_db[d]]
        rng.shuffle(items)
        lines = [f"# Packet {k:02d}", ""]
        for j, d in enumerate(group, 1):
            s = schemas[d]
            named = {p[1] for p in items if p[0] == d} | {p[3] for p in items if p[0] == d}
            lines.append(f"## Database {k:02d}-{j}")
            lines.append("")
            for t in s["tables"]:
                if t["name"] in named:
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
            key[pid] = {"pair": list(p), "kind": pairs[p]}
        (D / "packets" / f"packet-{k:02d}.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (D / "key.json").write_text(json.dumps(key, indent=1) + "\n", encoding="utf-8")
    print(json.dumps({"packets": len(packets), "pairs": len(key),
                      "kinds": Counter(v["kind"] for v in key.values()),
                      "pools": {k: len(set(v)) for k, v in pools.items()}}, indent=1))


def count() -> None:
    key = json.load(open(D / "key.json", encoding="utf-8"))
    answers = {}
    for f in sorted((D / "answers").glob("packet-*.json")):
        for pid, a in json.load(open(f, encoding="utf-8")).items():
            answers[pid] = str(a).strip().lower()
    by_kind = defaultdict(Counter)
    for pid, v in key.items():
        by_kind[v["kind"]][answers.get(pid, "missing")] += 1
    out = {"answered": len(answers), "of": len(key), "answers by kind": {k: dict(c) for k, c in by_kind.items()}}
    for k, c in by_kind.items():
        said = c["yes"] + c["no"] + c["unsure"]
        out[f"{k}: yes of answered"] = round(c["yes"] / max(said, 1), 4)
    (D / "count.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(out, indent=1))


def pack_drawn(name: str, models: list) -> None:
    """The keys the models draw on a development set that no declaration
    names, judged as round 1t judged its models': every such pair once,
    whichever models drew it, among 60 declared keys and 60 pairs drawn at
    random of columns nobody draws or declares. Into
    corpus/round-1v/judge-<set>/."""
    import misses  # noqa: E402
    import whole  # noqa: E402
    d = teach.ROOT / "round-1v" / ("judge-" + whole.slug(name))
    rng = random.Random(SEED)
    pairs, schemas = {}, None
    drawn_columns = set()
    for m in models:
        first, second = m.split("--")
        schemas, rec, raw, threshold = misses.load(first, second, name)
        for (si, t, c, g), v in misses.fates(schemas, rec, raw, threshold).items():
            if v[0] in ("found", "drawn, no key declared", "another target first, drawn"):
                drawn_columns.add((si, t, c))
            if v[0] == "drawn, no key declared":
                pairs.setdefault((si, t, c, g), {"models": [], "kind": "undeclared, drawn"})["models"].append(m)
    declared = sorted((si, a, c, g) for si, s in enumerate(schemas) for a, c, g in s["fk"])
    for p in rng.sample(declared, min(60, len(declared))):
        pairs.setdefault(p, {"models": [], "kind": "control: declared"})
    declared_columns = {p[:3] for p in declared}
    free = [(si, t["name"], c["name"]) for si, s in enumerate(schemas) for t in s["tables"] for c in t["columns"]
            if c["family"] in KEYED and not c["primary"] and (si, t["name"], c["name"]) not in declared_columns
            and (si, t["name"], c["name"]) not in drawn_columns]
    for si, t, c in rng.sample(free, min(60, len(free))):
        g = schemas[si]["tables"][rng.randrange(len(schemas[si]["tables"]))]["name"]
        pairs.setdefault((si, t, c, g), {"models": [], "kind": "control: not a key"})
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
            named = {p[1] for p in items if p[0] == x} | {p[3] for p in items if p[0] == x}
            lines.append(f"## Database {k:02d}-{j}")
            lines.append("")
            for t in s["tables"]:
                if t["name"] in named:
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
            key[pid] = {"pair": [schemas[p[0]]["id"], *p[1:]], **pairs[p]}
        (d / "packets" / f"packet-{k:02d}.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (d / "key.json").write_text(json.dumps(key, indent=1) + "\n", encoding="utf-8")
    print(json.dumps({"folder": d.name, "packets": len(packets), "pairs": len(key),
                      "kinds": Counter(v["kind"] for v in key.values()),
                      "by model": Counter(m for v in key.values() for m in v["models"])}, indent=1))


def count_drawn(name: str) -> None:
    import whole  # noqa: E402
    d = teach.ROOT / "round-1v" / ("judge-" + whole.slug(name))
    key = json.load(open(d / "key.json", encoding="utf-8"))
    answers = {}
    for f in sorted((d / "answers").glob("packet-*.json")):
        for pid, a in json.load(open(f, encoding="utf-8")).items():
            answers[pid] = str(a).strip().lower()
    by_kind = defaultdict(Counter)
    by_model = defaultdict(Counter)
    only = defaultdict(Counter)
    for pid, v in key.items():
        a = answers.get(pid, "missing")
        by_kind[v["kind"]][a] += 1
        for m in v["models"]:
            by_model[m][a] += 1
        if len(v["models"]) == 1:
            only[v["models"][0] + " only"][a] += 1
    out = {"answered": len(answers), "of": len(key), "answers by kind": {k: dict(c) for k, c in by_kind.items()},
           "undeclared keys drawn, by model": {k: dict(c) for k, c in by_model.items()},
           "undeclared keys only one model draws": {k: dict(c) for k, c in only.items()}}
    (d / "count.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(json.dumps(out, indent=1))


def pack_clean(which: str, take: int = 400) -> None:
    """After a clean set's one reading: a sample of the keys the frozen
    models drew there that no declaration names, `take` of them drawn with
    the round's seed from every such pair of any model, among 60 declared
    keys and 60 pairs drawn at random of columns nobody draws or declares.
    Into corpus/round-1v/judge-<set>/."""
    out = teach.ROOT / "round-1v"
    d = out / ("judge-" + which)
    rng = random.Random(SEED)
    schemas = [json.loads(line) for line in open(out / f"heldout-github-{which}" / "hidden.jsonl", encoding="utf-8")]
    drawn = json.load(open(out / f"heldout-{which}-drawn.json", encoding="utf-8"))
    truth = [{tuple(k) for k in s["fk"]} for s in schemas]
    by_pair = defaultdict(list)
    drawn_columns = set()
    for m, per in drawn.items():
        for si, keys in enumerate(per):
            for t, c, g in keys:
                drawn_columns.add((si, t, c))
                if (t, c, g) not in truth[si]:
                    by_pair[(si, t, c, g)].append(m)
    pool = sorted(by_pair)
    pairs = {p: {"models": by_pair[p], "kind": "undeclared, drawn"} for p in rng.sample(pool, min(take, len(pool)))}
    declared = sorted((si, a, c, g) for si, s in enumerate(schemas) for a, c, g in s["fk"])
    for p in rng.sample(declared, min(60, len(declared))):
        pairs.setdefault(p, {"models": [], "kind": "control: declared"})
    declared_columns = {p[:3] for p in declared}
    free = [(si, t["name"], c["name"]) for si, s in enumerate(schemas) for t in s["tables"] for c in t["columns"]
            if c["family"] in KEYED and not c["primary"] and (si, t["name"], c["name"]) not in declared_columns
            and (si, t["name"], c["name"]) not in drawn_columns]
    for si, t, c in rng.sample(free, min(60, len(free))):
        g = schemas[si]["tables"][rng.randrange(len(schemas[si]["tables"]))]["name"]
        pairs.setdefault((si, t, c, g), {"models": [], "kind": "control: not a key"})
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
            named = {p[1] for p in items if p[0] == x} | {p[3] for p in items if p[0] == x}
            lines.append(f"## Database {k:02d}-{j}")
            lines.append("")
            for t in s["tables"]:
                if t["name"] in named:
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
            key[pid] = {"pair": [schemas[p[0]]["id"], *p[1:]], **pairs[p]}
        (d / "packets" / f"packet-{k:02d}.md").write_text(NL.join(lines) + NL, encoding="utf-8")
    (d / "key.json").write_text(json.dumps(key, indent=1) + NL, encoding="utf-8")
    totals = {m: sum(1 for p in pool if m in by_pair[p]) for m in drawn}
    (d / "totals.json").write_text(json.dumps({"undeclared keys drawn, every one": totals,
                                               "pairs any model draws undeclared": len(pool)}, indent=1) + NL,
                                   encoding="utf-8")
    print(json.dumps({"folder": d.name, "packets": len(packets), "pairs": len(key),
                      "kinds": Counter(v["kind"] for v in key.values()),
                      "sampled, by model": Counter(m for v in key.values() for m in v["models"]),
                      "undeclared keys drawn, every one": totals}, indent=1))


def count_clean(which: str) -> None:
    """The judge's answers on a clean set's sample, and each model's
    precision counted with them: its declared keys found, and of its keys no
    declaration names the share the judge calls keys in the sample."""
    out = teach.ROOT / "round-1v"
    d = out / ("judge-" + which)
    key = json.load(open(d / "key.json", encoding="utf-8"))
    totals = json.load(open(d / "totals.json", encoding="utf-8"))["undeclared keys drawn, every one"]
    read = json.load(open(out / f"heldout-{which}.json", encoding="utf-8"))["models"]
    answers = {}
    for f in sorted((d / "answers").glob("packet-*.json")):
        for pid, a in json.load(open(f, encoding="utf-8")).items():
            answers[pid] = str(a).strip().lower()
    by_kind = defaultdict(Counter)
    by_model = defaultdict(Counter)
    for pid, v in key.items():
        a = answers.get(pid, "missing")
        by_kind[v["kind"]][a] += 1
        for m in v["models"]:
            by_model[m][a] += 1
    res = {"answered": len(answers), "of": len(key), "answers by kind": {k: dict(c) for k, c in by_kind.items()},
           "models": {}}
    for m, c in by_model.items():
        said = c["yes"] + c["no"] + c["unsure"]
        share = c["yes"] / max(said, 1)
        tp, fp, fn = (sum(x[i] for x in read[m]["per database"]) for i in range(3))
        res["models"][m] = {"sampled": said, "yes": c["yes"], "no": c["no"], "unsure": c["unsure"],
                            "share called keys": round(share, 4), "drawn": tp + fp, "declared keys found": tp,
                            "keys no declaration names": totals[m],
                            "precision, declarations": round(tp / max(tp + fp, 1), 4),
                            "precision, with the judge's share": round((tp + share * totals[m]) / max(tp + fp, 1), 4)}
    (d / "count.json").write_text(json.dumps(res, indent=1) + NL, encoding="utf-8")
    print(json.dumps(res, indent=1))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    if sys.argv[1] == "pack-clean":
        pack_clean(sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 400)
    elif sys.argv[1] == "count-clean":
        count_clean(sys.argv[2])
    elif sys.argv[1] == "pack-drawn":
        pack_drawn(sys.argv[2], sys.argv[3:])
    elif sys.argv[1] == "count-drawn":
        count_drawn(sys.argv[2])
    else:
        {"pack": pack, "count": count}[sys.argv[1]]()
