"""The kv round from Python: TinyStore through its SDK, or Redis through
redis-py's asyncio client, with the Go round's load.

    python kv.py <tinystore|tinystore-server|redis|redis-tcp> <dir> <seconds>

It prints one run as the Go harness writes it. TINYSTORE_BIN is the binary the
SDK starts its sidecar with; TINYSTORE_URL and TINYSTORE_TOKEN the remote
server sdk.sh started; REDIS_URL the Redis it started, and SERVICE_PID the
server's process for its memory.
"""

import asyncio
import json
import os
import random
import re
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "../../../source/sdk/python/src"))

import tinystore  # noqa: E402

KEYS = 100_000
VALUE_BYTES = 128
contender, directory, seconds = sys.argv[1], sys.argv[2], float(sys.argv[3])


def value(i: int, version: int) -> bytes:
    x = (i * 0x9E3779B97F4A7C15 + version) & 0xFFFFFFFFFFFFFFFF
    out = bytearray(VALUE_BYTES)
    for j in range(VALUE_BYTES):
        x = (x * 6364136223846793005 + 1442695040888963407) & 0xFFFFFFFFFFFFFFFF
        out[j] = x >> 56
    return bytes(out)


# values made once, so that making one costs no set its time
VALUES = [value(i, 0) for i in range(1024)]


def key(i: int) -> str:
    return f"session:{i}"


def pick() -> int:
    """a key; random, not a small LCG, whose low bit alternates so that a mixed
    call drawing twice would always decide on the same parity"""
    return random.randrange(KEYS)


class TinyStoreKv:
    def __init__(self, store, service_pid):
        self.store, self.bucket, self.service_pid = store, store.kv.bucket("sessions", bytes), service_pid

    async def get(self, k):
        return await self.bucket.get(k)

    async def set(self, k, v):
        await self.bucket.set(k, v)

    async def close(self):
        await self.store.close()


class RedisKv:
    def __init__(self, client):
        self.client, self.service_pid = client, int(os.environ["SERVICE_PID"])

    async def get(self, k):
        return await self.client.get(k)

    async def set(self, k, v):
        await self.client.set(k, v)

    async def close(self):
        await self.client.aclose()


async def open_kv():
    if contender == "tinystore":
        store = await tinystore.open(directory, idle=0)
        with open(os.path.join(directory, "server", "SERVE")) as f:
            pid = json.load(f)["pid"]
        return TinyStoreKv(store, pid)
    if contender == "tinystore-server":
        store = await tinystore.connect(os.environ["TINYSTORE_URL"], token=os.environ["TINYSTORE_TOKEN"])
        return TinyStoreKv(store, int(os.environ["SERVICE_PID"]))
    if contender in ("redis", "redis-tcp"):
        import redis.asyncio

        return RedisKv(redis.asyncio.Redis.from_url(os.environ["REDIS_URL"], max_connections=64))
    raise SystemExit(f"no contender {contender}")


async def time_stage(name, workers, op):
    warm = time.monotonic() + 1

    async def warming():
        while time.monotonic() < warm:
            await op()

    await asyncio.gather(*(warming() for _ in range(workers)))
    latencies, errors, first = [], 0, None
    spent = cpu("self") + cpu(SERVICE[0])
    began = time.perf_counter()
    end = began + seconds

    async def worker():
        nonlocal errors, first
        while True:
            start = time.perf_counter()
            if start >= end:
                return
            try:
                await op()
            except Exception as err:  # noqa: BLE001 - counted, as the Go harness counts
                errors += 1
                first = first or repr(err)
            latencies.append((time.perf_counter() - start) * 1e6)

    await asyncio.gather(*(worker() for _ in range(workers)))
    elapsed = time.perf_counter() - began
    spent = cpu("self") + cpu(SERVICE[0]) - spent
    latencies.sort()

    def at(q):
        return latencies[min(len(latencies) - 1, int(q * len(latencies)))] if latencies else 0

    stage = {"name": name, "goroutines": workers, "ops": len(latencies), "errors": errors, "seconds": elapsed,
             "per_second": len(latencies) / elapsed, "p50_us": at(0.5), "p99_us": at(0.99), "cpu_seconds": spent}
    if first:
        stage["first_error"] = first
    return stage


def cpu(pid) -> float:
    """utime and stime of a process, in seconds; its children are not counted"""
    try:
        with open(f"/proc/{pid}/stat") as f:
            fields = f.read().rsplit(")", 1)[1].split()
        return (int(fields[11]) + int(fields[12])) / os.sysconf("SC_CLK_TCK")
    except (OSError, ValueError):
        return 0.0


SERVICE = [0]


def peak(pid) -> int:
    try:
        with open(f"/proc/{pid}/status") as f:
            found = re.search(r"VmHWM:\s+(\d+) kB", f.read())
        return int(found.group(1)) * 1024 if found else 0
    except OSError:
        return 0


async def main():
    opened = time.perf_counter()
    kv = await open_kv()
    open_seconds = time.perf_counter() - opened
    SERVICE[0] = kv.service_pid

    async def fill(w):
        for i in range(w, KEYS, 64):
            await kv.set(key(i), VALUES[i % 1024])

    await asyncio.gather(*(fill(w) for w in range(64)))

    version = 1

    async def get():
        i = pick()
        got = await kv.get(key(i))
        if got is None or len(got) != VALUE_BYTES:
            raise ValueError(f"{key(i)}: {got and len(got)} bytes")

    async def put():
        nonlocal version
        i = pick()
        version += 1
        await kv.set(key(i), VALUES[(i + version) % 1024])

    async def mixed():
        await (put() if random.random() < 0.1 else get())

    stages = []
    for name, op in (("get", get), ("set", put), ("mixed", mixed)):
        for workers in (1, 8, 64):
            stages.append(await time_stage(name, workers, op))
    service_peak = peak(kv.service_pid)
    await kv.close()
    print(json.dumps({"contender": f"python-{contender}", "stages": stages, "open_rss_bytes": 0,
                      "peak_rss_bytes": peak("self"), "service_peak_rss_bytes": service_peak, "disk_bytes": 0,
                      "open_seconds": open_seconds, "processes": 2}))


asyncio.run(main())
