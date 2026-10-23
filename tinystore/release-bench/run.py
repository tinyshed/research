#!/usr/bin/env python3
"""The release round: each case a fresh directory, the services its contender
needs started there, then a Bun program that loads them while this process
samples every process's memory and CPU, ten times a second, by the phases the
program marks. A case's line in runs.jsonl holds what the program measured,
memory and CPU by group and phase, each service's time to ready, and each
data directory's size once everything has stopped.

  ROUNDS=idle,kv PASSES=3 SECONDS=10 OUT=/work/results/<name> python3 run.py

Nothing else may build, test or measure on the machine meanwhile.
"""
import json
import os
import shutil
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path

import services

HERE = Path(__file__).resolve().parent
OUT = Path(os.environ.get("OUT", "/work/results/try"))
WORK = Path(os.environ.get("WORK", "/work/runs"))
PASSES = int(os.environ.get("PASSES", "1"))
SECONDS = os.environ.get("SECONDS", "10")
ROUNDS = os.environ.get("ROUNDS", "idle").split(",")
ONLY = set(filter(None, os.environ.get("ONLY", "").split(",")))
PAGE = os.sysconf("SC_PAGE_SIZE")
TICK = os.sysconf("SC_CLK_TCK")


def tinystore(mode, durability="full", **extra):
    return {"tinystore": mode, "durability": durability, **extra}


# A round's contenders: what each starts beside the program, and what it
# tells the program. `services` are started in this order, stopped in reverse.
ROUND_CASES = {
    "idle": [
        ("bun", {"program": "idle", "args": {"contender": "bun"}}),
        ("tinystore-embedded", {"program": "idle", "args": {"contender": "tinystore", "mode": "embedded"}}),
        ("tinystore-sidecar", {"program": "idle", "args": {"contender": "tinystore", "mode": "sidecar"}}),
        # the server started by itself, as a person starts PostgreSQL, which open(dir) then finds
        ("tinystore-serve", {"program": "idle", "args": {"contender": "tinystore", "mode": "sidecar"},
                             "services": ["tinystore"]}),
        ("services", {"program": "idle", "args": {"contender": "services"},
                      "services": ["postgres", "redis", "minio", "victoria", "loki"]}),
        # the alternatives, each alone: a stack runs one or the other
        ("valkey", {"program": "idle", "args": {"contender": "valkey"}, "services": ["valkey"]}),
        ("prometheus", {"program": "idle", "args": {"contender": "prometheus"}, "services": ["prometheus"]}),
    ],
    "kv": [
        (f"{name}-{durability}", {"program": "kv", "args": {"contender": contender, "durability": durability, **extra},
                                  "services": [svc] if svc else []})
        for durability in ("synced", "written")
        for name, contender, extra, svc in (
            ("tinystore-embedded", "tinystore", {"mode": "embedded"}, None),
            ("tinystore-sidecar", "tinystore", {"mode": "sidecar"}, None),
            ("redis", "redis", {}, "redis"),
            ("valkey", "valkey", {}, "valkey"),
        )
    ],
    "kv-memory": [
        ("tinystore-embedded", {"program": "kv-memory", "args": {"contender": "tinystore", "mode": "embedded"}}),
        ("tinystore-sidecar", {"program": "kv-memory", "args": {"contender": "tinystore", "mode": "sidecar"}}),
        ("redis", {"program": "kv-memory", "args": {"contender": "redis"}, "services": ["redis"]}),
        ("valkey", {"program": "kv-memory", "args": {"contender": "valkey"}, "services": ["valkey"]}),
    ],
    "app": [
        ("tinystore-embedded", {"program": "app", "args": {"contender": "tinystore", "mode": "embedded"}}),
        ("tinystore-embedded-joined", {"program": "app",
                                       "args": {"contender": "tinystore", "mode": "embedded", "joined": "1"}}),
        ("tinystore-sidecar", {"program": "app", "args": {"contender": "tinystore", "mode": "sidecar"}}),
        ("services", {"program": "app", "args": {"contender": "services"},
                      "services": ["postgres", "redis", "minio", "victoria-scrape", "loki"]}),
    ],
    "sql": [
        ("tinystore-embedded", {"program": "sql", "args": {"contender": "tinystore", "mode": "embedded"}}),
        ("tinystore-sidecar", {"program": "sql", "args": {"contender": "tinystore", "mode": "sidecar"}}),
        ("postgres", {"program": "sql", "args": {"contender": "postgres"}, "services": ["postgres"]}),
    ],
    "files": [
        ("tinystore-embedded", {"program": "files", "args": {"contender": "tinystore", "mode": "embedded"}}),
        ("tinystore-sidecar", {"program": "files", "args": {"contender": "tinystore", "mode": "sidecar"}}),
        ("minio", {"program": "files", "args": {"contender": "minio"}, "services": ["minio"]}),
    ],
}

