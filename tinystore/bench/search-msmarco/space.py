#!/usr/bin/env python3
"""H2.4 of the MS MARCO round: each engine's index divided by object, in bytes a posting.

Usage: python -I space.py <data>

Reads <data>/logs: the builds' lines (dbstat for the SQLite stores, a file
each for Tantivy, Lucene and PISA), compact-columns.jsonl and sizes.jsonl.
Every engine indexes the same 345,553,319 postings, a term and a passage
each, which prepare counted and each build counted again; bytes a posting
divides every object by that one count. Writes <data>/h24.json and prints
the report's tables.
"""

import json
import pathlib
import sys

ROLES = {
    "compact": {
        "cblob": "blocks: documents and frequencies",
        "ckeys": "keys, directories, short lists inline",
        "norms": "lengths",
        "stats": "statistics",
        "docs": "document keys",
        "sqlite_autoindex_docs_1": "document keys",
    },
    "fts5": {
        "idx_data": "postings with positions, term dictionary",
        "idx_idx": "segment index",
        "idx_docsize": "lengths",
        "idx_config": "statistics",
    },
    "tantivy": {
        "idx": "documents and frequencies, skips, block maxima",
        "term": "term dictionary",
        "fieldnorm": "lengths",
        "pos": "positions",
        "fast": "document keys",
        "store": "document store, empty",
    },
    "lucene": {
        "doc": "documents and frequencies, skips, impacts",
        "psm": "documents and frequencies, skips, impacts",
        "tim": "term dictionary",
        "tip": "term dictionary",
        "tmd": "term dictionary",
        "nvd": "lengths",
        "nvm": "lengths",
        "pos": "positions",
        "dvd": "document keys",
        "dvm": "document keys",
        "dvs": "document keys",
        "fdt": "document store, empty",
        "fdx": "document store, empty",
        "fdm": "document store, empty",
    },
    "pisa": {
        "index": "documents and frequencies",
        "wand": "block maxima, lengths, term statistics",
        "termlex": "term dictionary",
        "doclex": "document keys",
    },
}
# What a top-ten query reads: the postings and what finds and scores them,
# without positions, document keys or stored documents.
QUERY = {"blocks: documents and frequencies", "keys, directories, short lists inline", "lengths", "statistics",
         "postings with positions, term dictionary", "segment index",
         "documents and frequencies, skips, block maxima", "term dictionary",
         "documents and frequencies, skips, impacts", "documents and frequencies",
         "block maxima, lengths, term statistics"}


def lines(path):
    out = []
    if not path.exists():
        return out
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line.startswith("{"):
            try:
                out.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return out


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    data = pathlib.Path(sys.argv[1])
    logs = data / "logs"
    prepared = next(row for row in lines(logs / "prepare.log") if row.get("step") == "prepare")
    postings = prepared["postings"]
    objects = {}
    counted = {"prepare": postings}
    compact = lines(logs / "build-compact.jsonl")[-1]
    objects["compact"] = [(o["name"], o["bytes"]) for o in compact["objects"]]
    counted["compact"] = compact["postings"]
    fts5 = lines(logs / "build-fts5.jsonl")[-1]
    objects["fts5"] = [(o["name"], o["bytes"]) for o in fts5["objects"]]
    counted["fts5"] = fts5["postings"]
    tantivy = lines(logs / "build-tantivy.jsonl")[-1]
    objects["tantivy"] = [(ext, size) for ext, size in tantivy["files"]["by_extension"].items()]
    counted["tantivy"] = tantivy["postings"]
    sizes = lines(logs / "sizes.jsonl")
    lucene_files = next(row for row in sizes if any(name.endswith(".si") for name in row["by_file"]))
    by_ext = {}
    for name, size in lucene_files["by_file"].items():
        ext = name.rsplit(".", 1)[-1] if "." in name else name
        by_ext[ext] = by_ext.get(ext, 0) + size
    objects["lucene"] = sorted(by_ext.items())
    lucene_build = [row for row in lines(logs / "build-lucene.jsonl") if row.get("engine") == "lucene"][-1]
    counted["lucene"] = lucene_build["postings"]
    pisa = lines(logs / "build-pisa.jsonl")[-1]
    objects["pisa"] = [("index", pisa["index_bytes"]), ("wand", pisa["wand_bytes"]),
                       ("termlex", pisa["termlex_bytes"]), ("doclex", pisa["doclex_bytes"])]
    columns = lines(logs / "compact-columns.jsonl")[-1]

    report = {"postings": postings, "postings_counted": counted, "engines": {}, "compact_columns": columns}
    print(f"Postings, a term and a passage each: {postings:,}; counted again by "
          + ", ".join(f"{k} {v:,}" for k, v in counted.items() if k != "prepare") + ".\n")
    print("| engine | object | role | MiB | bytes a posting |")
    print("|---|---|---|---:|---:|")
    totals = {}
    for engine in ("compact", "fts5", "tantivy", "lucene", "pisa"):
        roles = ROLES[engine]
        rows = []
        for name, size in sorted(objects[engine], key=lambda pair: -pair[1]):
            role = roles.get(name, "metadata, empty tables")
            rows.append({"object": name, "role": role, "bytes": size})
            if size >= 1 << 20:
                print(f"| {engine} | {name} | {role} | {size / 2**20:,.1f} | {size / postings:.3f} |")
        small = sum(r["bytes"] for r in rows if r["bytes"] < 1 << 20)
        print(f"| {engine} | the rest, each under 1 MiB | | {small / 2**20:,.1f} | {small / postings:.3f} |")
        query = sum(r["bytes"] for r in rows if r["role"] in QUERY)
        positions = sum(r["bytes"] for r in rows if r["role"] == "positions")
        whole = sum(r["bytes"] for r in rows)
        totals[engine] = {"query": query, "positions": positions, "whole": whole}
        report["engines"][engine] = {"objects": rows, **totals[engine]}
    print("\n| engine | what a top-ten query reads, MiB | bytes a posting | positions apart, MiB | everything, MiB |")
    print("|---|---:|---:|---:|---:|")
    for engine, total in totals.items():
        positions = f"{total['positions'] / 2**20:,.1f}" if total["positions"] else "—"
        print(f"| {engine} | {total['query'] / 2**20:,.1f} | {total['query'] / postings:.3f} | {positions} | "
              f"{total['whole'] / 2**20:,.1f} |")
    print(f"\nThe compact key rows' payload: keys {columns['key_bytes'] / 2**20:.1f} MiB, directories "
          f"{columns['directory_bytes'] / 2**20:.1f} MiB, inline lists {columns['inline_bytes'] / 2**20:.1f} MiB "
          f"({columns['postings_inline']:,} postings of {columns['keys_inline']:,} keys); blocks' payload "
          f"{columns['block_payload_bytes'] / 2**20:.1f} MiB in {columns['blocks']:,} blocks.")
    (data / "h24.json").write_text(json.dumps(report, indent=1) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
