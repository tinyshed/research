#!/usr/bin/env python3
"""The slice round's passes: every case of every program, a process and a fresh
store each, the programs' order turned round on every other pass.

Nothing else may build, test or measure on the machine meanwhile.
"""
import datetime
import json
import os
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path

WORK = Path("/perf/slice")
OUT = Path(os.environ.get("OUT", Path(__file__).resolve().parent.parent / "reports/data/rust-slice-2026-10-10"))
PASSES = int(os.environ.get("PASSES", "3"))
SECONDS = os.environ.get("SECONDS_A_CASE", "3")

# case, callers
NATIVE = [
    ("kv-get", 1), ("kv-get", 64), ("kv-set", 1), ("kv-set", 64),
    ("sql-point", 1), ("sql-point", 64), ("sql-insert", 1), ("sql-insert", 64), ("sql-rows", 1),
    ("jobs-add", 1), ("jobs-add", 64), ("jobs-drain", 8),
]
BUN = [("kv-get", 1), ("kv-get", 64), ("kv-set", 64), ("sql-point", 64), ("sql-rows", 1), ("jobs-add", 64)]


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")


def native(binary):
    return lambda case, callers, store: [
        str(WORK / "bin" / binary), "--dir", store, "--case", case, "--callers", str(callers), "--seconds", SECONDS]


def bun(flavor, mode):
    source = "rust-src" if flavor == "rust" else "go-src"
    return lambda case, callers, store: [
        "/perf/bin/bun", str(Path(__file__).resolve().parent / "bun/bench.ts"),
        "--sdk", str(WORK / source / "sdk/js/src/index.ts"), "--flavor", flavor, "--mode", mode,
        "--binary", str(WORK / "bin" / f"tinystore-{flavor}"), "--library", str(WORK / "bin/libtinystore_ffi.so"),
        "--dir", store, "--case", case, "--callers", str(callers), "--seconds", SECONDS]


PROGRAMS = {
    "go": (native("go-slice"), NATIVE),
    "rust": (native("rust-slice"), NATIVE),
    "bun-go-sidecar": (bun("go", "sidecar"), BUN),
    "bun-rust-sidecar": (bun("rust", "sidecar"), BUN),
    "bun-rust-embedded": (bun("rust", "embedded"), BUN),
}
GROUPS = [["go", "rust"], ["bun-go-sidecar", "bun-rust-sidecar", "bun-rust-embedded"]]

# ALSO names trees built the same way, "label=/perf/dir,...": another commit
# of the Rust core, or the same one with other flags. Each one's programs run
# beside the round's own, under the same names with -label. NEXT=/perf/dir is
# ALSO=next=/perf/dir.
ALSO = dict(each.split("=", 1) for each in os.environ.get("ALSO", "").split(",") if each)
if os.environ.get("NEXT"):
    ALSO["next"] = os.environ["NEXT"]


def also_native(tree):
    return lambda case, callers, store: [
        str(tree / "bin/rust-slice"), "--dir", store, "--case", case, "--callers", str(callers), "--seconds", SECONDS]


def also_bun(tree, mode):
    return lambda case, callers, store: [
        "/perf/bin/bun", str(Path(__file__).resolve().parent / "bun/bench.ts"),
        "--sdk", str(tree / "rust-src/sdk/js/src/index.ts"), "--flavor", "rust", "--mode", mode,
        "--binary", str(tree / "bin/tinystore-rust"), "--library", str(tree / "bin/libtinystore_ffi.so"),
        "--dir", store, "--case", case, "--callers", str(callers), "--seconds", SECONDS]


for label, tree in ALSO.items():
    PROGRAMS[f"rust-{label}"] = (also_native(Path(tree)), NATIVE)
    PROGRAMS[f"bun-rust-sidecar-{label}"] = (also_bun(Path(tree), "sidecar"), BUN)
    PROGRAMS[f"bun-rust-embedded-{label}"] = (also_bun(Path(tree), "embedded"), BUN)

