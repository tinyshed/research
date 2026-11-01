"""Readers and writers of the harness's series and bodies files (README.md)."""

import struct


def write_series(path, series):
    """series: iterable of (name, values) where values is a bytes object of
    little-endian float64s or a sequence of floats."""
    series = list(series)
    with open(path, "wb") as f:
        f.write(b"CPS1")
        f.write(struct.pack("<I", len(series)))
        for name, values in series:
            raw = values if isinstance(values, (bytes, bytearray, memoryview)) else struct.pack(f"<{len(values)}d", *values)
            assert len(raw) % 8 == 0
            nb = name.encode()
            f.write(struct.pack("<I", len(nb)))
            f.write(nb)
            f.write(struct.pack("<Q", len(raw) // 8))
            f.write(raw)


def read_series(path):
    """Yields (name, raw little-endian float64 bytes)."""
    with open(path, "rb") as f:
        assert f.read(4) == b"CPS1", "not a series file"
        (count,) = struct.unpack("<I", f.read(4))
        for _ in range(count):
            (nl,) = struct.unpack("<I", f.read(4))
            name = f.read(nl).decode()
            (n,) = struct.unpack("<Q", f.read(8))
            yield name, f.read(8 * n)


def series_lengths(path):
    """[(name, n)] without keeping the values."""
    out = []
    with open(path, "rb") as f:
        assert f.read(4) == b"CPS1", "not a series file"
        (count,) = struct.unpack("<I", f.read(4))
        for _ in range(count):
            (nl,) = struct.unpack("<I", f.read(4))
            name = f.read(nl).decode()
            (n,) = struct.unpack("<Q", f.read(8))
            f.seek(8 * n, 1)
            out.append((name, n))
    return out


def blocks_of(lengths, L):
    """The block rule: (series index, values) for every block, in order."""
    out = []
    for i, (_, n) in enumerate(lengths):
        full, rest = divmod(n, L)
        out.extend([(i, L)] * full)
        if rest:
            out.append((i, rest))
    return out


def read_bodies(path, with_bodies=False):
    """Yields (series index, values, status, body length or body bytes)."""
    with open(path, "rb") as f:
        assert f.read(4) == b"CPB1", "not a bodies file"
        (count,) = struct.unpack("<I", f.read(4))
        for _ in range(count):
            si, nv, bl, st = struct.unpack("<IIIB", f.read(13))
            if with_bodies:
                yield si, nv, st, f.read(bl)
            else:
                f.seek(bl, 1)
                yield si, nv, st, bl
