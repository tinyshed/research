#!/usr/bin/env python3
"""A results directory as the report's tables and the figures its cards and
charts are drawn from: each figure the median of its passes, never typed in.

  python3 summarize.py results/<name>          writes summary.json and summary.md there

A case that failed in a pass is left out of that pass and named under
"failed"; a figure is the median of the passes that measured it.
"""
import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path

MIB = 1 << 20
LEVELS = [1, 8, 64, 256]


def lines(directory, name):
    path = directory / f"{name}.jsonl"
    if not path.exists():
        return []
    return [json.loads(raw) for raw in path.read_text().splitlines() if raw.strip()]


def median(values):
    values = [v for v in values if v is not None]
    return statistics.median(values) if values else None


def by_contender(rows):
    out = defaultdict(list)
    failed = []
    for row in rows:
        if row.get("error") or row.get("exit") not in (0, None):
            failed.append({"contender": row["contender"], "pass": row["pass"], "error": row.get("error", "")[-300:]})
            continue
        out[row["contender"]].append(row)
    return out, failed


def stages(runs):
    """{(stage, inFlight): {perSecond, p50us, p99us, errors}} as medians over passes."""
    found = defaultdict(lambda: defaultdict(list))
    for run in runs:
        for result in run["results"]:
            if "stage" in result:
                key = (result["stage"], result["inFlight"])
                for name in ("perSecond", "p50us", "p99us", "errors"):
                    found[key][name].append(result[name])
    return {f"{stage}@{n}": {name: median(values) for name, values in measures.items()}
            for (stage, n), measures in found.items()}


def phase(runs, name, measure):
    return median([measure(run["phases"][name]) for run in runs if name in run.get("phases", {})])


def group(summary, name, field="pss_median"):
    return summary["groups"].get(name, {}).get(field)


def services_total(summary, field="pss_median"):
    """Every group but the program's: the services, or TinyStore's server."""
    return sum(values[field] for group_name, values in summary["groups"].items() if group_name != "app")


def load_memory(runs, stage, n):
    name = f"load:{stage}:{n}"
    return {
        "total_max": phase(runs, name, lambda s: s["pss_total_max"]),
        "services_max": phase(runs, name, lambda s: services_total(s, "pss_max")),
        "app_max": phase(runs, name, lambda s: group(s, "app", "pss_max")),
        "cpu_seconds": phase(runs, name, lambda s: sum(g["cpu_seconds"] for g in s["groups"].values())),
    }


def idle(directory):
    contenders, failed = by_contender(lines(directory, "idle"))
    out = {}
    for contender, runs in contenders.items():
        groups = defaultdict(list)
        for run in runs:
            for name, values in run["phases"].get("idle", {}).get("groups", {}).items():
                groups[name].append(values["pss_median"])
        ready = defaultdict(list)
        for run in runs:
            for name, seconds in run.get("ready", {}).items():
                ready[name].append(seconds)
        out[contender] = {
            "passes": len(runs),
            "pss_total": phase(runs, "idle", lambda s: s["pss_total_median"]),
            "pss_services": phase(runs, "idle", services_total),
            "groups": {name: median(values) for name, values in groups.items()},
            "cgroup_anon": median([run["phases"]["idle"]["cgroup_anon_median"] - run["cgroup_before"].get("anon", 0)
                                   for run in runs if "idle" in run["phases"]]),
            "cgroup_file": median([run["phases"]["idle"]["cgroup_file_median"] - run["cgroup_before"].get("file", 0)
                                   for run in runs if "idle" in run["phases"]]),
            "open_ms": median([r["openMs"] for run in runs for r in run["results"] if "openMs" in r]),
            "ready": {name: median(values) for name, values in ready.items()},
            "disk": median([sum((d or {}).get("allocated", 0) for ds in run["disk"].values() for d in ds)
                            for run in runs]),
        }
    return {"contenders": out, "failed": failed}


def throughput(directory, name):
    contenders, failed = by_contender(lines(directory, name))
    out = {}
    for contender, runs in contenders.items():
        measured = stages(runs)
        memory = {}
        for key in measured:
            stage, n = key.split("@")
            memory[key] = load_memory(runs, stage, int(n))
            calls = measured[key]["perSecond"]
            seconds = phase(runs, f"load:{stage}:{n}", lambda s: s["seconds"])
            if calls and seconds and memory[key]["cpu_seconds"] is not None:
                memory[key]["cpu_us_per_call"] = memory[key]["cpu_seconds"] / (calls * seconds) * 1e6
        out[contender] = {"passes": len(runs), "stages": measured, "memory": memory,
                          "rest_total": phase(runs, "rest", lambda s: s["pss_total_median"]),
                          "rest_services": phase(runs, "rest", services_total),
                          "rest_app": phase(runs, "rest", lambda s: group(s, "app")),
                          "rest_cgroup_file": phase(runs, "rest", lambda s: s.get("cgroup_file_median")),
                          "disk": median([sum((d or {}).get("allocated", 0) for ds in run["disk"].values() for d in ds)
                                          for run in runs])}
        kept = [r["kept"] for run in runs for r in run["results"] if "kept" in r]
        if kept:
            out[contender]["kept"] = kept
    return {"contenders": out, "failed": failed}


