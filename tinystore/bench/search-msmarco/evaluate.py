#!/usr/bin/env python3
"""H2.2 of the MS MARCO round: are the engines' top tens exact, and equally effective?

Usage: python -I evaluate.py <data> <corpus>

<data> is tinystore/reports/data/search-msmarco-2026-11-01, whose answers/
holds each engine's ten a query, <engine>-<set>.jsonl.gz, and the exhaustive
scorer's, exact-<set>.jsonl.gz; <corpus> holds the qrels fetch.sh fetched.
Writes <data>/h22.json and prints the report's tables.

Three checks:

- pruned against exhaustive: an engine's pruned ten against the same engine
  scoring every match (PISA's ranked_or, Tantivy with a count beside its top
  ten, Lucene counting every hit, the blocks' own exhaustive executor);
- engine against its BM25 variant: each engine's ten against exact.rs, which
  scores every posting apart from every engine, under the variant that
  engine computes (a compact, b FTS5 and PISA, l Lucene, t Tantivy);
- effectiveness: MRR@10 and nDCG@10 a set, each run ordered as trec_eval
  orders one, by score and then by document id descending.

Two tens are the same when their scores agree position by position within
the engine's precision; documents may then differ only where scores tie.
"""

import gzip
import json
import math
import pathlib
import random
import sys

K1 = 1.2
VARIANT = {
    "compact": "a",
    "compact-window": "a",
    "compact-cursor": "a",
    "compact-lab": "a",
    "compact-exhaustive": "a",
    "fts5": "b",
    "pisa-block_max_maxscore": "b",
    "pisa-block_max_wand": "b",
    "pisa-ranked_or": "b",
    "lucene": "l",
    "lucene-exhaustive": "l",
    "tantivy": "t",
    "tantivy-exhaustive": "t",
}
EXHAUSTIVE = {
    "compact": "compact-exhaustive",
    "compact-window": "compact-exhaustive",
    "compact-cursor": "compact-exhaustive",
    "compact-lab": "compact-exhaustive",
    "tantivy": "tantivy-exhaustive",
    "lucene": "lucene-exhaustive",
    "pisa-block_max_maxscore": "pisa-ranked_or",
    "pisa-block_max_wand": "pisa-ranked_or",
}
# Lucene leaves BM25's constant k1 + 1 out of its scores; the rest keep it.
SCALE = {"lucene": 1 + K1, "lucene-exhaustive": 1 + K1}
# Relative agreement expected of a score: f64 engines to rounding, f32 ones to theirs.
PRECISION = {"compact": 1e-9, "fts5": 1e-9, "tantivy": 2e-5, "lucene": 2e-5, "pisa": 2e-5}
SEED = 20261101
RESAMPLES = 10000


def precision(engine):
    return PRECISION[engine.split("-")[0]]


def load_jsonl(path):
    with gzip.open(path, "rt", encoding="utf-8") as lines:
        return [json.loads(line) for line in lines]


def answers(data, engine, subset):
    path = data / "answers" / f"{engine}-{subset}.jsonl.gz"
    if not path.exists():
        return None
    scale = SCALE.get(engine, 1.0)
    return {row["qid"]: [(int(doc), float(score) * scale) for doc, score in row["ten"]] for row in load_jsonl(path)}


def exact(data, subset):
    path = data / "answers" / f"exact-{subset}.jsonl.gz"
    if not path.exists():
        return None
    return {row["qid"]: row for row in load_jsonl(path)}


def qrels(path):
    judged = {}
    with open(path, encoding="utf-8") as lines:
        for line in lines:
            parts = line.split()
            if len(parts) == 4:
                qid, _, doc, grade = parts
            else:
                qid, doc, grade = parts[0], parts[2], parts[3]
            judged.setdefault(qid, {})[int(doc)] = int(grade)
    return judged


def same_scores(found, truth, tolerance):
    return len(found) == len(truth) and all(
        abs(a - b) <= tolerance * max(abs(b), 1.0) for (_, a), (_, b) in zip(found, truth)
    )


def worst(found, truth):
    return max((abs(a - b) / max(abs(b), 1.0) for (_, a), (_, b) in zip(found, truth)), default=0.0)


def compare(found, truth, tolerance, ties=None):
    """Counts of the queries both hold: same scores, same documents, and documents that differ."""
    shared = sorted(set(found) & set(truth))
    out = {"queries": len(shared), "same_scores": 0, "same_documents": 0, "documents_differ_at_equal_scores": 0,
           "scores_differ": 0, "worst_relative_difference": 0.0, "differ_qids": []}
    for qid in shared:
        a, b = found[qid], truth[qid]
        out["worst_relative_difference"] = max(out["worst_relative_difference"], worst(a, b))
        if same_scores(a, b, tolerance):
            out["same_scores"] += 1
            if sorted(doc for doc, _ in a) == sorted(doc for doc, _ in b):
                out["same_documents"] += 1
            else:
                out["documents_differ_at_equal_scores"] += 1
        else:
            out["scores_differ"] += 1
            out["differ_qids"].append(qid)
    out["differ_qids"] = out["differ_qids"][:20]
    return out


