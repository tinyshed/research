#!/usr/bin/env python3
"""What a store holds in memory: each program's process, and its server's
beside Bun, before the store opens, once it is open, after a case, and after a
rest with the store still open. The programs read it from
/proc/<pid>/smaps_rollup themselves, given --rest.

The cases load one at a time, and a program rests while the next ones load:
a rest is idle, and what a process holds is its own. Nothing else may build,
test or measure meanwhile.

  TREE=/perf/slice-mem OUT=<dir> REST=130 RUNS=memory-a python3 memory.py

TREE is a tree build.sh built, both sides of it. FIRST=1 starts at the second
pass's order, the programs' order turned round.
"""
import datetime
import json
import os
import platform
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

TREE = Path(os.environ.get("TREE", "/perf/slice"))
OUT = Path(os.environ["OUT"])
RUNS = os.environ.get("RUNS", "memory")
FIRST = int(os.environ.get("FIRST", "0"))
PASSES = int(os.environ.get("PASSES", "1"))
REST = os.environ.get("REST", "130")
SECONDS = os.environ.get("SECONDS_A_CASE", "3")
CASES = [("kv-get", 64), ("sql-point", 64), ("sql-rows", 1), ("jobs-add", 64)]
if os.environ.get("CASES"):
    CASES = [(case, int(callers)) for case, callers in (each.split(":") for each in os.environ["CASES"].split(","))]
HERE = Path(__file__).resolve().parent


def native(binary):
    return lambda case, callers, store: [
        str(TREE / "bin" / binary), "--dir", store, "--case", case, "--callers", str(callers), "--seconds", SECONDS]


def bun(flavor, mode):
    source = "rust-src" if flavor == "rust" else "go-src"
    return lambda case, callers, store: [
        "/perf/bin/bun", str(HERE / "bun/bench.ts"),
        "--sdk", str(TREE / source / "sdk/js/src/index.ts"), "--flavor", flavor, "--mode", mode,
        "--binary", str(TREE / "bin" / f"tinystore-{flavor}"), "--library", str(TREE / "bin/libtinystore_ffi.so"),
        "--dir", store, "--case", case, "--callers", str(callers), "--seconds", SECONDS]


PROGRAMS = {
    "rust": native("rust-slice"),
    "go": native("go-slice"),
    "bun-rust-embedded": bun("rust", "embedded"),
    "bun-rust-sidecar": bun("rust", "sidecar"),
    "bun-go-sidecar": bun("go", "sidecar"),
}


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")


class Running:
    """A program started on a case, its stderr read as it comes."""

    def __init__(self, name, case, callers, number):
        self.name, self.case, self.callers = name, case, callers
        self.store = TREE / f"mem-{number}-{name}-{case}-{callers}"
        shutil.rmtree(self.store, ignore_errors=True)
        self.store.mkdir(parents=True)
        self.command = PROGRAMS[name](case, callers, str(self.store)) + ["--rest", REST]
        self.started = utc()
        self.process = subprocess.Popen(self.command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.loaded = threading.Event()
        self.errors = []
        threading.Thread(target=self.read_errors, daemon=True).start()

    def read_errors(self):
        for line in self.process.stderr:
            if line.strip() == "loaded":
                self.loaded.set()
            else:
                self.errors.append(line)
        # ended without loading: row() says why
        self.loaded.set()

    def row(self):
        out = self.process.stdout.read()
        if self.process.wait():
            raise RuntimeError(f"{self.command}\n{out}\n{''.join(self.errors)}")
        # a sidecar outlives its client by a second: wait until it let go of the store
        deadline = time.monotonic() + 15
        while (self.store / "server" / "SERVE").exists() and time.monotonic() < deadline:
            time.sleep(0.05)
        shutil.rmtree(self.store, ignore_errors=True)
        row = json.loads(out.strip().splitlines()[-1])
        row.update(program=self.name, started_utc=self.started, finished_utc=utc(), rest_s=float(REST),
                   command=[part.replace(str(TREE), "<tree>") for part in self.command])
        return row


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    read = lambda name: (TREE / name).read_text().strip()
    environment = {
        "started_utc": utc(),
        "tree": {"tinystore_rust_commit": read("rust-commit"), "tinystore_go_commit": read("go-commit"),
                 "sqlite_flags": read("sqlite-flags")},
        "rustc": read("rustc-version"), "go": read("go-version"),
        "bun": subprocess.check_output(["/perf/bin/bun", "--version"], text=True).strip(),
        "machine": subprocess.check_output(["sh", "-c", "uname -a; lscpu"], text=True),
        "container_os": platform.platform(), "python": platform.python_version(),
        "store_volume": "a Docker volume on WSL2's ext4, not the bind mount",
        "passes": list(range(FIRST, FIRST + PASSES)), "rest_s": float(REST), "seconds_a_case": SECONDS,
        "cases": [f"{case}:{callers}" for case, callers in CASES],
        "order": "each case's programs in turn, one loading at a time while the ones before rest; "
                 "every other pass turns their order round",
    }
    rows = 0
    with (OUT / f"{RUNS}.jsonl").open("w") as raw:
        for number in range(FIRST, FIRST + PASSES):
            order = list(PROGRAMS) if number % 2 == 0 else list(reversed(PROGRAMS))
            running = []
            for case, callers in CASES:
                for name in order:
                    one = Running(name, case, callers, number)
                    if not one.loaded.wait(600) or one.process.poll() is not None:
                        raise RuntimeError(f"{one.command} never rested\n{''.join(one.errors)}")
                    running.append(one)
                    print(json.dumps({"loaded": name, "case": case, "callers": callers, "utc": utc()}), flush=True)
            for one in running:
                row = one.row()
                row["pass"] = number
                raw.write(json.dumps(row) + "\n")
                raw.flush()
                rows += 1
            print(json.dumps({"pass_finished": number, "utc": utc(), "rows": rows}), flush=True)
    environment.update(finished_utc=utc(), rows=rows)
    (OUT / f"environment-{RUNS}.json").write_text(json.dumps(environment, indent=2) + "\n")


if __name__ == "__main__":
    sys.exit(main())
