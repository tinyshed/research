"""Lays out what fetch-hybrid.sh fetched as the lab reads a cohort: a folder
a language, each with corpus.jsonl.gz, topics/dev.relevant.tsv and
qrels/dev.relevant.tsv, the format of NoMIRACL's.

python hybrid-corpus.py <fetched dir> <out dir>

ESCI: a folder a locale, every product the small version judges in that
locale, its title as the title and its brand, colour, bullet points and
description without markup as the text; the test split's queries, a product
relevant when labelled Exact, and qrels/graded.tsv with E 3, S 2, C 1, I 0
beside. CQADupStack: a folder a forum, its corpus, and its test queries with
their judged duplicates.
"""

import gzip
import json
import re
import sys
from pathlib import Path

import pyarrow.parquet as pq

given = Path(sys.argv[1])
out = Path(sys.argv[2])
markup = re.compile(r"<[^>]+>")


def text(value):
    return "" if value is None else markup.sub(" ", str(value)).strip()


def write_folder(folder, docs, topics, qrels, graded=None):
    target = out / folder
    (target / "topics").mkdir(parents=True, exist_ok=True)
    (target / "qrels").mkdir(parents=True, exist_ok=True)
    with gzip.open(target / "corpus.jsonl.gz", "wt", encoding="utf-8") as corpus:
        for docid, title, body in docs:
            corpus.write(json.dumps({"docid": docid, "title": title, "text": body}, ensure_ascii=False) + "\n")
    (target / "topics/dev.relevant.tsv").write_text(
        "".join(f"{qid}\t{query}\n" for qid, query in topics), encoding="utf-8")
    (target / "qrels/dev.relevant.tsv").write_text(
        "".join(f"{qid}\tQ0\t{docid}\t{grade}\n" for qid, docid, grade in qrels), encoding="utf-8")
    if graded is not None:
        (target / "qrels/graded.tsv").write_text(
            "".join(f"{qid}\tQ0\t{docid}\t{grade}\n" for qid, docid, grade in graded), encoding="utf-8")
    print(json.dumps({"folder": folder, "docs": len(docs), "topics": len(topics), "judged": len(qrels)}))


examples = pq.read_table(given / "esci/shopping_queries_dataset_examples.parquet").to_pylist()
small = [e for e in examples if e["small_version"] == 1]
grades = {"E": 3, "S": 2, "C": 1, "I": 0}
locales = ("us", "es", "jp")
wanted = {(e["product_locale"], e["product_id"]) for e in small}
docs_of = {locale: [] for locale in locales}
# A batch at a time: the products file holds 1.8 million rows.
for batch in pq.ParquetFile(given / "esci/shopping_queries_dataset_products.parquet").iter_batches(batch_size=50_000):
    for p in batch.to_pylist():
        if (p["product_locale"], p["product_id"]) in wanted:
            body = " ".join(part for part in (text(p["product_brand"]), text(p["product_color"]),
                                              text(p["product_bullet_point"]), text(p["product_description"])) if part)
            docs_of[p["product_locale"]].append((p["product_id"], text(p["product_title"]), body))
for locale in locales:
    docs = docs_of[locale]
    judged_here = [e for e in small if e["product_locale"] == locale]
    test = [e for e in judged_here if e["split"] == "test"]
    topics = sorted({(e["query_id"], e["query"].replace("\t", " ").replace("\n", " ")) for e in test})
    qrels = [(e["query_id"], e["product_id"], int(e["esci_label"] == "E")) for e in test]
    graded = [(e["query_id"], e["product_id"], grades[e["esci_label"]]) for e in test]
    write_folder(f"esci-{locale}", docs, topics, qrels, graded)

for forum in sorted(p.name for p in (given / "cqadupstack").iterdir()):
    root = given / "cqadupstack" / forum
    docs = []
    with open(root / "corpus.jsonl", encoding="utf-8") as lines:
        for line in lines:
            d = json.loads(line)
            docs.append((d["_id"], d.get("title", ""), d.get("text", "")))
    qrels = []
    for line in (root / "qrels/test.tsv").read_text(encoding="utf-8").splitlines()[1:]:
        qid, docid, score = line.split("\t")
        qrels.append((qid, docid, int(float(score) > 0)))
    asked = {qid for qid, _, _ in qrels}
    topics = []
    with open(root / "queries.jsonl", encoding="utf-8") as lines:
        for line in lines:
            q = json.loads(line)
            if q["_id"] in asked:
                topics.append((q["_id"], q["text"].replace("\t", " ").replace("\n", " ")))
    write_folder(f"cqa-{forum}", docs, topics, qrels)