def trec_order(ten):
    """A run's ten as trec_eval orders it: score descending, a tie by document id as a string, descending."""
    return sorted(ten, key=lambda pair: (pair[1], str(pair[0])), reverse=True)


def per_query(run, judged, relevant_from):
    """MRR@10 and nDCG@10 of each judged query; a query the run lacks scores 0."""
    rr, ndcg = {}, {}
    for qid, grades in judged.items():
        ten = trec_order(run.get(qid, []))[:10]
        rr[qid] = 0.0
        for rank, (doc, _) in enumerate(ten, 1):
            if grades.get(doc, 0) >= relevant_from:
                rr[qid] = 1.0 / rank
                break
        dcg = sum(grades.get(doc, 0) / math.log2(rank + 1) for rank, (doc, _) in enumerate(ten, 1))
        ideal = sorted(grades.values(), reverse=True)[:10]
        idcg = sum(grade / math.log2(rank + 1) for rank, grade in enumerate(ideal, 1))
        ndcg[qid] = dcg / idcg if idcg > 0 else 0.0
    return rr, ndcg


def mean(values):
    values = list(values)
    return sum(values) / len(values) if values else float("nan")


def bootstrap(a, b, seed):
    """A paired 95 % interval of mean(a) - mean(b) over the queries."""
    qids = sorted(a)
    differences = [a[q] - b[q] for q in qids]
    rng = random.Random(seed)
    n = len(differences)
    means = sorted(sum(differences[rng.randrange(n)] for _ in range(n)) / n for _ in range(RESAMPLES))
    return [means[int(0.025 * RESAMPLES)], means[int(0.975 * RESAMPLES) - 1]]


