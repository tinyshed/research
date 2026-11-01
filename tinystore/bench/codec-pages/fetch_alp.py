"""Fetch a prefix of each of ALP's public double datasets, by HTTP ranges.

ALP's authors publish their datasets as one zip of raw float64 binaries,
`complete_binaries.zip` (7,226,731,784 bytes) in the Google Drive folder their
BENCHMARKING.md links. This reads the zip's central directory from its tail,
then for each double dataset reads its local header and inflates its deflate
stream from the start until it has the first PREFIX values (or the whole
member, whose CRC-32 is then checked against the zip's). Nothing else of the
7 GB is fetched. The float32 `sp_*` members and `dummy2.bin` are left out, and so is `air_sensor_f.bin`, which is not among
the 30 datasets ALP's `alp_dataset.hpp` lists.

    HTTPS_PROXY=<proxy> python -I fetch_alp.py <corpus>/alp [prefix values]

Writes `<name>.bin` (little-endian float64) and `manifest.json`.
"""

import hashlib
import json
import os
import struct
import sys
import time
import urllib.request
import zlib

FILE_ID = "1-S7NJHIu9V8qPYnIUO3JIwie_2BLRt6N"
TOTAL = 7226731784
URL = f"https://drive.usercontent.google.com/download?id={FILE_ID}&export=download&confirm=t"
# not among the 30 datasets of ALP's alp_dataset.hpp, which prepare.py reads
SKIP = {"dummy2.bin", "air_sensor_f.bin"}


def get(start, end):
    """Bytes [start, end] inclusive, retried: the network here resets."""
    for attempt in range(30):
        try:
            req = urllib.request.Request(URL, headers={"Range": f"bytes={start}-{end}"})
            with urllib.request.urlopen(req, timeout=120) as r:
                data = r.read()
            if len(data) == end - start + 1:
                return data
        except Exception as e:  # noqa: BLE001
            print(f"  range {start}-{end} attempt {attempt}: {e}", flush=True)
        time.sleep(3)
    raise RuntimeError(f"could not fetch {start}-{end}")


def central_directory():
    tail = get(TOTAL - 65536, TOTAL - 1)
    base = TOTAL - len(tail)
    loc = tail.rfind(b"PK\x06\x07")
    z64 = struct.unpack("<Q", tail[loc + 8:loc + 16])[0] - base
    n, _, cd_off = struct.unpack("<QQQ", tail[z64 + 32:z64 + 56])
    p = cd_off - base
    out = []
    for _ in range(n):
        (_, _, _, _, method, _, _, crc, csize, usize, fnl, exl, cml, _, _, _, lho) = struct.unpack(
            "<IHHHHHHIIIHHHHHII", tail[p:p + 46])
        name = tail[p + 46:p + 46 + fnl].decode()
        extra = tail[p + 46 + fnl:p + 46 + fnl + exl]
        q = 0
        while q < len(extra):
            hid, hl = struct.unpack("<HH", extra[q:q + 4])
            if hid == 1:
                vals, k = extra[q + 4:q + 4 + hl], 0
                if usize == 0xFFFFFFFF:
                    usize = struct.unpack("<Q", vals[k:k + 8])[0]; k += 8
                if csize == 0xFFFFFFFF:
                    csize = struct.unpack("<Q", vals[k:k + 8])[0]; k += 8
                if lho == 0xFFFFFFFF:
                    lho = struct.unpack("<Q", vals[k:k + 8])[0]; k += 8
            q += 4 + hl
        out.append(dict(name=name, method=method, crc=crc, csize=csize, usize=usize, offset=lho))
        p += 46 + fnl + exl + cml
    return out


def fetch_prefix(m, want):
    head = get(m["offset"], m["offset"] + 29)
    fnl, exl = struct.unpack("<HH", head[26:30])
    start = m["offset"] + 30 + fnl + exl
    end = start + m["csize"]
    inflater = zlib.decompressobj(-15)
    out, pos, chunk = bytearray(), start, 4 << 20
    while len(out) < want and pos < end:
        piece = get(pos, min(end, pos + chunk) - 1)
        pos += len(piece)
        out += inflater.decompress(piece)
    whole = pos >= end
    if whole:
        out += inflater.flush()
    return bytes(out[:want]), whole, pos - start


def main():
    out_dir = sys.argv[1]
    prefix = int(sys.argv[2]) if len(sys.argv) > 2 else 1 << 20
    os.makedirs(out_dir, exist_ok=True)
    members = [m for m in central_directory()
               if m["name"].endswith(".bin") and not m["name"].startswith("sp_") and m["name"] not in SKIP]
    manifest = {"source": URL, "zip_bytes": TOTAL, "prefix_values": prefix, "members": []}
    for m in members:
        want = min(m["usize"], prefix * 8)
        data, whole, fetched = fetch_prefix(m, want)
        assert len(data) == want, (m["name"], len(data), want)
        if want == m["usize"]:
            assert whole and zlib.crc32(data) == m["crc"], f"CRC mismatch {m['name']}"
        path = os.path.join(out_dir, m["name"])
        with open(path, "wb") as f:
            f.write(data)
        rec = dict(m, values=want // 8, whole=want == m["usize"], compressed_bytes_fetched=fetched,
                   sha256=hashlib.sha256(data).hexdigest())
        manifest["members"].append(rec)
        print(json.dumps(rec), flush=True)
    with open(os.path.join(out_dir, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=1)


if __name__ == "__main__":
    main()
