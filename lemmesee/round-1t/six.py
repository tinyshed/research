"""Round 1t's version 6 candidates: more of the declared keys reachable,
and cues for the tables that relate others.

Looked into on the development sets (SchemaPile's test, Spider, BIRD, CTU,
LiveSQLBench) on 8 November 2026, a declared key was out of the candidates
when its column's family was `longtext` (PostgreSQL's `text` as round 1b
read it: 3.3 % of SchemaPile's test keys) or `decimal` (CTU's tpce), when
its target's key had several columns (lahman's `teams`), when it pointed at
a unique column rather than the key, or when the two columns' families
differed (`text` into `int`, 3.4 % of Spider's). And keys round 1s's model
drew none of: tables of a logic program (`aunt(name1, name2)` into
`person(name)`), a column named for its target's key with a prefix
(`AP_CA_ID` into `CA_ID`), a word in another number (`senha_usuario` into
`tb_senhas_auth`).

Version 6, each change general:

- **families**: `longtext` reads as text, `decimal` as an integer;
- **views of a target**: a table is a target through its one-column key, or
  its pseudo key (round 1p), or each column of a key of several columns, or
  each other unique column; a column meets each table once, through the
  view its names fit best (ties to the key);
- **across families** where the names say so: a column meets a target of
  another family when its name is the target key's or its stem the target's
  name;
- **cues**: round 1q's role cues (the key's name inside the column, less its
  digits), version 5's, and version 6's: which view; families differ; the
  column's table declares no key; the share of its table's columns named,
  less their digits, as some target's key; words shared with the target and
  with its key once both are singular; the column a decimal.
"""

from collections import Counter

import keys
import widen
from train import norm, pair_features, singular, tokens

KIND6 = {"int": "int", "decimal": "int", "uuid": "uuid", "text": "text", "longtext": "text"}
VIEW_ORDER = {"pk": 0, "pseudo": 1, "part": 2, "unique": 3}


def column(t, name):
    return next((c for c in t["columns"] if c["name"] == name), None)


def views(t):
    """(key column, view) of a table, in VIEW_ORDER."""
    out = []
    if len(t["pk"]) == 1:
        c = column(t, t["pk"][0])
        if c and KIND6.get(c["family"]):
            out.append((c, "pk"))
    elif not t["pk"]:
        unique = [c for c in t["columns"] if c["unique"] and KIND6.get(c["family"])]
        if len(unique) == 1:
            out.append((unique[0], "pseudo"))
        else:
            g = norm(t["name"])
            names = {"id", g + "id", singular(g) + "id"}
            named = [c for c in t["columns"] if norm(c["name"]) in names and KIND6.get(c["family"])]
            if named:
                out.append((named[0], "pseudo"))
    else:
        for p in t["pk"]:
            c = column(t, p)
            if c and KIND6.get(c["family"]):
                out.append((c, "part"))
    taken = {id(c) for c, _ in out}
    for c in t["columns"]:
        if c["unique"] and id(c) not in taken and KIND6.get(c["family"]):
            out.append((c, "unique"))
    return out


def words(name):
    return {singular(w) for w in tokens(name) if w not in keys.KEY_WORDS and len(w) >= 2}


