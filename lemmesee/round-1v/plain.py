#!/usr/bin/env python3
"""Round 1v's clean sets from the repositories round 1t fetched and did not read.

Round 1t's style pool (github.py `pool-style`) was fetched to its first 3 100
repositories by SHA-256 of `owner/name` for the style set and the fresh set.
Both kept only schemas of which at most a quarter of the keys end in `id`;
every schema that passed each other step of the rule and failed that one was
counted and dropped, read by no model and by nobody. They are the plain
schemas of the same pool: any date of creation, no repository SchemaPile
names, no schema a SchemaPile schema holds four fifths of.

The rule, applied here mechanically; counts only are printed:

  1. the pool's first 3 100 repositories by SHA-256, as fetched (the one
     `.sql` file a repository, round 1t's steps 3 to 5, the fixed collector:
     primary keys read by `fill_keys`, types in the training's families);
  2. kept as the fresh set's rule keeps a schema — at least five tables and
     three resolving keys, no copy of a sample database, no table-name set of
     the four GitHub sets read before, not held by a SchemaPile schema, a
     one-column key among its keys — except the style filter, turned round:
     kept when over 25 % of its one-column keys end in `id`;
  3. one schema a table-name set, the earliest created;
  4. in the order of SHA-256 of `owner/name`, the schemas at even places
     (the first, the third, ...) are set A and those at odd places set B.

    python plain.py parse     writes corpus/round-1v/heldout-github-plain-a and -b
    python plain.py style3    writes corpus/round-1v/heldout-github-style3

`style3` is the fresh set's rule exactly (github.py `parse-style fresh`) on
the pool after its first 3 100 repositories by SHA-256, as far as it is
fetched: at most a quarter of the keys ending in `id`, the first 100 that
pass; the fresh set's table-name sets are dropped as the earlier sets' are.

Run with round 1t's parsing environment (sqlglot).
"""

import json
import os
import shutil
import sys
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.normpath(os.path.join(HERE, "..", "round-1t")))

import github as gh  # noqa: E402  (round 1t's collector)

OUT = os.path.normpath(os.path.join(HERE, "..", "corpus", "round-1v"))
PREFIX = 3100
SEEN = (gh.OUT, gh.OUT + "-rest", gh.STYLE_OUT, gh.STYLE_OUT + "-fresh")