def archive_disk(run):
    for result in run["results"]:
        if "disk" in result:
            return result["disk"]
    return {}


def first(run, key):
    for result in run["results"]:
        if key in result:
            return result[key]
    return None


def queries(runs):
    found = defaultdict(list)
    sizes = {}
    for run in runs:
        for result in run["results"]:
            if "query" in result and "medianMs" in result:
                found[result["query"]].append(result["medianMs"])
                if result.get("size"):
                    sizes[result["query"]] = result["size"]
            if "points" in result:
                sizes[result["points"]["query"]] = result["points"]["points"]
    return {name: {"ms": median(values), "size": sizes.get(name)} for name, values in found.items()}


def store_group(contender):
    return {"tinystore": "app", "victoria": "victoria-metrics", "prometheus": "prometheus", "loki": "loki"}[contender]


def metrics(directory):
    contenders, failed = by_contender(lines(directory, "metrics"))
    out = {}
    for name, runs in contenders.items():
        contender, corpus = name.split("-", 1)
        holder = store_group(contender)
        ingest = [first(run, "ingest") for run in runs]
        samples = ingest[0]["samples"]
        disks = [archive_disk(run) for run in runs]
        entry = {
            "contender": contender, "corpus": corpus, "passes": len(runs), "samples": samples,
            "series": ingest[0]["series"],
            "ingest_per_second": median([i["perSecond"] for i in ingest]),
            "ingest_seconds": median([i["seconds"] for i in ingest]),
            "inside_seconds": median([i["insideSeconds"] for i in ingest]),
            "settle_seconds": median([first(run, "settle")["seconds"] for run in runs]),
            "disk_closed": median([d["closed"]["allocated"] for d in disks]),
            "disk_closed_apparent": median([d["closed"]["apparent"] for d in disks]),
            "queries": queries(runs),
            "memory_ingest_max": phase(runs, "ingest", lambda s: group(s, "promtool" if contender == "prometheus"
                                                                        else holder, "pss_max")),
            "memory_query_max": phase(runs, "query", lambda s: group(s, holder, "pss_max")),
            "memory_settled": phase(runs, "query", lambda s: group(s, holder, "pss_median")),
            "cpu_ingest": phase(runs, "ingest", lambda s: sum(g["cpu_seconds"] for g in s["groups"].values())),
        }
        if contender == "victoria":
            entry["disk_merged"] = median([d["merged"]["allocated"] for d in disks])
            entry["disk_merged_data"] = median([d["merged"]["parts"].get("data", {}).get("allocated", 0) for d in disks])
            entry["disk_closed_cache"] = median([d["closed"]["parts"].get("cache", {}).get("allocated", 0)
                                                 for d in disks])
        if contender == "prometheus":
            entry["disk_backfilled"] = median([d["backfilled"]["allocated"] for d in disks])
        if contender == "tinystore":
            stat = runs[0].get("dbstat", {}).get("metrics.db", [])
            if isinstance(stat, list):
                entry["dbstat"] = {row["name"]: {"bytes": row["bytes"], "payload": row["payload"]} for row in stat}
                entry["sample_payload"] = entry["dbstat"].get("payloads", {}).get("payload")
        out[name] = entry
    return {"cases": out, "failed": failed}


