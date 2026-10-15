#!/usr/bin/env python3
"""Run the already built harness, without other build/test work in the interval."""
import json
from pathlib import Path
import resource
import subprocess
import sys
import time

started_utc = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
started = time.monotonic()
mode = sys.argv[2] if len(sys.argv) > 2 else "bench"
result = subprocess.run(["target/release/tinystore-replication-spike", mode])
usage = resource.getrusage(resource.RUSAGE_CHILDREN)
Path(sys.argv[1]).write_text(json.dumps({
    "started_utc": started_utc,
    "ended_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "wall_seconds": time.monotonic() - started,
    "user_seconds": usage.ru_utime, "system_seconds": usage.ru_stime,
    "max_rss_kib": usage.ru_maxrss, "exit_code": result.returncode,
}, indent=2) + "\n")
raise SystemExit(result.returncode)
