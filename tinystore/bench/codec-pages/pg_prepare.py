"""Write a PostgreSQL COPY file (text format, bytea as hex) for each bodies file.

    python3 -I pg_prepare.py <out dir> <bodies file> ...

`<out dir>/<dataset>__<codec>__L<L>.tsv`, the dataset taken from the bodies
file's folder: one line a block, `id<TAB>\\x<hex body>`, ids 1..N in block
order, as store.py numbers them for SQLite.
"""

import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cpfiles import read_bodies  # noqa: E402


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for path in sys.argv[2:]:
        ds = os.path.basename(os.path.dirname(path))
        m = re.match(r"(.+)\.L(\d+)\.bodies$", os.path.basename(path))
        dst = os.path.join(out, f"{ds}__{m.group(1)}__L{m.group(2)}.tsv")
        with open(dst, "w") as f:
            for i, (_, _, _, body) in enumerate(read_bodies(path, with_bodies=True), start=1):
                f.write(f"{i}\t\\\\x{body.hex()}\n")
        print(dst, flush=True)


if __name__ == "__main__":
    main()
