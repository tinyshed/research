"""Chapter 14's measurement of what looking costs: every PostgreSQL store of
lemmesee's corpus with a table over 10 000 rows, looked at by `see` as the
rounds read (`--unbounded`) and as it reads by default, one after the other
on each store, each look traced and measured by `cost.py`.

    python looks.py <see.exe> <out-dir> [--passes 2] [store ...]

Each look writes `<out>/<pass>-<a|b>/<store>.{jsonl,doc.json,cost.json}` and
a line to standard output. Run it on a quiet machine: nothing built meanwhile.
"""

import argparse
import glob
import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
CORPUS = os.path.normpath(os.path.join(HERE, "..", "..", "lemmesee", "corpus", "see"))


def stores():
    for f in sorted(glob.glob(os.path.join(CORPUS, "*", "snapshot.json"))):
        s = json.load(open(f, encoding="utf-8"))
        if s["engine"] == "postgresql" and any(((t.get("features", {}).get("rows_estimate") or {}).get("value") or 0) > 10_000 for t in s["tables"]):
            yield os.path.basename(os.path.dirname(f)), s["database"], f


def look(binary, name, db, snap, out, bounded):
    os.makedirs(out, exist_ok=True)
    trace = os.path.join(out, f"{name}.jsonl")
    if os.path.exists(trace):
        os.remove(trace)
    args = [binary, snap, "--docker", "lemmesee-round-1-postgres-1", "--user", "lemmesee", "--name", name, "--doc", os.path.join(out, f"{name}.doc.json"), "--doc-only"]
    if not bounded:
        args.append("--unbounded")
    started = time.time()
    p = subprocess.run(args, env=dict(os.environ, SEE_TRACE=trace, NO_COLOR="1"), capture_output=True)
    took = time.time() - started
    c = subprocess.run([sys.executable, "-I", os.path.join(HERE, "cost.py"), trace, "--database", db, "--snapshot", snap, "--out", os.path.join(out, f"{name}.cost.json")], capture_output=True)
    summary = json.loads(c.stdout) if c.returncode == 0 else {"error": c.stderr.decode("utf-8", "replace")[-300:]}
    return {"store": name, "bounded": bounded, "look_s": round(took, 2), "exit": p.returncode, **summary}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("see")
    ap.add_argument("out")
    ap.add_argument("--passes", type=int, default=2)
    ap.add_argument("only", nargs="*")
    a = ap.parse_args()
    for n in range(1, a.passes + 1):
        for name, db, snap in stores():
            if a.only and name not in a.only:
                continue
            # A then B on the same store, so both meet the same caches and the same machine
            for bounded in (False, True):
                r = look(a.see, name, db, snap, os.path.join(a.out, f"{n}-{'b' if bounded else 'a'}"), bounded)
                print(json.dumps({"pass": n, "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), **r}), flush=True)


if __name__ == "__main__":
    main()
