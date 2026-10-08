# Completed capped collection

The completed report is
`../reports/records-optimization-2026-10-09.md`; completed raw data lives in
`../reports/data/records-optimization-2026-10-09/capped/`.

`run.py` is preserved as the initial fastest-count plan. The final collection
uses `run_capped.py`, which reads the retained original pilots under
`partial-fastest-counts/calibration.json` and applies identical capped counts
across candidates. No new pilot is required to reproduce that count plan.

In the same Linux environment and layout described by README:

```sh
export PATH=/work/cargo/bin:$PATH
python3 /src/tinystore/records-opt-bench/build.py
python3 /src/tinystore/records-opt-bench/prove_guard.py
python3 /src/tinystore/records-opt-bench/build.py
chmod 755 /work/records-opt/rust-alloc-records
python3 /src/tinystore/records-opt-bench/verify.py
# After competing research CPU work stops:
python3 /src/tinystore/records-opt-bench/run_capped.py --measure
```

Keep new reproduction output separate from retained published raw data.
The original run needed a permission-only diagnostic recovery; the command
above sets executable mode before collection. `finish_diagnostics.py` remains
as the exact post-collection helper for that recorded failure, rather than
as a normal second benchmark command. It does not repeat successful timings.

This file, `report.py` and `report-template.md` were added after measurement.
Their absence from the measured source inventory is deliberate; original
runtime and runner hashes remain unchanged.