ARCHIVE = os.environ.get("ARCHIVE", "/archive/release/archive")
CORPUS = os.environ.get("CORPUS", "/corpus")
DATASETS = os.environ.get("DATASETS", "Apache,HealthApp,Linux,Mac,OpenStack,Proxifier,SSH,Zookeeper,HPC,Hadoop,"
                          "Android_v1,BGL,HDFS_v1").split(",")
ROUND_CASES["metrics"] = [
    (f"{contender}-{corpus}", {"command": [ARCHIVE, "metrics", "--corpus", f"{CORPUS}/tsbs/{corpus}.lp",
                                           "--contender", contender], "args": {}})
    for corpus in ("wide", "long") for contender in ("tinystore", "victoria", "prometheus")
]
ROUND_CASES["logs"] = [
    (f"{contender}-{dataset}", {"command": [ARCHIVE, "logs", "--prepared", f"{CORPUS}/prepared", "--dataset", dataset,
                                            "--contender", contender], "args": {}})
    for dataset in DATASETS for contender in ("tinystore", "loki")
]


def start(name, directory, case):
    """A service by the round's name for it, with what the program is told."""
    durability = case["args"].get("durability", "default")
    if name == "postgres":
        service, data = services.postgres(directory)
        return service, data, {"postgres": service.address}
    if name in ("redis", "valkey"):
        service, data = getattr(services, name)(directory, durability)
        return service, data, {name: service.address}
    if name == "minio":
        service, data = services.minio(directory)
        return service, data, {"minio": service.address}
    if name in ("victoria", "victoria-scrape"):
        scrape = "127.0.0.1:9464" if name == "victoria-scrape" else None
        service, data = services.victoria(directory, scrape)
        return service, data, {"victoria": service.address}
    if name == "prometheus":
        service, data = services.prometheus(directory)
        return service, data, {"prometheus": service.address}
    if name == "loki":
        service, data = services.loki(directory)
        return service, data, {"loki": service.address}
    if name == "tinystore":
        service, data = services.tinystore(directory)
        return service, data, {}
    raise ValueError(f"no service {name}")


def processes():
    """Every process: pid -> (ppid, comm, cpu ticks)."""
    out = {}
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        try:
            raw = Path(f"/proc/{entry}/stat").read_text()
        except OSError:
            continue
        comm = raw[raw.index("(") + 1:raw.rindex(")")]
        fields = raw[raw.rindex(")") + 2:].split()
        if fields[0] == "Z":
            continue  # a zombie holds no memory, and its name is no process's any more
        out[int(entry)] = (int(fields[1]), comm, int(fields[11]) + int(fields[12]))
    return out


def memory(pid):
    """(rss, pss) in bytes, or None once the process is gone."""
    try:
        rss = pss = 0
        for line in Path(f"/proc/{pid}/smaps_rollup").read_text().splitlines():
            if line.startswith("Rss:"):
                rss = int(line.split()[1]) * 1024
            elif line.startswith("Pss:"):
                pss = int(line.split()[1]) * 1024
        return rss, pss
    except (OSError, ValueError):
        return None


def cgroup():
    """The container's memory as the kernel charges it: anonymous memory, and
    the page cache that every process here brought in, which PSS leaves out
    and a store that reads its files through the cache leans on."""
    out = {}
    try:
        for line in Path("/sys/fs/cgroup/memory.stat").read_text().splitlines():
            name, value = line.split()
            if name in ("anon", "file", "shmem", "kernel"):
                out[name] = int(value)
    except OSError:
        pass
    return out


def drop_caches():
    """Empties the page cache, so that what a case's cgroup holds of it is the
    case's own; needs --privileged, and is skipped without it."""
    subprocess.run(["sync"])
    try:
        Path("/proc/sys/vm/drop_caches").write_text("3\n")
        return True
    except OSError:
        return False