# A follow-up on the round's own programs: CASES="kv-get:4,kv-get:8" for the
# programs of ONLY, written as RUNS beside the round's runs.
if os.environ.get("CASES"):
    asked = [(case, int(callers)) for case, callers in (each.split(":") for each in os.environ["CASES"].split(","))]
    GROUPS = [os.environ.get("ONLY", "go,rust").split(",")]
    PROGRAMS = {name: (PROGRAMS[name][0], asked) for name in GROUPS[0]}
RUNS = os.environ.get("RUNS", "runs")


def invoke(name, case, callers):
    """One case in a process of its own, with what the process took."""
    store = WORK / "run-store"
    if store.exists():
        shutil.rmtree(store)
    store.mkdir()
    command = PROGRAMS[name][0](case, callers, str(store))
    started = utc()
    # to files, so that wait4 can reap the process and say what it took
    with (WORK / "stdout").open("w") as out, (WORK / "stderr").open("w") as err:
        process = subprocess.Popen(command, stdout=out, stderr=err)
        _, status, usage = os.wait4(process.pid, 0)
    out, err = (WORK / "stdout").read_text(), (WORK / "stderr").read_text()
    if os.waitstatus_to_exitcode(status):
        raise RuntimeError(f"{command}\n{out}\n{err}")
    # a sidecar outlives its client by a second: wait until it let go of the store
    deadline = time.monotonic() + 15
    while (store / "server" / "SERVE").exists() and time.monotonic() < deadline:
        time.sleep(0.05)
    row = json.loads(out.strip().splitlines()[-1])
    row.update(program=name, started_utc=started, finished_utc=utc(),
               process={"max_rss_kib": usage.ru_maxrss, "user_s": usage.ru_utime, "system_s": usage.ru_stime},
               command=[part.replace(str(WORK), "<work>") for part in command])
    return row


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    read = lambda name: (WORK / name).read_text().strip()
    environment = {
        "started_utc": utc(),
        "tinystore_rust_commit": read("rust-commit"), "tinystore_go_commit": read("go-commit"),
        "rustc": read("rustc-version"), "go": read("go-version"),
        "bun": subprocess.check_output(["/perf/bin/bun", "--version"], text=True).strip(),
        "machine": subprocess.check_output(["sh", "-c", "uname -a; lscpu"], text=True),
        "container_os": platform.platform(), "python": platform.python_version(),
        "store_volume": "a Docker volume on WSL2's ext4, not the bind mount",
        "passes": PASSES, "seconds_a_case": SECONDS,
        "order": "each pass runs a group's programs in turn, case by case; every other pass turns their order round",
    }
    kept = lambda tree, name: (Path(tree) / name).read_text().strip() if (Path(tree) / name).exists() else None
    environment["sqlite_flags"] = kept(WORK, "sqlite-flags") or "SQLITE_DQS=0 -DHAVE_FDATASYNC=1"
    environment["also"] = {
        label: {"tinystore_rust_commit": kept(tree, "rust-commit"),
                "sqlite_flags": kept(tree, "sqlite-flags") or "SQLITE_DQS=0 -DHAVE_FDATASYNC=1"}
        for label, tree in ALSO.items()}
    rows = 0
    with (OUT / f"{RUNS}.jsonl").open("w") as raw:
        for number in range(PASSES):
            for group in GROUPS:
                order = group if number % 2 == 0 else list(reversed(group))
                for case, callers in PROGRAMS[group[0]][1]:
                    for name in order:
                        if (case, callers) not in PROGRAMS[name][1]:
                            continue
                        row = invoke(name, case, callers)
                        row["pass"] = number
                        raw.write(json.dumps(row) + "\n")
                        raw.flush()
                        rows += 1
                        print(json.dumps({k: row[k] for k in ("program", "case", "callers", "per_second")}), flush=True)
            print(json.dumps({"pass_finished": number, "utc": utc(), "rows": rows}), flush=True)
    environment.update(finished_utc=utc(), rows=rows)
    name = "environment.json" if RUNS == "runs" else f"environment-{RUNS}.json"
    (OUT / name).write_text(json.dumps(environment, indent=2) + "\n")


if __name__ == "__main__":
    sys.exit(main())