def logs(directory):
    contenders, failed = by_contender(lines(directory, "logs"))
    prepared = directory / "prepared"
    out = {}
    for name, runs in contenders.items():
        contender, dataset = name.split("-", 1)
        holder = store_group(contender)
        described = first(runs[0], "dataset") and runs[0]["results"][0]
        ingest = [first(run, "ingest") for run in runs]
        disks = [archive_disk(run) for run in runs]
        entry = {
            "contender": contender, "dataset": dataset, "passes": len(runs),
            "lines": described["lines"], "bytes": described["bytes"], "needle": described["needle"],
            "ingest_per_second": median([i["perSecond"] for i in ingest]),
            "settle_seconds": median([first(run, "settle")["seconds"] for run in runs]),
            "disk_closed": median([d["closed"]["allocated"] for d in disks]),
            "disk_closed_apparent": median([d["closed"]["apparent"] for d in disks]),
            "queries": queries(runs),
            "memory_ingest_max": phase(runs, "ingest", lambda s: group(s, holder, "pss_max")),
            "memory_query_max": phase(runs, "query", lambda s: group(s, holder, "pss_max")),
            "cpu_ingest": phase(runs, "ingest", lambda s: sum(g["cpu_seconds"] for g in s["groups"].values())),
        }
        if contender == "loki":
            parts = [d["closed"]["parts"] for d in disks]
            entry["chunks"] = median([p.get("chunks", {}).get("apparent", 0) for p in parts])
            entry["index"] = median([sum(v["apparent"] for k, v in p.items() if k.startswith("tsdb")) for p in parts])
            entry["wal"] = median([p.get("wal", {}).get("apparent", 0) for p in parts])
        if contender == "tinystore":
            stat = runs[0].get("dbstat", {}).get("records.db", [])
            if isinstance(stat, list):
                entry["dbstat"] = {row["name"]: {"bytes": row["bytes"], "payload": row["payload"]} for row in stat}
                entry["pages"] = sum(row["bytes"] for row in stat)
                entry["payload"] = sum(row["payload"] for row in stat)
        baseline = prepared / f"{dataset}.json"
        if baseline.exists():
            base = json.loads(baseline.read_text())
            entry.update({k: base[k] for k in ("gzip6", "zstd3", "zstd19", "unparsed")})
        out[name] = entry
    return {"cases": out, "failed": failed}


def table(rows):
    widths = [max(len(str(row[i])) for row in rows) for i in range(len(rows[0]))]
    def line(row):
        return "| " + " | ".join(str(cell).ljust(width) for cell, width in zip(row, widths)) + " |"
    head = [line(rows[0]), "|" + "|".join("-" * (width + 2) for width in widths) + "|"]
    return "\n".join(head + [line(row) for row in rows[1:]])


def mib(value):
    return "" if value is None else f"{value / MIB:.1f}"


def rate(value):
    if value is None:
        return ""
    if value >= 1e6:
        return f"{value / 1e6:.2f} M"
    if value >= 1e4:
        return f"{value / 1e3:.0f} k"
    if value >= 1e3:
        return f"{value / 1e3:.1f} k"
    return f"{value:.0f}"


def ms(value):
    return "" if value is None else (f"{value:.2f}" if value < 10 else f"{value:.0f}")


def us(value):
    return "" if value is None else (f"{value:.0f}" if value >= 10 else f"{value:.1f}")


