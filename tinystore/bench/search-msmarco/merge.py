#!/usr/bin/env python3
"""FTS5's dev answers from its two runs: usage: python -I merge.py <data>.

FTS5 takes seconds a query, so it answered the sample (1,000 dev queries
and the TREC ones) and then dev-rest, the 5,980 dev queries the sample does
not hold. This joins the sample's dev queries and dev-rest into
answers/fts5-dev.jsonl.gz, in dev-rest's order after the sample's, and
checks that every dev qid is there once."""

import gzip
import json
import pathlib
import sys

data = pathlib.Path(sys.argv[1]) / "answers"
dev = {json.loads(line)["qid"] for line in gzip.open(data / "pisa-ranked_or-dev.jsonl.gz", "rt", encoding="utf-8")}
rows = [json.loads(line) for line in gzip.open(data / "fts5-sample.jsonl.gz", "rt", encoding="utf-8")]
rows = [row for row in rows if row["qid"] in dev]
rows += [json.loads(line) for line in gzip.open(data / "fts5-dev-rest.jsonl.gz", "rt", encoding="utf-8")]
qids = [row["qid"] for row in rows]
assert len(qids) == len(set(qids)) == len(dev) and set(qids) == dev, (len(qids), len(set(qids)), len(dev))
with gzip.open(data / "fts5-dev.jsonl.gz", "wt", encoding="utf-8", compresslevel=9) as out:
    for row in rows:
        out.write(json.dumps(row, separators=(",", ":")) + "\n")
print(json.dumps({"step": "merge", "engine": "fts5", "queries": len(rows)}))
