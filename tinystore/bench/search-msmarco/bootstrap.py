#!/usr/bin/env python3
"""The timing harness's passes as p50 and p99 with 95 % bootstrap intervals.

Usage: python -I bootstrap.py <timing run directory> <label>

Reads passes.jsonl and every <arm>-<mode>-<round>-<set>.jsonl.gz beside it.
For each arm, mode and set, a pass a round: p50 and p99 of its queries'
latencies, nearest rank, each with a percentile interval from 10,000
resamples of the queries; and each arm's p50 and p99 over Tantivy's in the
same round, resampled in pairs, the same queries for both. Also counts the
queries whose ids differ between an arm's passes, which must be none.
"""

import gzip
import json
import math
import pathlib
import random
import sys
import zlib

RESAMPLES = 10000
SEED = 20261101


def quantile(values, q):
    ordered = sorted(values)
    return ordered[max(1, math.ceil(q * len(ordered))) - 1]


def interval(stat, samples, seed):
    rng = random.Random(seed)
    n = len(samples[0])
    values = []
    for _ in range(RESAMPLES):
        picks = [rng.randrange(n) for _ in range(n)]
        values.append(stat(*[[s[i] for i in picks] for s in samples]))
    values.sort()
    return values[int(0.025 * RESAMPLES)], values[int(0.975 * RESAMPLES) - 1]


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    run, label = pathlib.Path(sys.argv[1]), sys.argv[2]
    passes = {}
    for path in sorted(run.glob("*-*.jsonl.gz")):
        arm, mode, round_, subset = path.name[: -len(".jsonl.gz")].rsplit("-", 3)
        with gzip.open(path, "rt", encoding="utf-8") as lines:
            rows = [json.loads(line) for line in lines]
        passes[(arm, mode, int(round_), subset)] = rows
    print(f"# Timing: {label}\n")
    if label != "measurement":
        print("**This run is labelled `" + label + "`: its numbers are not a measurement.**\n")
    print("| arm | mode | set | round | queries | p50 ms [95 %] | p99 ms [95 %] | p50 / Tantivy [95 %] | p99 / Tantivy [95 %] |")
    print("|---|---|---|---|---|---|---|---|---|")
    summary = []
    for key in sorted(passes, key=lambda k: (k[3], k[1], k[0], k[2])):
        arm, mode, round_, subset = key
        rows = passes[key]
        ms = [row["ns"] / 1e6 for row in rows]
        seed = SEED + zlib.crc32(f"{arm} {mode} {round_} {subset}".encode()) % 1000
        p50, p99 = quantile(ms, 0.5), quantile(ms, 0.99)
        lo50, hi50 = interval(lambda x: quantile(x, 0.5), [ms], seed)
        lo99, hi99 = interval(lambda x: quantile(x, 0.99), [ms], seed + 1)
        line = {"arm": arm, "mode": mode, "round": round_, "set": subset, "queries": len(ms),
                "p50": p50, "p50_interval": [lo50, hi50], "p99": p99, "p99_interval": [lo99, hi99]}
        ratio50 = ratio99 = ""
        base = passes.get(("tantivy", mode, round_, subset))
        if base is not None and arm != "tantivy":
            by_qid = {row["qid"]: row["ns"] / 1e6 for row in base}
            paired = [(m, by_qid[row["qid"]]) for m, row in zip(ms, rows) if row["qid"] in by_qid]
            a, b = [p[0] for p in paired], [p[1] for p in paired]
            r50 = quantile(a, 0.5) / quantile(b, 0.5)
            r99 = quantile(a, 0.99) / quantile(b, 0.99)
            i50 = interval(lambda x, y: quantile(x, 0.5) / quantile(y, 0.5), [a, b], seed + 2)
            i99 = interval(lambda x, y: quantile(x, 0.99) / quantile(y, 0.99), [a, b], seed + 3)
            line.update({"p50_over_tantivy": r50, "p50_over_tantivy_interval": i50,
                         "p99_over_tantivy": r99, "p99_over_tantivy_interval": i99})
            ratio50 = f"{r50:.2f} [{i50[0]:.2f}, {i50[1]:.2f}]"
            ratio99 = f"{r99:.2f} [{i99[0]:.2f}, {i99[1]:.2f}]"
        summary.append(line)
        print(f"| {arm} | {mode} | {subset} | {round_} | {len(ms)} | {p50:.3f} [{lo50:.3f}, {hi50:.3f}] | "
              f"{p99:.3f} [{lo99:.3f}, {hi99:.3f}] | {ratio50} | {ratio99} |")
    differ = {}
    for (arm, mode, round_, subset), rows in passes.items():
        ids = {row["qid"]: row["ids"] for row in rows}
        first = differ.setdefault((arm, subset), {"reference": ids, "passes": 0, "queries_differ": 0})
        first["passes"] += 1
        first["queries_differ"] += sum(ids[q] != first["reference"].get(q, ids[q]) for q in ids)
    print("\n| arm | set | passes | queries whose ids differ from the first pass |")
    print("|---|---|---|---|")
    for (arm, subset), entry in sorted(differ.items()):
        print(f"| {arm} | {subset} | {entry['passes']} | {entry['queries_differ']} |")
    (run / "summary.json").write_text(json.dumps({"label": label, "passes": summary}, indent=1) + "\n",
                                      encoding="utf-8")


if __name__ == "__main__":
    main()