# Processes counted by their name wherever they hang: TinyStore's server, and
# the services an archive case starts as its own children. A name is the
# kernel's, cut to fifteen bytes.
NAMED = {"tinystore": "tinystore", "prometheus": "prometheus", "promtool": "promtool", "loki": "loki",
         "victoria-metric": "victoria-metrics"}


class Sampler(threading.Thread):
    """Memory and CPU of each group of processes, ten times a second. A group
    is a process this round started and every process under it; a tinystore
    process that is no one's child here is the sidecar the SDK started, which
    the container's first process adopts."""

    def __init__(self, roots):
        super().__init__(daemon=True)
        self.roots = roots  # group -> pid
        self.samples = []  # (time, {group: (rss, pss, ticks)})
        self.stopping = threading.Event()

    def groups(self, table):
        children = {}
        for pid, (ppid, _, _) in table.items():
            children.setdefault(ppid, []).append(pid)
        # a tinystore process is the store's server wherever it hangs: the
        # sidecar the SDK starts is the program's child, and counted apart
        owned = {}
        for pid, (_, comm, _) in table.items():
            if comm in NAMED:
                stack = [pid]
                while stack:
                    child = stack.pop()
                    owned[child] = NAMED[comm]
                    stack.extend(children.get(child, []))
        roots = dict(self.roots)
        for group, root in roots.items():
            stack = [root]
            while stack:
                pid = stack.pop()
                if pid in table and pid not in owned:
                    owned[pid] = group
                    stack.extend(children.get(pid, []))
        return owned

    def sample(self):
        table = processes()
        totals = {}
        for pid, group in self.groups(table).items():
            measured = memory(pid)
            if measured is None:
                continue
            rss, pss, ticks = totals.get(group, (0, 0, 0))
            totals[group] = (rss + measured[0], pss + measured[1], ticks + table[pid][2])
        self.samples.append((time.time() * 1000, totals, cgroup()))

    def run(self):
        while not self.stopping.is_set():
            self.sample()
            self.stopping.wait(0.1)


