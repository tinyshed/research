"""Round 1z: stop the round cleanly and free the machine.

    python -I pause.py <lock folder>

Writes corpus/round-1z/stop, which local.py reads before each prompt and
net.py before a fit and every hundred steps of one: the request in flight is
let finish (twenty minutes at most, then its process is ended), a fit in
flight is dropped and starts again later. Then every model is unloaded, the
round's Ollama server is stopped, the round's locks are removed and one line
goes to corpus/round-1z/paused.txt. Nothing is started again from here:
the round resumes by removing `stop`, starting the server and running the
job script again, which asks only what has no record.
"""

import json
import sys
import time
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

import psutil

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "corpus" / "round-1z"
HOST = "http://127.0.0.1:11435"
SETS = {"style3-db": 100, "plain-a-db": 718, "db": 398, "heldout-db": 22}


def ours(*needles) -> list:
    """The round's processes: one whose program lies under the round's runtime folder, or one
    with an argument (a path, not a whole command) that ends in a needle. Never this process
    or one it descends from."""
    me = psutil.Process()
    skip = {me.pid, *(p.pid for p in me.parents())}
    out = []
    for p in psutil.process_iter(["pid", "name", "cmdline", "exe"]):
        try:
            if p.pid in skip:
                continue
            args = [a.replace("\\", "/") for a in (p.info["cmdline"] or []) if " " not in a]
            exe = (p.info["exe"] or "").replace("\\", "/")
            if any(a.endswith(n) for a in args for n in needles) or any(n in exe for n in needles if n.startswith("round-1z/")):
                out.append(p)
        except Exception:  # noqa: BLE001
            pass
    return out


def main(locks: Path) -> None:
    t0 = time.time()
    running = [" ".join(p.info["cmdline"][-5:]) for p in ours("round-1z/local.py", "round-1z/net.py") if "python" in (p.info["name"] or "").lower()]
    (OUT / "stop").write_text(datetime.now(timezone.utc).isoformat(timespec="seconds") + "\n", encoding="utf-8")
    while ours("round-1z/local.py", "round-1z/net.py", "logs/job2.sh") and time.time() - t0 < 1200:
        time.sleep(5)
    killed = []
    for p in ours("round-1z/local.py", "round-1z/net.py", "logs/job2.sh", "round-1z/with-lock.sh"):
        killed.append(p.pid)
        p.kill()
    try:
        req = urllib.request.Request(HOST + "/api/ps")
        for m in json.loads(urllib.request.urlopen(req, timeout=20).read()).get("models", []):
            body = json.dumps({"model": m["name"], "messages": [], "keep_alive": 0}).encode()
            urllib.request.urlopen(urllib.request.Request(HOST + "/api/chat", data=body,
                                                          headers={"Content-Type": "application/json"}), timeout=120).read()
    except Exception as e:  # noqa: BLE001
        print("unload:", repr(e)[:200])
    time.sleep(3)
    for p in ours("round-1z/runtime/ollama"):
        try:
            p.kill()
        except Exception:  # noqa: BLE001
            pass
    for name in ("gpu.lock", "pairs.lock"):
        f = locks / name
        if f.exists() and f.read_text(encoding="utf-8", errors="replace").startswith("round-1z"):
            f.unlink()
    asked = []
    for d in sorted((OUT / "llm").glob("*/*")):
        if d.name in SETS:
            asked.append(f"{d.parent.name} {d.name} {len(list(d.glob('db-*.json')))}/{SETS[d.name]}")
    nets = [f"{d.name} {'fitted' if (d / 'fit.json').exists() else 'not fitted'}" for d in sorted((OUT / "nets").glob("*"))]
    line = (f"{datetime.now(timezone.utc).isoformat(timespec='seconds')} paused; running: {running or 'nothing'}; "
            f"ended by force: {killed or 'none'}; asked: {'; '.join(asked)}; networks: {'; '.join(nets) or 'none fitted'}; "
            f"resume: rm corpus/round-1z/stop && (sh corpus/round-1z/logs/serve.sh &) && sh corpus/round-1z/logs/job2.sh "
            f">> corpus/round-1z/logs/job2.log 2>&1\n")
    with open(OUT / "paused.txt", "a", encoding="utf-8") as f:
        f.write(line)
    print(line)


if __name__ == "__main__":
    main(Path(sys.argv[1]))
