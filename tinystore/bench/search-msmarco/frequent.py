#!/usr/bin/env python3
"""The terms on which the BM25 variants part: usage: python3 -I frequent.py <out>.

A term in more than half the passages has a Robertson–Spärck Jones idf of at
most zero, which FTS5 and PISA raise to 10⁻⁶, while Lucene's idf, which the
compact blocks and Tantivy share, stays above zero for every term. Prints
those terms with their documents, and how many queries of each set hold one,
from PISA's binary collection and the query files prepare and queries wrote."""

import json
import math
import mmap
import pathlib
import struct
import sys

out = pathlib.Path(sys.argv[1])
terms = (out / "msmarco.terms").read_text(encoding="utf-8").split("\n")
with open(out / "msmarco.docs", "rb") as file:
    docs = mmap.mmap(file.fileno(), 0, access=mmap.ACCESS_READ)
    n = struct.unpack_from("<I", docs, 4)[0]
    at, term, frequent = 8, 0, {}
    while at < len(docs):
        df = struct.unpack_from("<I", docs, at)[0]
        if df * 2 > n:
            frequent[terms[term]] = df
        at += 4 * (1 + df)
        term += 1
listed = {t: {"documents": df, "share": df / n, "lucene_idf": math.log(1 + (n - df + 0.5) / (df + 0.5)),
              "rsj_idf": math.log((n - df + 0.5) / (df + 0.5))} for t, df in sorted(frequent.items(), key=lambda p: -p[1])}
sets = {}
for name in ("dev", "dl19", "dl20", "sample"):
    holding, total = 0, 0
    for line in (out / "q" / f"{name}.tsv").read_text(encoding="utf-8").splitlines():
        total += 1
        holding += any(t in frequent for t in line.split("\t", 1)[1].split())
    sets[name] = {"queries": total, "holding_a_term_in_more_than_half": holding}
print(json.dumps({"step": "frequent", "documents": n, "terms": listed, "sets": sets}))
