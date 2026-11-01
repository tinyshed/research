"""A small series file for checking encoders: a few real columns and edge cases.

    python3 -I make_test_series.py <out.series> [<csv> ...]

Each CSV given is one series of its first column's numbers (Elf's test data
has one number a line). Four synthetic series follow: integers, a decimal
walk, random IEEE-754 bit patterns, and edge values (-0.0, infinities,
subnormals, the largest and smallest doubles). NaN is left out on purpose:
several codecs use a NaN as their end-of-stream marker, and a series with one
is a separate test.
"""

import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cpfiles import write_series  # noqa: E402


def main():
    out, csvs = sys.argv[1], sys.argv[2:]
    series = []
    for path in csvs:
        vals = []
        with open(path) as f:
            for line in f:
                line = line.strip().split(",")[0]
                try:
                    vals.append(float(line))
                except ValueError:
                    continue
        series.append((os.path.basename(path), vals[:20000]))
    rnd = random.Random(20261101)
    series.append(("integers", [float(rnd.randint(0, 100)) for _ in range(5000)]))
    walk, v = [], 20.0
    for _ in range(5000):
        v = round(v + rnd.choice((-0.1, 0.0, 0.1)), 1)
        walk.append(v)
    series.append(("decimal-walk", walk))
    bits = [struct.unpack("<d", struct.pack("<Q", rnd.getrandbits(64)))[0] for _ in range(3000)]
    series.append(("random-bits", [b for b in bits if b == b]))
    edge = [0.0, -0.0, float("inf"), float("-inf"), 5e-324, -5e-324, 2.2250738585072014e-308,
            1.7976931348623157e308, -1.7976931348623157e308, 1.0, -1.0, 0.1, 1e-300, 123456789.123]
    series.append(("edge", [edge[i % len(edge)] for i in range(2100)]))
    write_series(out, series)
    print(out, [(n, len(v)) for n, v in series])


if __name__ == "__main__":
    main()
