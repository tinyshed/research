"""Measure each small consumer in a separate process and fresh directory."""

import argparse
import json
import os
import pathlib
import selectors
import shutil
import subprocess
import tempfile
import time

CASES = ["empty", "root", "kv", "sqldb", "jobs", "blobs", "records", "metrics", "all"]


def memory(pid):
    result = {}
    for file in ("status", "smaps_rollup"):
        for line in pathlib.Path(f"/proc/{pid}/{file}").read_text().splitlines():
            fields = line.split()
            if len(fields) == 3 and fields[2] == "kB":
                result[fields[0].rstrip(":")] = int(fields[1]) * 1024
    return result


def one(binary, directory):
    with subprocess.Popen([str(binary), directory], stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) as child:
        samples = []
        try:
            for command in (None, "o", "g", "r"):
                if command is not None:
                    child.stdin.write(command)
                    child.stdin.flush()
                with selectors.DefaultSelector() as ready:
                    ready.register(child.stdout, selectors.EVENT_READ)
                    if not ready.select(timeout=30):
                        raise TimeoutError("consumer did not report its memory phase")
                    line = child.stdout.readline()
                if not line:
                    raise RuntimeError(child.stderr.read())
                sample = json.loads(line)
                time.sleep(0.25)
                sample["process"] = memory(child.pid)
                samples.append(sample)
            child.stdin.write("q")
            child.stdin.flush()
            child.wait(timeout=10)
            if child.returncode:
                raise RuntimeError(child.stderr.read())
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()
    return samples


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", type=pathlib.Path, required=True)
    parser.add_argument("--data", required=True)
    parser.add_argument("--out", type=pathlib.Path, required=True)
    parser.add_argument("--repeats", type=int, default=5)
    args = parser.parse_args()
    result = {"source": os.environ["TINYSTORE_COMMIT"], "harness": os.environ["HARNESS_COMMIT"], "runs": []}
    for repeat in range(1, args.repeats + 1):
        for case in CASES if repeat % 2 else reversed(CASES):
            directory = tempfile.mkdtemp(prefix="idle-", dir=args.data)
            try:
                samples = one(args.bin / case, directory)
                result["runs"].append({"case": case, "repeat": repeat, "samples": samples})
            finally:
                shutil.rmtree(directory)
            print(f"{repeat} {case}", flush=True)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
