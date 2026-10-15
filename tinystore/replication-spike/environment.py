#!/usr/bin/env python3
"""Capture non-secret runtime facts and the exact harness input hashes."""
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def read(path):
    return Path(path).read_text().strip()


root = Path(__file__).resolve().parent
files = sorted([*root.glob("src/*.rs"), root / "Cargo.toml", root / "Cargo.lock",
                root / ".cargo/config.toml", root / "source-revision"])
hashes = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
cpu = next(line.split(":", 1)[1].strip() for line in read("/proc/cpuinfo").splitlines()
           if line.startswith("model name"))
print(json.dumps({
    "recorded_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "kernel": platform.release(), "architecture": platform.machine(), "cpu": cpu,
    "visible_cpus": os.cpu_count(), "cpu_max": read("/sys/fs/cgroup/cpu.max"),
    "memory_max": read("/sys/fs/cgroup/memory.max"),
    "rustc": command("rustc", "--version"), "cargo": command("cargo", "--version"),
    "source_commit": command("git", "-C", str(root / "source"), "rev-parse", "HEAD"),
    "research_base_commit": command("git", "-C", str(root), "rev-parse", "HEAD"),
    "harness_status": "uncommitted interim experiment; content hashes identify this revision",
    "harness_sha256": hashlib.sha256(json.dumps(hashes, sort_keys=True).encode()).hexdigest(),
    "files": hashes,
    "stores_filesystem": command("stat", "-f", "-c", "%T", os.getenv("TMPDIR", "/tmp")),
    "stores_location": "<tmp>",
}, indent=2))
