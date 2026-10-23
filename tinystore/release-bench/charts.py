#!/usr/bin/env python3
"""The report's charts from a results directory's summary.json, as PNG and
SVG under charts/: one figure a question, every number from the summary.

  docker run --rm -v <results>:/r python:3.13-slim sh -c 'pip install -q matplotlib && python3 /r/../charts.py /r/<name>'
"""
import json
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

MIB = 1 << 20
OURS = "#5b4fc4"
OURS_LIGHT = "#a79ff0"
THEIRS = ["#8a8f98", "#c27c2c", "#3b8f6a", "#b5485d", "#4a7ab8", "#9a6a2f"]
plt.rcParams.update({"figure.dpi": 130, "font.size": 9, "axes.spines.top": False, "axes.spines.right": False,
                     "axes.grid": True, "grid.alpha": 0.25, "legend.frameon": False})


def colour(name, index):
    if name.startswith("tinystore") or name.startswith("TinyStore"):
        return OURS if "sidecar" not in name and "serve" not in name else OURS_LIGHT
    return THEIRS[index % len(THEIRS)]


def save(figure, directory, name):
    figure.tight_layout()
    for suffix in ("png", "svg"):
        figure.savefig(directory / f"{name}.{suffix}")
    plt.close(figure)


def idle_memory(summary, out):
    idle = summary["idle"]["contenders"]
    order = [name for name in ("bun", "tinystore-embedded", "tinystore-serve", "services", "valkey", "prometheus")
             if name in idle]
    figure, axis = plt.subplots(figsize=(7.5, 3.8))
    groups = sorted({group for name in order for group in idle[name]["groups"]},
                    key=lambda g: (g != "app", g != "tinystore", g))
    left = [0.0] * len(order)
    for index, group in enumerate(groups):
        values = [idle[name]["groups"].get(group, 0) / MIB for name in order]
        label = "the program (Bun)" if group == "app" else group
        bar_colour = "#d6d8de" if group == "app" else colour(group, index)
        axis.barh(order, values, left=left, color=bar_colour, label=label, edgecolor="white", linewidth=0.5)
        left = [a + b for a, b in zip(left, values)]
    for y, total in enumerate(left):
        axis.text(total + 4, y, f"{total:.0f}", va="center")
    axis.invert_yaxis()
    axis.set_xlabel("MiB of memory at rest, PSS of every process, every engine open")
    axis.legend(ncol=5, fontsize=7, loc="upper center", bbox_to_anchor=(0.45, -0.2))
    save(figure, out, "idle-memory")


def by_level(summary, section, stage, contenders, title, out, name, field="perSecond", ylabel="calls a second"):
    entries = summary[section]["contenders"]
    figure, axis = plt.subplots(figsize=(5.2, 3.2))
    for index, contender in enumerate(contenders):
        if contender not in entries:
            continue
        points = sorted((int(key.split("@")[1]), value[field]) for key, value in entries[contender]["stages"].items()
                        if key.split("@")[0] == stage)
        if not points:
            continue
        xs, ys = zip(*points)
        if field != "perSecond":
            ys = [y / 1000 for y in ys]
        axis.plot(xs, ys, marker="o", color=colour(contender, index), label=contender,
                  linestyle="-" if "tinystore" in contender else "--")
    axis.set_xscale("log", base=2)
    axis.set_xticks([1, 8, 64, 256], labels=["1", "8", "64", "256"])
    if field != "perSecond":
        axis.set_yscale("log")
    axis.set_xlabel("calls in flight")
    axis.set_ylabel(ylabel)
    axis.set_title(title, loc="left")
    axis.legend(fontsize=7)
    save(figure, out, name)


def metrics_disk(summary, out):
    cases = summary["metrics"]["cases"]
    corpora = [c for c in ("wide", "long") if f"tinystore-{c}" in cases]
    kinds = [("TinyStore, file", lambda e: e["disk_closed"]),
             ("TinyStore, sample blocks", lambda e: e.get("sample_payload")),
             ("VictoriaMetrics, merged", lambda e: e.get("disk_merged_data")),
             ("Prometheus, blocks", lambda e: e["disk_closed"])]
    owners = ["tinystore", "tinystore", "victoria", "prometheus"]
    figure, axis = plt.subplots(figsize=(6, 3.2))
    width = 0.8 / len(kinds)
    for index, ((label, measure), owner) in enumerate(zip(kinds, owners)):
        values = []
        for corpus in corpora:
            entry = cases.get(f"{owner}-{corpus}")
            value = measure(entry) if entry else None
            values.append(value / entry["samples"] if value else 0)
        xs = [i + index * width for i in range(len(corpora))]
        bars = axis.bar(xs, values, width, label=label,
                        color=[OURS, OURS_LIGHT, THEIRS[1], THEIRS[2]][index])
        axis.bar_label(bars, fmt="%.2f", fontsize=7)
    axis.set_xticks([i + width * 1.5 for i in range(len(corpora))],
                    labels=[f"TSBS {c}\n{cases[f'tinystore-{c}']['samples'] / 1e6:.1f} M samples" for c in corpora])
    axis.set_ylabel("bytes a sample on disk")
    axis.legend(fontsize=7)
    save(figure, out, "metrics-disk")