def pairs(schemas, split, guesses, seven=False, eight=False, nine=False):
    import cues  # noqa: E402  (version 5, after this module is imported by it)
    import roles  # noqa: E402  (version 9)
    import words7  # noqa: E402
    for s in schemas:
        if s["split"] != split or not s["fk"]:
            continue
        declared = {(a, c): b for a, c, b in s["fk"]}
        targets = []
        for t in s["tables"]:
            g = norm(t["name"])
            for key, how in views(t):
                targets.append({
                    "table": t, "view": {**t, "pk": [key["name"]]}, "kind": KIND6[key["family"]], "how": how,
                    "name": g, "singular": singular(g), "grams": keys.grams(g) | keys.grams(singular(g)),
                    "pk": norm(key["name"]), "pk_type": key.get("type"), "key": key["name"],
                    "words": words(t["name"]), "key words": words(key["name"]),
                })
        stems = [keys.stem_of(c["name"])[0] for t in s["tables"] for c in t["columns"]]
        named_by = {id(g): sum(1 for st in stems if st and st in (g["name"], g["singular"])) for g in targets}
        all_tokens = [keys.tokens(c["name"]) for t in s["tables"] for c in t["columns"]]
        id_convention = sum(1 for tk in all_tokens if tk and tk[-1] == "id") / max(len(all_tokens), 1)
        key_count = Counter(g["pk"] for g in targets)
        bare = [norm(c["name"]).rstrip("0123456789") for t in s["tables"] for c in t["columns"]]
        carrying = {k: sum(1 for b in bare if k in b) for k in key_count if k != "id" and len(k) >= 3}
        key_names = set(key_count)
        keyed_share = {}
        for t in s["tables"]:
            names = [norm(c["name"]).rstrip("0123456789") for c in t["columns"]]
            keyed_share[t["name"]] = sum(1 for n in names if n in key_names) / max(1, len(names))
        info = cues.schema_info(s)

        for table in s["tables"]:
            no_key = float(not table["pk"])
            for col in table["columns"]:
                kind = KIND6.get(col["family"])
                if not kind:
                    continue
                own = col["primary"] and len(table["pk"]) == 1
                stem, tk = keys.stem_of(col["name"])
                sg = keys.grams(stem) if stem else set()
                cname = norm(col["name"])
                cwords = words(col["name"])
                # each table once, through the view the names fit best
                best_view = {}
                for g in targets:
                    if g["table"] is table and col["name"] == g["key"]:
                        continue
                    cross = g["kind"] != kind
                    if cross and not (cname == g["pk"] or (stem and stem in (g["name"], g["singular"]))):
                        continue
                    exact = stem in (g["name"], g["singular"]) or stem + "s" == g["name"]
                    p = keys.prefix(stem, g["singular"])
                    b = max(float(exact), keys.jaccard(sg, g["grams"]), float(cname == g["pk"]),
                            0.8 * p / max(1, min(len(stem), len(g["singular"]))) if p >= 3 else 0.0)
                    k = id(g["table"])
                    if k not in best_view or b > best_view[k][1] or (
                            b == best_view[k][1] and VIEW_ORDER[g["how"]] < VIEW_ORDER[best_view[k][0]["how"]]):
                        best_view[k] = (g, b, cross)
                mine = list(best_view.values())
                if not mine:
                    continue
                base = [b for _, b, _ in mine]
                best = max(base)
                guess = guesses.get((s["id"], table["name"], col["name"]), {}).get("fk")
                name = cname
                less = name.rstrip("0123456789")
                for j, (g, b, cross) in enumerate(mine):
                    target = g["table"]
                    label = declared.get((table["name"], col["name"])) == target["name"]
                    f = pair_features(s, table, col, g["view"], with_index=True)
                    f["self"] = float(target is table)
                    others = [x for i, x in enumerate(base) if i != j]
                    f["v2 letters"] = keys.jaccard(sg, g["grams"])
                    f["v2 edit"] = keys.similarity(stem, g["singular"])
                    f["v2 prefix"] = min(keys.prefix(stem, g["singular"]), 8) / 8
                    f["v2 base"] = b
                    f["v2 best of the column"] = float(b >= best)
                    f["v2 margin"] = b - max(others, default=0.0)
                    f["v2 close rivals"] = min(sum(1 for x in others if x >= 0.5), 5) / 5
                    f["v2 named as the key"] = float(cname == g["pk"])
                    f["v2 same type"] = float(col.get("type") == g["pk_type"])
                    f["v2 named by"] = min(named_by[id(g)], 10) / 10
                    f["v2 id convention"] = id_convention
                    f["v2 bare"] = float(not stem)
                    f["v2 key word:" + (tk[-1] if tk else "")] = 1.0
                    for gram in list(sg)[:12]:
                        f["cg:" + gram] = 1.0
                    f[f"p3:{stem[:3]}|{g['singular'][:3]}"] = 1.0
                    f["v3 own key"] = float(own)
                    f["v3 pseudo target"] = float(g["how"] == "pseudo")
                    k = g["pk"]
                    f["v4 less digits is the key"] = float(less == k and less != name)
                    f["v4 key inside"] = float(k != "id" and len(k) >= 3 and k in less)
                    f["v4 ends with the key"] = float(len(k) >= 3 and less.endswith(k) and less != k)
                    f["v4 targets keyed so"] = 1 / key_count[k]
                    f["v4 columns carrying the key"] = min(carrying.get(k, 0), 20) / 20
                    cues.add_cues(f, s, table, col, target, g["key"], info)
                    f["v6 view:" + g["how"]] = 1.0
                    f["v6 families differ"] = float(cross)
                    f["v6 source declares no key"] = no_key
                    f["v6 source columns keyed"] = keyed_share[table["name"]]
                    f["v6 words shared"] = min(len(cwords & g["words"]), 3) / 3
                    f["v6 words shared with the key"] = min(len(cwords & g["key words"]), 3) / 3
                    f["v6 decimal"] = float(col["family"] == "decimal")
                    if seven:
                        words7.add_cues(f, col, target, g["key"])
                    if eight:
                        words7.add_related(f, col, target)
                    if nine:
                        roles.add_cues(f, col, target, table)
                    yield s, table, col, target, label, guess == target["name"], f, g["key"]