def markdown(summary):
    out = []
    if summary["idle"]["contenders"]:
        out.append("## At rest\n")
        rows = [["contender", "PSS, all processes", "program", "store or services", "page cache", "open, ms",
                 "ready, s"]]
        for name, entry in summary["idle"]["contenders"].items():
            services = {k: v for k, v in entry["groups"].items() if k != "app"}
            rows.append([name, mib(entry["pss_total"]), mib(entry["groups"].get("app")),
                         " + ".join(f"{k} {mib(v)}" for k, v in sorted(services.items(), key=lambda kv: -kv[1])) or "",
                         mib(entry["cgroup_file"]), ms(entry["open_ms"]),
                         ", ".join(f"{k} {v:.2f}" for k, v in entry["ready"].items())])
        out += [table(rows), ""]
    for name, title in (("app", "An application"), ("kv", "KV"), ("sql", "SQL"), ("files", "Files"),
                        ("kv-memory", "A million keys")):
        section = summary.get(name, {}).get("contenders", {})
        if not section:
            continue
        out.append(f"## {title}\n")
        stage_names = sorted({key for entry in section.values() for key in entry["stages"]},
                             key=lambda k: (k.split("@")[0], int(k.split("@")[1])))
        rows = [["contender", "stage", "calls/s", "p50 µs", "p99 µs", "peak PSS MiB", "CPU µs/call"]]
        for contender, entry in section.items():
            for key in stage_names:
                if key not in entry["stages"]:
                    continue
                measured, memory = entry["stages"][key], entry["memory"].get(key, {})
                rows.append([contender, key, rate(measured["perSecond"]), us(measured["p50us"]), us(measured["p99us"]),
                             mib(memory.get("total_max")), us(memory.get("cpu_us_per_call"))])
        out += [table(rows), ""]
        if name in ("kv-memory", "app"):
            rows = [["contender", "at rest, all", "program", "store or services", "page cache", "disk MiB"]]
            for contender, entry in section.items():
                rows.append([contender, mib(entry["rest_total"]), mib(entry["rest_app"]), mib(entry["rest_services"]),
                             mib(entry["rest_cgroup_file"]), mib(entry["disk"])])
            out += [table(rows), ""]
        if name == "app":
            rows = [["contender", "requests", "writes", "jobs left", "logs stored", "logs dropped", "versions"]]
            for contender, entry in section.items():
                for kept in entry.get("kept", []):
                    rows.append([contender, kept["requests"], kept["writes"], kept.get("jobsLeft"),
                                 kept.get("logsStored"), kept.get("logsDropped"), kept.get("versions")])
            out += [table(rows), ""]
    cases = summary.get("metrics", {}).get("cases", {})
    if cases:
        out.append("## Metrics\n")
        rows = [["case", "samples", "ingest/s", "settle s", "disk MiB", "B/sample", "payload B/sample",
                 "memory ingest", "memory settled"]]
        for name, e in cases.items():
            rows.append([name, e["samples"], rate(e["ingest_per_second"]), ms(e["settle_seconds"]), mib(e["disk_closed"]),
                         f"{e['disk_closed'] / e['samples']:.2f}",
                         f"{e['sample_payload'] / e['samples']:.2f}" if e.get("sample_payload") else "",
                         mib(e["memory_ingest_max"]), mib(e["memory_settled"])])
        out += [table(rows), ""]
        names = sorted({q for e in cases.values() for q in e["queries"]})
        rows = [["case"] + names]
        for name, e in cases.items():
            rows.append([name] + [ms(e["queries"].get(q, {}).get("ms")) for q in names])
        out += ["Query medians, ms:\n", table(rows), ""]
    cases = summary.get("logs", {}).get("cases", {})
    if cases:
        out.append("## Logs\n")
        rows = [["dataset", "lines", "raw MiB", "gzip -6", "zstd -19", "TinyStore file", "TinyStore pages",
                 "Loki chunks+index", "TinyStore lines/s", "Loki lines/s", "TinyStore MiB", "Loki MiB"]]
        datasets = sorted({e["dataset"] for e in cases.values()}, key=lambda d: next(
            e["bytes"] for e in cases.values() if e["dataset"] == d))
        for dataset in datasets:
            tiny = cases.get(f"tinystore-{dataset}", {})
            loki = cases.get(f"loki-{dataset}", {})
            base = tiny or loki
            raw = base.get("bytes", 0)
            pct = lambda v: "" if not v or not raw else f"{100 * v / raw:.1f} %"
            rows.append([dataset, base.get("lines"), mib(raw), pct(base.get("gzip6")), pct(base.get("zstd19")),
                         pct(tiny.get("disk_closed_apparent")), pct(tiny.get("pages")),
                         pct((loki.get("chunks") or 0) + (loki.get("index") or 0)),
                         rate(tiny.get("ingest_per_second")), rate(loki.get("ingest_per_second")),
                         mib(tiny.get("memory_query_max")), mib(loki.get("memory_query_max"))])
        out += [table(rows), ""]
        names = ["needle-count", "needle-lines", "error-count", "newest-100", "minute"]
        rows = [["dataset"] + [f"{q} TS / Loki" for q in names]]
        for dataset in datasets:
            tiny = cases.get(f"tinystore-{dataset}", {}).get("queries", {})
            loki = cases.get(f"loki-{dataset}", {}).get("queries", {})
            rows.append([dataset] + [f"{ms(tiny.get(q, {}).get('ms'))} / {ms(loki.get(q, {}).get('ms'))}" for q in names])
        out += ["Query medians, ms:\n", table(rows), ""]
    for name in ("idle", "app", "kv", "sql", "files", "kv-memory", "metrics", "logs"):
        failed = summary.get(name, {}).get("failed", [])
        if failed:
            out.append(f"Failed in {name}: " + "; ".join(f"{f['contender']} pass {f['pass']}" for f in failed))
    return "\n".join(out) + "\n"


def main():
    directory = Path(sys.argv[1])
    summary = {
        "idle": idle(directory),
        "app": throughput(directory, "app"),
        "kv": throughput(directory, "kv"),
        "sql": throughput(directory, "sql"),
        "files": throughput(directory, "files"),
        "kv-memory": throughput(directory, "kv-memory"),
        "metrics": metrics(directory),
        "logs": logs(directory),
    }
    (directory / "summary.json").write_text(json.dumps(summary, indent=1))
    (directory / "summary.md").write_text(markdown(summary))
    print(markdown(summary))


if __name__ == "__main__":
    main()