def metrics_queries(summary, out):
    cases = summary["metrics"]["cases"]
    for corpus in ("wide", "long"):
        names = sorted({q for key, e in cases.items() if key.endswith(corpus) for q in e["queries"]})
        if not names:
            continue
        figure, axis = plt.subplots(figsize=(7, 3.2))
        width = 0.27
        for index, contender in enumerate(("tinystore", "victoria", "prometheus")):
            entry = cases.get(f"{contender}-{corpus}")
            if not entry:
                continue
            values = [entry["queries"].get(q, {}).get("ms") or 0 for q in names]
            axis.bar([i + index * width for i in range(len(names))], values, width, label=contender,
                     color=[OURS, THEIRS[1], THEIRS[2]][index])
        axis.set_yscale("log")
        axis.set_xticks([i + width for i in range(len(names))], labels=names, rotation=20, ha="right", fontsize=7)
        axis.set_ylabel("ms, median")
        axis.set_title(f"TSBS {corpus}: TSBS DevOps queries", loc="left")
        axis.legend(fontsize=7)
        save(figure, out, f"metrics-queries-{corpus}")


def logs_ratio(summary, out):
    cases = summary["logs"]["cases"]
    datasets = sorted({e["dataset"] for e in cases.values()},
                      key=lambda d: next(e["bytes"] for e in cases.values() if e["dataset"] == d))
    kinds = [("gzip -6", lambda t, l: t.get("gzip6")), ("zstd -19", lambda t, l: t.get("zstd19")),
             ("Loki chunks + index", lambda t, l: (l.get("chunks") or 0) + (l.get("index") or 0) if l else None),
             ("TinyStore pages", lambda t, l: t.get("pages")),
             ("TinyStore file", lambda t, l: t.get("disk_closed_apparent"))]
    figure, axis = plt.subplots(figsize=(9, 3.6))
    width = 0.8 / len(kinds)
    for index, (label, measure) in enumerate(kinds):
        values = []
        for dataset in datasets:
            tiny = cases.get(f"tinystore-{dataset}", {})
            loki = cases.get(f"loki-{dataset}", {})
            raw = (tiny or loki).get("bytes")
            value = measure(tiny, loki)
            values.append(100 * value / raw if value and raw else 0)
        axis.bar([i + index * width for i in range(len(datasets))], values, width, label=label,
                 color=[THEIRS[0], THEIRS[4], THEIRS[2], OURS_LIGHT, OURS][index])
    axis.set_xticks([i + width * 2 for i in range(len(datasets))], labels=datasets, rotation=30, ha="right")
    axis.set_ylabel("% of the raw text")
    axis.set_title("LogHub: what each keeps on disk, smallest dataset first", loc="left")
    axis.legend(fontsize=7, ncol=5)
    save(figure, out, "logs-disk")


def logs_queries(summary, out):
    cases = summary["logs"]["cases"]
    names = ["needle-count", "needle-lines", "error-count", "newest-100", "minute"]
    datasets = sorted({e["dataset"] for e in cases.values()},
                      key=lambda d: next(e["bytes"] for e in cases.values() if e["dataset"] == d))
    figure, axes = plt.subplots(1, len(names), figsize=(12, 3.2), sharey=True)
    for axis, query in zip(axes, names):
        for index, contender in enumerate(("tinystore", "loki")):
            values = [cases.get(f"{contender}-{d}", {}).get("queries", {}).get(query, {}).get("ms") or 0
                      for d in datasets]
            axis.plot(range(len(datasets)), values, marker="o", label=contender,
                      color=OURS if contender == "tinystore" else THEIRS[2])
        axis.set_yscale("log")
        axis.set_title(query, loc="left", fontsize=8)
        axis.set_xticks(range(len(datasets)), labels=datasets, rotation=60, ha="right", fontsize=6)
    axes[0].set_ylabel("ms, median")
    axes[0].legend(fontsize=7)
    save(figure, out, "logs-queries")


def main():
    directory = Path(sys.argv[1])
    summary = json.loads((directory / "summary.json").read_text())
    out = directory / "charts"
    out.mkdir(exist_ok=True)
    if summary["idle"]["contenders"]:
        idle_memory(summary, out)
    app = ["tinystore-embedded", "tinystore-sidecar", "tinystore-embedded-joined", "services"]
    if summary["app"]["contenders"]:
        by_level(summary, "app", "request", app, "An application: requests a second", out, "app-throughput")
        by_level(summary, "app", "request", app, "An application: p99", out, "app-p99", "p99us", "p99, ms")
    if summary["kv"]["contenders"]:
        for durability in ("synced", "written"):
            names = [f"{n}-{durability}" for n in ("tinystore-embedded", "tinystore-sidecar", "redis", "valkey")]
            by_level(summary, "kv", "set", names, f"KV writes, {durability}", out, f"kv-set-{durability}")
        names = [f"{n}-synced" for n in ("tinystore-embedded", "tinystore-sidecar", "redis", "valkey")]
        by_level(summary, "kv", "get", names, "KV reads", out, "kv-get")
    if summary["sql"]["contenders"]:
        names = ["tinystore-embedded", "tinystore-sidecar", "postgres"]
        by_level(summary, "sql", "read", names, "SQL: a row by its key", out, "sql-read")
        by_level(summary, "sql", "insert", names, "SQL: a row inserted", out, "sql-insert")
    if summary["files"]["contenders"]:
        names = ["tinystore-embedded", "tinystore-sidecar", "minio"]
        for stage in ("get-16k", "put-16k", "get-1m", "put-1m"):
            by_level(summary, "files", stage, names, f"Files: {stage}", out, f"files-{stage}")
    if summary["metrics"]["cases"]:
        metrics_disk(summary, out)
        metrics_queries(summary, out)
    if summary["logs"]["cases"]:
        logs_ratio(summary, out)
        logs_queries(summary, out)


if __name__ == "__main__":
    main()
