#!/usr/bin/env python3
"""Separates what parts BM25 variant a from b: usage: python -I variants.py <data> <corpus> <dev.tsv>.

Variants a and b differ only in idf: a is Lucene's, b the Robertson–Spärck
Jones idf that FTS5 and PISA floor at 10⁻⁶, which differs most for a term in
more than half the passages, where b's is the floor. If the floor is what
parts them, the dev queries that hold none of those terms should rank alike
under both, and the difference should sit in the queries that hold one.
Reads exact-dev from <data>/answers, the terms frequent.py listed from
<data>/logs/frequent.jsonl, each dev query's terms from <dev.tsv> as queries
wrote it (it is the corpus's, and stays out of the repository), and prints
MRR@10 under a, b and l, split by whether a query holds such a term."""

import gzip
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from evaluate import bootstrap, per_query, qrels, variant_run  # noqa: E402

data, corpus, dev = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), pathlib.Path(sys.argv[3])
sys.stdout.reconfigure(encoding="utf-8")
frequent = json.loads((data / "logs" / "frequent.jsonl").read_text(encoding="utf-8"))["terms"]
holding = {}
for line in dev.read_text(encoding="utf-8").splitlines():
    qid, terms = line.split("\t", 1)
    holding[qid] = any(term in frequent for term in terms.split())
with gzip.open(data / "answers" / "exact-dev.jsonl.gz", "rt", encoding="utf-8") as lines:
    rows = {row["qid"]: row for row in map(json.loads, lines)}
judged = qrels(corpus / "qrels.dev.small.tsv")
rr = {v: per_query(variant_run(rows, v), judged, 1)[0] for v in "abl"}
out = {}
print("| dev queries | count | MRR@10 a | MRR@10 b | MRR@10 l | a − b | 95 % interval | queries whose rank differs, a and b |")
print("|---|---:|---:|---:|---:|---:|---|---:|")
for name, wanted in (("holding a term in more than half the passages", True), ("holding none", False)):
    qids = [q for q in judged if holding.get(q) is wanted]
    pick = {v: {q: rr[v][q] for q in qids} for v in "abl"}
    means = {v: sum(pick[v].values()) / len(qids) for v in "abl"}
    interval = bootstrap(pick["a"], pick["b"], 20261101)
    differ = sum(abs(pick["a"][q] - pick["b"][q]) > 1e-12 for q in qids)
    out[name] = {"queries": len(qids), **{f"mrr_{v}": means[v] for v in "abl"}, "a_minus_b_interval": interval,
                 "rank_differs": differ}
    print(f"| {name} | {len(qids)} | {means['a']:.4f} | {means['b']:.4f} | {means['l']:.4f} | "
          f"{means['a'] - means['b']:+.4f} | [{interval[0]:+.4f}, {interval[1]:+.4f}] | {differ} |")
(data / "variants.json").write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
