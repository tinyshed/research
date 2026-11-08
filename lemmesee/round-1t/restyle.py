"""Round 1t: a schema renamed into another house style, its keys kept.

`restyle(schema, rng)` draws a style — how words are shortened, how a name's
words are joined and cased, which words end a key and a reference, what
prefixes tables carry — and renames every table and column of the schema in
it, one shortening a word throughout the schema, so that a reference and its
target stay as alike as the style lets them. The primary keys and the
declared keys are renamed with them. Nothing is read from any corpus: the
styles are drawn from the lists below, written for round 1t (teacher.md says
when, and what they were written after).
"""

import random
import re

# Words that end a key's or a reference's name in some house style
KEY_ENDS = ["id", "key", "no", "nr", "num", "code", "cd", "ref", "link", "fk", "ptr", "uid", "ident", "handle",
            "token", "serial", "registry", "reg", "sk", "idx", "tie", "join", "pk", "seq", "oid"]
TABLE_PREFIXES = ["", "", "", "tbl", "t", "tb", "dim", "app", "core", "m"]
CASES = ["snake", "camel", "pascal", "upper", "concat", "upperconcat", "kebabless"]
VOWELS = set("aeiou")
# Set by a fit that shortens words as abbrev.json's expansions say people do
# (customer to cust); off for the renamed test, so that it stays as drawn
DICTIONARY = False
_REVERSE = None


def reverse():
    global _REVERSE
    if _REVERSE is None:
        import json
        from pathlib import Path
        _REVERSE = {}
        for short, longs in json.load(open(Path(__file__).resolve().parent / "abbrev.json", encoding="utf-8")).items():
            for full in longs:
                if " " not in full:
                    _REVERSE.setdefault(full, []).append(short)
    return _REVERSE


def words(name: str) -> list[str]:
    """A name's words, as round 1's names.rs splits them, digits kept."""
    out, cur = [], ""
    for i, c in enumerate(name):
        if not c.isalnum():
            if cur:
                out.append(cur)
                cur = ""
            continue
        if c.isupper() and cur:
            prev = name[i - 1]
            nxt = i + 1 < len(name) and name[i + 1].islower()
            if prev.islower() or prev.isdigit() or (prev.isupper() and nxt):
                out.append(cur)
                cur = ""
        cur += c.lower()
    if cur:
        out.append(cur)
    return out


def shorten(word: str, how: str, rng: random.Random) -> str:
    if len(word) <= 3 or word.isdigit():
        return word
    if how == "first":
        return word[:rng.choice((3, 4, 4, 5))]
    if how == "consonants":
        s = word[0] + "".join(c for c in word[1:] if c not in VOWELS)
        return s[:rng.choice((3, 4, 5))] if len(s) >= 3 else word[:4]
    if how == "mixed":
        return shorten(word, rng.choice(("first", "consonants", "keep")), rng)
    if how == "dictionary":
        known = reverse().get(word)
        return rng.choice(known) if known else shorten(word, rng.choice(("first", "consonants", "keep")), rng)
    return word


def join(ws: list[str], case: str) -> str:
    ws = [w for w in ws if w]
    if not ws:
        return "x"
    if case == "snake":
        return "_".join(ws)
    if case == "camel":
        return ws[0] + "".join(w[:1].upper() + w[1:] for w in ws[1:])
    if case == "pascal":
        return "".join(w[:1].upper() + w[1:] for w in ws)
    if case == "upper":
        return "_".join(ws).upper()
    if case == "upperconcat":
        return "".join(ws).upper()
    if case == "kebabless":  # first word lower, the rest capitals: a mix seen in grown schemas
        return ws[0] + "_" + "_".join(w.upper() for w in ws[1:]) if len(ws) > 1 else ws[0]
    return "".join(ws)


def draw(rng: random.Random) -> dict:
    """A house style."""
    pk_end = rng.choice(KEY_ENDS)
    ref_end = pk_end if rng.random() < 0.4 else rng.choice(KEY_ENDS)
    return {
        "shorten": rng.choice(("keep", "keep", "first", "consonants", "mixed", "dictionary", "dictionary")
                              if DICTIONARY else ("keep", "keep", "first", "consonants", "mixed")),
        "shorten_share": rng.choice((0.3, 0.6, 1.0)),
        "case": rng.choice(CASES),
        "mixed_case": rng.random() < 0.25,  # each table in its own case
        "table_prefix": rng.choice(TABLE_PREFIXES),
        "pk_end": pk_end, "ref_end": ref_end,
        "pk_named": rng.choice(("keep", "entity", "entity", "bare")),  # a bare id becomes the entity's word and pk_end
        "ref_from": rng.choice(("keep", "target pk", "entity")),  # what a reference is built from
        "ref_varied": rng.random() < 0.3,  # each reference picks its own end word
        "singular_tables": rng.random() < 0.5,
    }


def singular(w: str) -> str:
    if w.endswith("ies") and len(w) > 4:
        return w[:-3] + "y"
    if w.endswith(("ses", "xes")):
        return w[:-2]
    if w.endswith("s") and not w.endswith("ss") and len(w) > 3:
        return w[:-1]
    return w


