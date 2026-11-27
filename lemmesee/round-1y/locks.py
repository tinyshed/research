"""Round 1y: the session's lock files, one pair build and one graphics-card
fit at a time across the rounds that share the machine.

The folder is given in LEMMESEE_LOCKS; without it nothing is locked.
"""

import os
import time
from contextlib import contextmanager
from pathlib import Path

NAME = "round-1y"


@contextmanager
def held(which: str, what: str = ""):
    folder = os.environ.get("LEMMESEE_LOCKS")
    if not folder:
        yield
        return
    path = Path(folder) / f"{which}.lock"
    path.parent.mkdir(parents=True, exist_ok=True)
    waited = time.time()
    while True:
        try:
            fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            break
        except FileExistsError:
            time.sleep(0.02)
    os.write(fd, f"{NAME} {what}\n".encode())
    os.close(fd)
    if time.time() - waited > 6:
        print(f"waited {time.time() - waited:.0f} s for {which}.lock", flush=True)
    try:
        yield
    finally:
        path.unlink(missing_ok=True)
