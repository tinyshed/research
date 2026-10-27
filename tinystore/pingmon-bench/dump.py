"""Prints every sample of the Pingmon store at argv[1] as `name,target,ms,value`,
the value as Python's repr, read through its running server."""
import asyncio
import sys
from datetime import datetime, timezone

import tinystore

START = datetime(2026, 10, 26, tzinfo=timezone.utc)


async def main() -> None:
    end = datetime.now(timezone.utc).replace(second=0, microsecond=0)
    print(f"# to {end.isoformat()}", file=sys.stderr)
    store = await tinystore.open(sys.argv[1])
    try:
        for name in ("ping_rtt_ms", "ping_lost"):
            page = await store.metrics.read(name=name, from_=START, to=end, limit=100_000)
            async for series in page:
                target = series.labels["target"]
                for at, value in zip(series.times, series.values):
                    sys.stdout.write(f"{name},{target},{int(at)},{value!r}\n")
    finally:
        await store.close()


asyncio.run(main())