def restyle(schema: dict, rng: random.Random, style: dict | None = None) -> dict:
    st = style or draw(rng)
    vocab = {}

    def word(w):
        if w not in vocab:
            vocab[w] = shorten(w, st["shorten"], rng) if rng.random() < st["shorten_share"] else w
        return vocab[w]

    table_case = {t["name"]: (rng.choice(CASES) if st["mixed_case"] else st["case"]) for t in schema["tables"]}
    tnames, entity = {}, {}
    for t in schema["tables"]:
        ws = words(t["name"]) or ["t"]
        if st["singular_tables"]:
            ws = ws[:-1] + [singular(ws[-1])]
        ent = [word(w) for w in ws if w not in (st["table_prefix"],)]
        entity[t["name"]] = ent
        pre = [st["table_prefix"]] if st["table_prefix"] else []
        tnames[t["name"]] = join(pre + ent, table_case[t["name"]])
    if len(set(n.lower() for n in tnames.values())) < len(tnames):
        return None  # two tables shortened alike: the style does not fit this schema

    refs = {(a, c): b for a, c, b in schema["fk"]}
    pk_of = {t["name"]: t["pk"] for t in schema["tables"]}
    new_pk = {}

    def key_name(t, c):
        ws = words(c)
        case = table_case[t["name"]]
        if st["pk_named"] == "keep" or not ws:
            return join([word(w) for w in ws], case)
        if st["pk_named"] == "bare" and len(ws) == 1 and ws[0] in ("id", "key", "code", "no"):
            return join([st["pk_end"]], case)
        body = ws[:-1] if len(ws) > 1 and ws[-1] in KEY_ENDS else (ws if ws[0] not in KEY_ENDS else [])
        body = [word(w) for w in body] or entity[t["name"]]
        return join(body + [st["pk_end"]], case)

    cols = {}
    for t in schema["tables"]:
        case = table_case[t["name"]]
        for c in t["columns"]:
            n = c["name"]
            if len(t["pk"]) == 1 and n == t["pk"][0] and (t["name"], n) not in refs:
                new = key_name(t, n)
                new_pk[t["name"]] = new
            elif (t["name"], n) in refs:
                continue  # after every key is named
            else:
                new = join([word(w) for w in words(n)], case)
            cols[(t["name"], n)] = new
    for t in schema["tables"]:
        case = table_case[t["name"]]
        for c in t["columns"]:
            n = c["name"]
            if (t["name"], n) not in refs:
                continue
            target = refs[(t["name"], n)]
            ws = words(n)
            end = rng.choice(KEY_ENDS) if st["ref_varied"] else st["ref_end"]
            body = ws[:-1] if len(ws) > 1 and ws[-1] in KEY_ENDS else ws
            how = st["ref_from"]
            pk_words = words(new_pk[target]) if target in new_pk and len(pk_of[target]) == 1 else []
            pk_body = pk_words[:-1] if len(pk_words) > 1 and pk_words[-1] in KEY_ENDS else pk_words
            if how == "target pk" and pk_body and not all(w in KEY_ENDS for w in pk_body):
                new = join(pk_body + [end], case)
            elif how == "entity" and target in entity and body == words(target)[:len(body)] + []:
                new = join(entity[target] + [end], case)
            else:
                new = join([word(w) for w in body] + [end], case)
            cols[(t["name"], n)] = new
    out_tables = []
    for t in schema["tables"]:
        seen, columns = set(), []
        for c in t["columns"]:
            new = cols[(t["name"], c["name"])]
            k = 2
            base = new
            while new.lower() in seen:
                new = f"{base}{k}"
                k += 1
            seen.add(new.lower())
            cols[(t["name"], c["name"])] = new
            columns.append({**c, "name": new})
        out_tables.append({**t, "name": tnames[t["name"]], "columns": columns,
                           "pk": [cols[(t["name"], p)] for p in t["pk"] if (t["name"], p) in cols]})
    fk = [[tnames[a], cols[(a, c)], tnames[b]] for a, c, b in schema["fk"] if (a, c) in cols and b in tnames]
    return {**schema, "tables": out_tables, "fk": fk, "closed": [], "restyled": st}


def check() -> None:
    """A demonstration on a small schema, printed."""
    s = {"id": "demo", "repo": "demo", "split": "train", "fk": [["orders", "customer_id", "customers"],
                                                                 ["order_items", "order_id", "orders"],
                                                                 ["order_items", "product_id", "products"]],
         "tables": [{"name": n, "pk": [pk], "columns": [{"name": c, "family": "int", "type": "integer", "primary": c == pk,
                                                          "nullable": c != pk, "unique": False, "index": False,
                                                          "position": i} for i, c in enumerate(cols)]}
                    for n, pk, cols in (("customers", "id", ["id", "full_name", "email"]),
                                        ("orders", "id", ["id", "customer_id", "placed_at", "total_amount"]),
                                        ("products", "product_id", ["product_id", "title", "unit_price"]),
                                        ("order_items", "id", ["id", "order_id", "product_id", "quantity"]))]}
    for k in range(6):
        r = restyle(s, random.Random(k))
        if r is None:
            print("(refused)")
            continue
        print({kk: v for kk, v in r["restyled"].items() if kk in ("case", "pk_end", "ref_end", "shorten", "ref_from")})
        for t in r["tables"]:
            print("  ", t["name"], t["pk"], [c["name"] for c in t["columns"]])
        print("   fk", r["fk"])


if __name__ == "__main__":
    check()
