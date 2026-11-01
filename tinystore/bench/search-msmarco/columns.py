#!/usr/bin/env python3
"""The compact store's key rows divided by column, which dbstat cannot do:
usage: python3 -I columns.py <compact.db>. A key row holds the key, its
number and count, and either its directory or its postings inline; dbstat
gives the b-tree's pages, this the payload bytes of each column in them."""

import json
import sqlite3
import sys

connection = sqlite3.connect(f"file:{sys.argv[1]}?mode=ro", uri=True)
row = connection.execute(
    "select count(*), sum(length(key)), sum(length(directory)), sum(length(inline)),"
    " sum(directory is not null), sum(inline is not null), sum(documents),"
    " sum(case when inline is not null then documents else 0 end) from ckeys"
).fetchone()
blob = connection.execute("select count(*), sum(length(payload)) from cblob").fetchone()
names = ["keys", "key_bytes", "directory_bytes", "inline_bytes", "keys_with_blocks", "keys_inline",
         "postings", "postings_inline"]
out = dict(zip(names, row))
out.update({"blocks": blob[0], "block_payload_bytes": blob[1], "sqlite": sqlite3.sqlite_version})
print(json.dumps({"step": "columns", "engine": "compact", **out}))
