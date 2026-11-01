"""Turn the fetched corpora into the harness's series files (README.md).

    python3 -I prepare.py <corpus dir>

Reads, under <corpus dir>:

  alp/<member>.bin           fetch_alp.py's prefixes of ALP's 30 datasets
  nab/<category>/<file>.csv  NAB's files, as ../nab-sha256.txt pins them
  tsbs-long.jsonl            ../tsbs's conversion of TSBS DevOps, ten hosts, three days
  alibaba-series.jsonl       the Alibaba selection of metrics-storage-inputs
  tsbs-wide.jsonl            optional, the 5,090,400-sample TSBS corpus of 2026-09

and writes <corpus dir>/series/<dataset>.series, one series file a dataset,
with series in input order, and prints each file's SHA-256.
"""

import glob
import hashlib
import json
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cpfiles import write_series  # noqa: E402

# ALP's 30 double datasets, by the names its alp_dataset.hpp gives them
ALP = [
    ("Air-Pressure", "neon_air_pressure.bin"), ("Arade-4", "arade4.bin"),
    ("Basel-Temp", "basel_temp_f.bin"), ("Basel-Wind", "basel_wind_f.bin"),
    ("Bird-Mig", "bird_migration_f.bin"), ("Btc-Price", "bitcoin_f.bin"),
    ("Blockchain", "bitcoin_transactions_f.bin"), ("City-Temp", "city_temperature_f.bin"),
    ("CMS-1", "cms1.bin"), ("CMS-9", "cms9.bin"), ("CMS-25", "cms25.bin"),
    ("Dew-Temp", "neon_dew_point_temp.bin"), ("Bio-Temp", "neon_bio_temp_c.bin"),
    ("Food-prices", "food_prices.bin"), ("Gov-10", "gov10.bin"), ("Gov-26", "gov26.bin"),
    ("Gov-30", "gov30.bin"), ("Gov-31", "gov31.bin"), ("Gov-40", "gov40.bin"),
    ("Medicare-1", "medicare1.bin"), ("Medicare-9", "medicare9.bin"),
    ("PM10-dust", "neon_pm10_dust.bin"), ("NYC-29", "nyc29.bin"), ("POI-lat", "poi_lat.bin"),
    ("POI-lon", "poi_lon.bin"), ("SD-bench", "ssd_hdd_benchmarks_f.bin"),
    ("Stocks-DE", "stocks_de.bin"), ("Stocks-UK", "stocks_uk.bin"),
    ("Stocks-USA", "stocks_usa_c.bin"), ("Wind-dir", "neon_wind_dir.bin"),
]


def sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(1 << 20):
            h.update(chunk)
    return h.hexdigest()


def jsonl_series(path):
    out = []
    with open(path) as f:
        for line in f:
            rec = json.loads(line)
            m = rec["metric"]
            name = m.get("__name__", "") + "{" + ",".join(f"{k}={v}" for k, v in m.items() if k != "__name__") + "}"
            vals = [float(v) for v in rec["values"]]
            ts = rec["timestamps"]
            assert all(b > a for a, b in zip(ts, ts[1:])), f"timestamps not increasing in {name}"
            out.append((name, struct.pack(f"<{len(vals)}d", *vals)))
    return out


def nab_series(root):
    out = []
    for path in sorted(glob.glob(os.path.join(root, "*", "*.csv"))):
        vals = []
        with open(path) as f:
            header = f.readline()
            assert header.strip().lower().startswith("timestamp"), path
            for line in f:
                line = line.strip()
                if line:
                    vals.append(float(line.split(",")[1]))
        rel = os.path.relpath(path, root).replace(os.sep, "/")
        out.append((rel, struct.pack(f"<{len(vals)}d", *vals)))
    return out


def main():
    root = sys.argv[1]
    dst = os.path.join(root, "series")
    os.makedirs(dst, exist_ok=True)
    jobs = []
    for name, member in ALP:
        p = os.path.join(root, "alp", member)
        if os.path.exists(p):
            jobs.append((f"alp-{name}", lambda p=p, name=name: [(name, open(p, "rb").read())]))
    if os.path.isdir(os.path.join(root, "nab")):
        jobs.append(("nab", lambda: nab_series(os.path.join(root, "nab"))))
    for ds, fn in (("tsbs-long", "tsbs-long.jsonl"), ("alibaba", "alibaba-series.jsonl"),
                   ("tsbs-wide", "tsbs-wide.jsonl")):
        p = os.path.join(root, fn)
        if os.path.exists(p):
            jobs.append((ds, lambda p=p: jsonl_series(p)))
    for ds, make in jobs:
        out = os.path.join(dst, ds + ".series")
        if os.path.exists(out):
            print(f"{sha(out)}  series/{ds}.series  (kept)")
            continue
        series = make()
        write_series(out, series)
        n = sum(len(v) // 8 for _, v in series)
        print(f"{sha(out)}  series/{ds}.series  series={len(series)} values={n}", flush=True)


if __name__ == "__main__":
    main()