def variant_run(rows, variant):
    return {qid: [(int(doc), float(score)) for doc, score in row[variant]["ten"]] for qid, row in rows.items()}


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    data, corpus = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    judged = {
        "dev": qrels(corpus / "qrels.dev.small.tsv"),
        "dl19": qrels(corpus / "2019qrels-pass.txt"),
        "dl20": qrels(corpus / "2020qrels-pass.txt"),
    }
    relevant_from = {"dev": 1, "dl19": 2, "dl20": 2}
    engines = sorted(VARIANT)
    report = {"pruned_against_exhaustive": {}, "engine_against_variant": {}, "effectiveness": {}, "agreement": {}}

    exact_rows = {subset: exact(data, subset) for subset in ("sample", "dev")}
    for engine in engines:
        for subset in ("sample", "dev"):
            found = answers(data, engine, subset)
            if found is None:
                continue
            if engine in EXHAUSTIVE:
                full = answers(data, EXHAUSTIVE[engine], subset)
                if full is not None:
                    report["pruned_against_exhaustive"][f"{engine} {subset}"] = compare(found, full, precision(engine))
            rows = exact_rows[subset]
            if rows is not None:
                truth = variant_run(rows, VARIANT[engine])
                report["engine_against_variant"][f"{engine} {subset}"] = compare(found, truth, precision(engine))

    # Each set's run of an engine: its own file, or the sample's queries of that set.
    def run_of(engine, subset):
        own = answers(data, engine, subset)
        if own is not None:
            return own, subset
        sample = answers(data, engine, "sample")
        if sample is None:
            return None, None
        wanted = judged[subset]
        part = {qid: ten for qid, ten in sample.items() if qid in wanted}
        return part, "sample"

    runs = {}
    for engine in engines:
        for subset in ("dev", "dl19", "dl20"):
            run, source = run_of(engine, subset)
            if run:
                runs[(engine, subset)] = (run, source)
    for variant in "ablt":
        for subset in ("dev", "dl19", "dl20"):
            source = "dev" if subset == "dev" and exact_rows["dev"] is not None else "sample"
            rows = exact_rows[source]
            if rows is None:
                continue
            run = variant_run(rows, variant)
            if source == "sample":
                run = {qid: ten for qid, ten in run.items() if qid in judged[subset]}
            runs[(f"exact-{variant}", subset)] = (run, source)

    per = {}
    for (engine, subset), (run, source) in runs.items():
        grades = judged[subset]
        # A run of the sample is judged on the sample's queries of the set only.
        if subset == "dev" and source == "sample":
            grades = {qid: g for qid, g in grades.items() if qid in run}
        rr, ndcg = per_query(run, grades, relevant_from[subset])
        per[(engine, subset)] = (rr, ndcg)
        report["effectiveness"][f"{engine} {subset}"] = {
            "queries": len(grades), "from": source, "mrr@10": mean(rr.values()), "ndcg@10": mean(ndcg.values())}

    # Every pair on the same queries: how many queries' metrics differ, and by how much on average.
    keys = sorted(per)
    for subset in ("dev", "dl19", "dl20"):
        names = [engine for engine, s in keys if s == subset]
        for i, a in enumerate(names):
            for b in names[i + 1:]:
                (rr_a, nd_a), (rr_b, nd_b) = per[(a, subset)], per[(b, subset)]
                shared = sorted(set(rr_a) & set(rr_b))
                if not shared:
                    continue
                pick = lambda table: {q: table[q] for q in shared}  # noqa: E731
                entry = {
                    "queries": len(shared),
                    "rr_differ": sum(abs(rr_a[q] - rr_b[q]) > 1e-12 for q in shared),
                    "ndcg_differ": sum(abs(nd_a[q] - nd_b[q]) > 1e-12 for q in shared),
                    "mrr_difference": mean(rr_a[q] - rr_b[q] for q in shared),
                    "ndcg_difference": mean(nd_a[q] - nd_b[q] for q in shared),
                }
                if a.startswith("exact-") and b.startswith("exact-"):
                    entry["mrr_interval"] = bootstrap(pick(rr_a), pick(rr_b), SEED)
                    entry["ndcg_interval"] = bootstrap(pick(nd_a), pick(nd_b), SEED + 1)
                report["agreement"][f"{a} | {b} {subset}"] = entry

    (data / "h22.json").write_text(json.dumps(report, indent=1, sort_keys=True) + "\n", encoding="utf-8")

    print("## Pruned against the same engine scoring every match\n")
    print("| engine, queries | same ten scores | same documents | documents differ at equal scores | scores differ | worst relative difference |")
    print("|---|---|---|---|---|---|")
    for key, row in sorted(report["pruned_against_exhaustive"].items()):
        print(f"| {key}, {row['queries']} | {row['same_scores']} | {row['same_documents']} | "
              f"{row['documents_differ_at_equal_scores']} | {row['scores_differ']} | {row['worst_relative_difference']:.2e} |")
    print("\n## Each engine against exact.rs under its variant\n")
    print("| engine, queries | variant | same ten scores | same documents | documents differ at equal scores | scores differ | worst relative difference |")
    print("|---|---|---|---|---|---|---|")
    for key, row in sorted(report["engine_against_variant"].items()):
        engine = key.split(" ")[0]
        print(f"| {key}, {row['queries']} | {VARIANT[engine]} | {row['same_scores']} | {row['same_documents']} | "
              f"{row['documents_differ_at_equal_scores']} | {row['scores_differ']} | {row['worst_relative_difference']:.2e} |")
    print("\n## Effectiveness\n")
    print("| run | set | queries | from | MRR@10 | nDCG@10 |")
    print("|---|---|---|---|---|---|")
    for key, row in sorted(report["effectiveness"].items()):
        engine, subset = key.split(" ")
        print(f"| {engine} | {subset} | {row['queries']} | {row['from']} | {row['mrr@10']:.4f} | {row['ndcg@10']:.4f} |")
    print("\n## Variants against one another, paired 95 % intervals\n")
    print("| pair | set | queries | MRR@10 a − b | interval | nDCG@10 a − b | interval |")
    print("|---|---|---|---|---|---|---|")
    for key, row in sorted(report["agreement"].items()):
        if "mrr_interval" not in row:
            continue
        pair, subset = key.rsplit(" ", 1)
        print(f"| {pair} | {subset} | {row['queries']} | {row['mrr_difference']:+.4f} | "
              f"[{row['mrr_interval'][0]:+.4f}, {row['mrr_interval'][1]:+.4f}] | {row['ndcg_difference']:+.4f} | "
              f"[{row['ndcg_interval'][0]:+.4f}, {row['ndcg_interval'][1]:+.4f}] |")
    print("\n## Each engine against the exhaustive scorer of its variant: queries whose metric differs\n")
    print("| engine | set | queries | reciprocal rank differs | nDCG@10 differs | MRR@10 difference | nDCG@10 difference |")
    print("|---|---|---|---|---|---|---|")
    for key, row in sorted(report["agreement"].items()):
        pair, subset = key.rsplit(" ", 1)
        a, b = pair.split(" | ")
        engine, other = (b, a) if a.startswith("exact-") else (a, b)
        if engine.startswith("exact-") or other != f"exact-{VARIANT[engine]}":
            continue
        print(f"| {engine} | {subset} | {row['queries']} | {row['rr_differ']} | {row['ndcg_differ']} | "
              f"{-row['mrr_difference'] if engine == b else row['mrr_difference']:+.5f} | "
              f"{-row['ndcg_difference'] if engine == b else row['ndcg_difference']:+.5f} |")


if __name__ == "__main__":
    main()
