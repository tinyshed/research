#!/usr/bin/env python3
"""Sequential prebuilt processes. No builds, test runners or lint during timing."""
import argparse
import datetime as dt
import json
import os
from pathlib import Path
import subprocess as sp
import uuid

HERE = Path(__file__).resolve().parent
CONTAINER = os.environ.get("BENCH_CONTAINER", "channels-locks-lab")
BIN = "/target/release/channels-locks-bench"


def stamp():
    return dt.datetime.now(dt.timezone.utc).isoformat()


def invoke(args, db):
    return ["docker", "exec", "-e", f"BENCH_DB={db}", CONTAINER, BIN, *map(str, args)]


def clean(db):
    sp.run(["docker", "exec", CONTAINER, "rm", "-f", db, db + "-wal", db + "-shm"], check=True)


def run_case(args, output, label):
    db = f"/data/{uuid.uuid4().hex}.db"
    begin = stamp()
    result = sp.run(invoke(args, db), capture_output=True, text=True, timeout=180)
    record = {"label": label, "args": args, "start_utc": begin, "end_utc": stamp(), "exit": result.returncode}
    if result.returncode:
        record.update(stdout=result.stdout, stderr=result.stderr)
    else:
        record["result"] = json.loads(result.stdout)
    with output.open("a") as stream:
        stream.write(json.dumps(record, ensure_ascii=False) + "\n")
    clean(db)
    print(label, args, result.returncode, flush=True)
    if result.returncode:
        raise RuntimeError(result.stderr)


def kill_checks(output):
    for stage in ("uncommitted", "committed"):
        for iteration in range(3):
            db = f"/data/{uuid.uuid4().hex}.db"
            begin = stamp()
            p = sp.Popen(invoke(["lease-owner", stage], db), stdout=sp.PIPE, stderr=sp.PIPE, text=True)
            ready = json.loads(p.stdout.readline())
            assert ready["ready"]
            # Kill the actual binary inside the container; killing docker exec leaves it running.
            sp.run(["docker", "exec", CONTAINER, "pkill", "-9", "-f", f"^{BIN} lease-owner {stage}$"], check=True)
            p.wait(timeout=10)
            result = sp.run(invoke(["lease-recover", stage], db), text=True, capture_output=True, check=True)
            record = {"label": f"kill-{stage}-{iteration}", "start_utc": begin, "end_utc": stamp(),
                      "owner_exit": p.returncode, "result": json.loads(result.stdout)}
            with output.open("a") as stream:
                stream.write(json.dumps(record) + "\n")
            clean(db)
            print(record["label"], "pass", flush=True)


def cases(suite):
    if suite == "preflight":
        yield ["channel-check"]
        yield ["lease-check"]
        yield ["commit-check"]
        for k in ("broadcast", "ring", "queues"):
            yield ["channels", k, 10, 2000, 64, "saturate", 1024]
        for k in ("kv-poll", "sql-poll", "hybrid", "admitted"):
            yield ["leases", k, 10, 1, "hot", 1, "FULL", 64]
        for k in ("plain", "fts", "rtree"):
            yield ["indexes", k, 10000]
    elif suite == "channels":
        for repeat in range(3):
            kinds = ["broadcast", "ring", "queues"]
            kinds = kinds[repeat:] + kinds[:repeat]
            for n in (1, 100, 1000):
                for mode in ("saturate", "slow", "paced"):
                    for k in kinds:
                        yield ["channels", k, n, 3000 if mode == "paced" else (200000 if n == 1 else 20000), 64, mode, 1024]
    elif suite == "resources":
        for repeat in range(2):
            kinds = ["broadcast", "ring", "queues"][::1 if repeat == 0 else -1]
            for n in (1, 100, 1000):
                for size in (64, 65536):
                    for k in kinds:
                        yield ["channels", k, n, 4096, size, "paused", 1024]
            for n in (1000, 10000, 100000):
                for k in ("scan", "trie")[::1 if repeat == 0 else -1]:
                    yield ["route", k, n, 10000 if k == "scan" else 1000000]
    elif suite == "leases":
        kinds = ["kv-poll", "sql-poll", "hybrid", "admitted"]
        for repeat in range(3):
            order = kinds[repeat:] + kinds[:repeat]
            for n in (10, 100, 1000):
                for topology in ("hot", "spread"):
                    for k in order:
                        yield ["leases", k, n, 4, topology, 1, "FULL", 64]
    elif suite == "lease-controls":
        for repeat in range(2):
            for permits in (1, 8):
                for batch in (1, 64):
                    for k in ("sql-poll", "admitted")[::1 if repeat == 0 else -1]:
                        yield ["leases", k, 100, 4, "hot", permits, "FULL", batch]
            for durability in ("FULL", "NORMAL"):
                for k in ("sql-poll", "admitted"):
                    yield ["leases", k, 100, 4, "spread", 1, durability, 64]
            for k in ("sql-poll", "admitted"):
                yield ["leases", k, 100, 4, "spread", 1, "FULL", 64, "renew"]
    elif suite == "indexes":
        for repeat in range(3):
            kinds = ["plain", "fts", "rtree"]
            for k in kinds[repeat:] + kinds[:repeat]:
                yield ["indexes", k, 100000]
    else:
        raise ValueError(suite)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("suite", choices=["preflight", "channels", "resources", "leases", "lease-controls", "indexes", "kill"])
    parser.add_argument("output", type=Path)
    a = parser.parse_args()
    assert not a.output.exists(), "never overwrite evidence"
    a.output.parent.mkdir(parents=True, exist_ok=True)
    if a.suite == "kill":
        kill_checks(a.output)
    else:
        for i, args in enumerate(cases(a.suite)):
            run_case(args, a.output, f"{a.suite}-{i:03}")