def phases(samples, marks, end):
    """Memory and CPU by phase: the samples between a mark and the next."""
    out = {}
    bounds = marks + [{"mark": "end", "at": end}]
    for this, after in zip(bounds, bounds[1:]):
        inside = [totals for at, totals, _ in samples if this["at"] <= at < after["at"]]
        charged = [kernel for at, _, kernel in samples if this["at"] <= at < after["at"]]
        if not inside:
            continue
        groups = sorted({group for totals in inside for group in totals})
        summary = {"seconds": (after["at"] - this["at"]) / 1000, "samples": len(inside), "groups": {}}
        for group in groups:
            rss = [totals[group][0] for totals in inside if group in totals]
            pss = [totals[group][1] for totals in inside if group in totals]
            ticks = [totals[group][2] for totals in inside if group in totals]
            summary["groups"][group] = {
                "rss_max": max(rss), "pss_max": max(pss), "pss_median": sorted(pss)[len(pss) // 2],
                "pss_last": pss[-1], "cpu_seconds": (ticks[-1] - ticks[0]) / TICK,
            }
        whole = [sum(value[1] for value in totals.values()) for totals in inside]
        summary["pss_total_max"] = max(whole)
        summary["pss_total_median"] = sorted(whole)[len(whole) // 2]
        for name in ("anon", "file"):
            values = [kernel.get(name, 0) for kernel in charged]
            summary[f"cgroup_{name}_max"] = max(values)
            summary[f"cgroup_{name}_median"] = sorted(values)[len(values) // 2]
        out[this["mark"]] = summary
    return out


def disk(path):
    """Bytes a directory holds on the disk, and its files' sizes."""
    if not path.exists():
        return None
    allocated = subprocess.run(["du", "-s", "--block-size=1", str(path)], capture_output=True, text=True)
    apparent = subprocess.run(["du", "-s", "-b", str(path)], capture_output=True, text=True)
    return {"allocated": int(allocated.stdout.split()[0]), "apparent": int(apparent.stdout.split()[0])}


def dbstat(path):
    """Each SQLite file's bytes by its tables and indexes, as SQLite's dbstat
    counts its pages: what a report divides a file into."""
    import sqlite3
    out = {}
    if not path.exists():
        return out
    for file in sorted(path.rglob("*.db")):
        try:
            with sqlite3.connect(f"file:{file}?mode=ro", uri=True) as db:
                rows = db.execute("select name, count(*), sum(pgsize), sum(payload), sum(unused) from dbstat group by name"
                                  " order by sum(pgsize) desc").fetchall()
            out[str(file.relative_to(path))] = [
                {"name": name, "pages": pages, "bytes": size, "payload": payload, "unused": unused}
                for name, pages, size, payload, unused in rows]
        except sqlite3.Error as error:
            out[str(file.relative_to(path))] = repr(error)
    return out


def stop_sidecars():
    for pid, (_, comm, _) in processes().items():
        if comm == "tinystore":
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline and any(comm == "tinystore" for _, comm, _ in processes().values()):
        time.sleep(0.05)


def run_case(round_name, contender, case, pass_index):
    directory = WORK / f"{round_name}-{contender}-{pass_index}"
    shutil.rmtree(directory, ignore_errors=True)
    directory.mkdir(parents=True)
    os.chmod(directory, 0o777)
    store = directory / "store"
    started, data, told, ready = [], {}, {}, {}
    line = {"round": round_name, "contender": contender, "pass": pass_index, "started": time.time(),
            "dropped_caches": drop_caches(), "cgroup_before": cgroup()}
    try:
        for name in case.get("services", []):
            service, paths, address = start(name, directory, case)
            started.append(service)
            data[service.name] = paths
            told.update(address)
            ready[service.name] = service.ready_seconds
        if "command" in case:
            command = case["command"] + ["--dir", str(directory / "case")]
            if os.environ.get("REPEAT"):
                command += ["--repeat", os.environ["REPEAT"]]
        else:
            command = ["bun", str(HERE / "bench" / f"{case['program']}.ts"), "--dir", str(store), "--seconds", SECONDS]
        if os.environ.get("REST"):
            command += ["--rest", os.environ["REST"]]
        for key, value in {**case["args"], **told}.items():
            command += [f"--{key}", str(value)]
        sampler = Sampler({service.name: service.process.pid for service in started})
        sampler.sample()
        sampler.start()
        marks = [{"mark": "start", "at": time.time() * 1000}]
        results = []
        program = subprocess.Popen(command, cwd=HERE, stdout=subprocess.PIPE, stderr=open(directory / "bun.log", "wb"),
                                   text=True)
        sampler.roots["app"] = program.pid
        for raw in program.stdout:
            try:
                message = json.loads(raw)
            except json.JSONDecodeError:
                continue
            if "mark" in message:
                marks.append(message)
            elif "result" in message:
                results.append(message["result"])
        code = program.wait()
        end = time.time() * 1000
        sampler.stopping.set()
        sampler.join()
        line.update(exit=code, results=results, phases=phases(sampler.samples, marks, end), ready=ready,
                    marks=[m["mark"] for m in marks])
        if code != 0:
            line["error"] = (directory / "bun.log").read_text(errors="replace")[-4000:]
    except Exception as error:  # a failed case is reported and the round goes on
        line["error"] = repr(error)
    finally:
        stop_sidecars()
        for service in reversed(started):
            service.stop()
    line["disk"] = {name: [disk(path) for path in paths] for name, paths in data.items()}
    line["disk"]["tinystore"] = [disk(store)]
    line["dbstat"] = dbstat(store if store.exists() else directory / "case" / "store")
    line["finished"] = time.time()
    shutil.rmtree(directory, ignore_errors=True)
    return line


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    with open(OUT / "versions.txt", "w") as versions:
        subprocess.run(["sh", str(HERE / "versions.sh")], cwd=HERE, stdout=versions, stderr=subprocess.STDOUT)
    failed = 0
    for round_name in ROUNDS:
        cases = [case for case in ROUND_CASES[round_name] if not ONLY or case[0] in ONLY]
        for pass_index in range(PASSES):
            # each pass starts one contender later, so that none always runs first
            turned = cases[pass_index % len(cases):] + cases[:pass_index % len(cases)]
            for contender, case in turned:
                print(f"{time.strftime('%H:%M:%S')} {round_name} {contender} pass {pass_index}", flush=True)
                line = run_case(round_name, contender, case, pass_index)
                if "error" in line:
                    failed += 1
                    print(f"  failed: {line['error'][-800:]}", flush=True)
                with open(OUT / f"{round_name}.jsonl", "a") as out:
                    out.write(json.dumps(line) + "\n")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