def parse(style3: bool = False) -> None:
    gh.PK_FALLBACK = True
    gh.TRAINING_FAMILIES = True
    gh._style_dirs()
    repos = gh.jload("repos.json", {})
    sp = gh.jload("schemapile.json", None)
    posting = {}
    for i, names in enumerate(sp["tables"]):
        for t in names:
            posting.setdefault(t, []).append(i)
    seen_sets = set()
    for d in SEEN:
        with open(os.path.join(d, "hidden.jsonl"), encoding="utf-8") as f:
            for line in f:
                seen_sets.add(frozenset(t["name"] for t in json.loads(line)["tables"]))
    surv = sorted((fn for fn, r in repos.items() if "commit" in r), key=lambda fn: gh.by_hash(repos[fn]["full_name"]))
    fetched = 0
    while fetched < len(surv) and ("path" in repos[surv[fetched]] or "drop" in repos[surv[fetched]]):
        fetched += 1
    span = surv[PREFIX:fetched] if style3 else surv[:PREFIX]
    st = {"pool": len(surv), "prefix by SHA-256": PREFIX, "fetched prefix by SHA-256": fetched, "drops": {}}

    def drop(why):
        st["drops"][why] = st["drops"].get(why, 0) + 1

    ok = []
    for fn in span:
        rec = repos[fn]
        if "drop" in rec:
            drop(rec["drop"])
            continue
        if "path" not in rec:
            drop("not fetched")
            continue
        p = os.path.join(gh.WORK, "files", rec["full_name"].replace("/", "__") + ".sql")
        with open(p, encoding="utf-8", newline="") as f:
            text = f.read()
        try:
            r = gh.parse_file(text)
        except Exception:
            r = None
        if r is None:
            drop("no_table_parsed")
            continue
        d, tables, fks = r
        names = set(tables)
        if gh.overlaps_sample(names):
            drop("sample_database_overlap")
            continue
        if len(tables) < gh.MIN_TABLES:
            drop("fewer_than_%d_tables" % gh.MIN_TABLES)
            continue
        if len(fks) < gh.MIN_FKS:
            drop("fewer_than_%d_resolved_fks" % gh.MIN_FKS)
            continue
        if frozenset(names) in seen_sets:
            drop("a schema of the four GitHub sets")
            continue
        shared = {}
        for t in names:
            for i in posting.get(t, ()):
                shared[i] = shared.get(i, 0) + 1
        top = max(shared.values(), default=0)
        if top >= 3 and top / len(names) >= gh.SP_HOLDS:
            drop("held by a SchemaPile schema")
            continue
        one = [(t, c[0], rt) for t, c, rt in fks if len(c) == 1]
        if not one:
            drop("no one-column key")
            continue
        share = sum(gh.ends_in_id(c) for _, c, _ in one) / len(one)
        if style3 and share > gh.STYLE_MAX_ID:
            drop("style: over %d %% of keys end in id" % round(gh.STYLE_MAX_ID * 100))
            continue
        if not style3 and share <= gh.STYLE_MAX_ID:
            drop("style: at most %d %% of keys end in id (the style set's and the fresh set's)" % round(gh.STYLE_MAX_ID * 100))
            continue
        ok.append(dict(rec, dialect=d, _tables=tables, _fks=fks, _text=text))
    groups = {}
    for rec in ok:
        groups.setdefault(frozenset(rec["_tables"]), []).append(rec)
    kept = []
    for g in groups.values():
        g.sort(key=lambda r: (r["created_at"], r["full_name"]))
        kept.append(g[0])
        for _ in g[1:]:
            drop("duplicate_table_set")
    kept.sort(key=lambda r: gh.by_hash(r["full_name"]))
    st["passing"] = len(kept)
    if style3 and len(kept) < gh.STYLE_TAKE:
        print(json.dumps(st, indent=1))
        sys.exit("fewer than %d pass in the fetched prefix: fetch a longer one" % gh.STYLE_TAKE)
    sets = (("style3", kept[:gh.STYLE_TAKE]),) if style3 else (("plain-a", kept[0::2]), ("plain-b", kept[1::2]))
    for half, chosen in sets:
        out = os.path.join(OUT, "heldout-github-" + half)
        shutil.rmtree(os.path.join(out, "raw"), ignore_errors=True)
        os.makedirs(os.path.join(out, "raw"))
        manifest, lines = [], []
        n_tab = n_fk1 = n_fkc = n_id = 0
        sizes = []
        for rec in chosen:
            fn = rec["full_name"]
            safe = fn.replace("/", "__")
            tables, fks = rec["_tables"], rec["_fks"]
            with open(os.path.join(out, "raw", safe + ".sql"), "w", encoding="utf-8", newline="") as f:
                f.write(rec["_text"])
            manifest.append({"full_name": fn, "created_at": rec["created_at"], "default_branch": rec["default_branch"],
                             "commit": rec["commit"], "path": rec["path"], "licence": rec["licence"],
                             "html_url": "https://github.com/%s/blob/%s/%s" % (fn, rec["commit"],
                                                                             urllib.parse.quote(rec["path"])),
                             "dialect": rec["dialect"]})
            tl = []
            for tn, tab in tables.items():
                cols = [{"name": cn, "family": gh.family(c["type"]), "type": c["type"], "nullable": c["nullable"],
                         "unique": c["unique"], "primary": c["primary"], "index": bool(c.get("index", False)),
                         "position": i} for i, (cn, c) in enumerate(tab["cols"].items())]
                tl.append({"name": tn, "pk": [c for c in tab["pk"] if c in tab["cols"]], "columns": cols})
            one = [[t, c[0], rt] for t, c, rt in fks if len(c) == 1]
            n_fkc += sum(1 for _, c, _ in fks if len(c) > 1)
            n_fk1 += len(one)
            n_id += sum(gh.ends_in_id(c) for _, c, _ in one)
            n_tab += len(tl)
            sizes.append(len(tl))
            source = "githubstyle3" if style3 else "githubplain"
            rid = source + "/" + safe
            lines.append(json.dumps({"id": rid, "repo": rid, "source": source, "licence": rec["licence"] or "",
                                     "split": "external", "novel": False, "closed": [], "tables": tl, "fk": one},
                                    ensure_ascii=False))
        with open(os.path.join(out, "hidden.jsonl"), "w", encoding="utf-8", newline="\n") as f:
            f.write("".join(line + "\n" for line in lines))
        with open(os.path.join(out, "manifest.json"), "w", encoding="utf-8", newline="\n") as f:
            json.dump(manifest, f, indent=1, ensure_ascii=False)
        dates = sorted(m["created_at"] for m in manifest)
        lic = {}
        for m in manifest:
            k = m["licence"] or "none"
            lic[k] = lic.get(k, 0) + 1
        bins = {}
        for lo, hi in ((0, 5), (5, 10), (10, 20), (20, 40), (40, 100), (100, 10 ** 6)):
            bins["%d to %s tables" % (lo, hi - 1 if hi < 10 ** 6 else "more")] = sum(1 for n in sizes if lo <= n < hi)
        s = dict(st, half=half, schemas=len(chosen), tables=n_tab, one_column_fks=n_fk1, composite_fks=n_fkc,
                 key_columns_ending_in_id=n_id, share_ending_in_id=round(n_id / n_fk1, 4) if n_fk1 else None,
                 created_before_2025_07_01=sum(1 for m in manifest if m["created_at"][:10] < gh.CREATED_FROM),
                 created_at_min=dates[0], created_at_max=dates[-1], licences=lic, schemas_by_size=bins)
        with open(os.path.join(out, "summary.json"), "w", encoding="utf-8", newline="\n") as f:
            json.dump(s, f, indent=1)
        with open(os.path.join(out, "sha256.txt"), "w", encoding="utf-8", newline="\n") as f:
            f.write("%s  hidden.jsonl\n" % gh.sha256_file(os.path.join(out, "hidden.jsonl")))
            for fnm in sorted(os.listdir(os.path.join(out, "raw"))):
                f.write("%s  raw/%s\n" % (gh.sha256_file(os.path.join(out, "raw", fnm)), fnm))
        print(json.dumps(s, indent=1))


if __name__ == "__main__":
    if sys.argv[1:] not in (["parse"], ["style3"]):
        sys.exit("usage: plain.py parse | style3")
    parse(sys.argv[1] == "style3")
