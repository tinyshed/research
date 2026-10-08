# Records optimization raw data

The completed six-pass collection and diagnostics are in **`capped/`**.
Use its `summary.json`, raw timing/ablation/cache JSONL, allocations, memory,
source/binary/fixture environment and `completion.json` together.

`partial-fastest-counts/` preserves the stopped first count plan: original
runner, environment, pilots and 224 complete timing rows with hashes/status.
The top-level `environment.json`, `calibration.json` and `timings.jsonl` are
the untouched original copies of that same partial collection. They are not
the completed round and are not merged into headline statistics.

Top-level `build.json`, `verification.json` and `guard-proof.json` are the
successful preparation/contract results used by both plans. The capped
allocation run first encountered an execute-mode error after all timings
completed; its one-row pre-fix output is retained separately. A permission-only
fix and hashed post-collection helper completed diagnostics without rebuilding
or repeating successful timings. See `capped/completion.json`.

`capped-projection.json` records the capped count-plan estimate. Timing
counts are identical across all candidates in each fixture/case; the cache-only
supplement uses a separately identified common count among its three variants.
Every sample count, achieved duration and pilot cap remains in the JSON.
