# KV normalization and optimization — 9 October 2026

**Exploratory uncommitted collection.** The user explicitly waived the pre-measurement harness commit. Exact source/binary/lockfile/fixture hashes and six balanced same-session passes are retained. Original 8 October prototype files/results and production TinyStore remain unchanged.

The work mismatch is fixed, but it did not explain a millisecond write penalty. Removing the owned return eliminates two allocations and 263 bytes per 256-byte Set, or 4103 bytes per 4 KiB Set. Paired old/normalized elapsed ratios remain 0.994× and 1.001×: no useful durable-write gain. Native transaction profiling places 98.2–99.4% of elapsed write-phase time inside FULL COMMIT. Precomputed paths improve short reads modestly; cached transaction commands do not yield a consistent end-to-end write gain. The new 64 KiB case still favors Go, 1.737 ms versus native 2.638 ms per synchronous call, even with matched return work.

The [independent Linux sync-policy supplement below](#linux-sync-policy-supplement) then tests a concrete cause: the native amalgamation was built without `HAVE_FDATASYNC`, so its Unix VFS used fsync while Go used fdatasync. Changing only that platform configuration flag improves native 4 KiB and 64 KiB writes by paired medians 1.507× and 1.615×, bringing them near Go (about 1.04× Go/native). Sync counts, page-write counts, FULL/WAL policy and committed states remain equal between native variants. The copied return was an unfair-work bug; the Linux sync implementation explains a substantial surviving large-value gap in this VM.

The old native benchmark did extra return work: Set/CAS and Take/Delete recreation copied the key, deep-cloned the input value and materialized an owned Entry. Go Set invokes SetEntry internally, but its internal Entry reuses the caller key/value. The new no-result Set returns `Result<()>` after writing only metadata. Explicit SetEntry returns an input-borrowing result with checked key/value pointer identity. Reads and Take continue to materialize owned results.

| Variant | Behavior |
|---|---|
| Go | Original public API executable from production main e307c48 |
| old | Original stock-rusqlite native executable, with extra owned Entry return work |
| normalized | Explicit no-result Set/CAS/recreation and borrowed SetEntry, original path/transaction algorithms |
| paths | Normalized plus precomputed branch prefix and bulk key escaping |
| transactions | Normalized plus cached safe transaction/savepoint statements |
| optimized | Both additional changes |

## Same-session results

Cells are median measured synchronous loop elapsed **microseconds per operation**, not bare-Linux latency guarantees. Ratios use medians of paired per-pass ratios; a Go/native ratio above one favors native. Every case has fixed equal counts selected before the six balanced passes.

| Case | Count/pass | Go μs/op | Old native | Normalized | Paths | Transactions | Optimized | Go/optimized ratio |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `get_bytes` | 80,000 | 6.434 | 3.678 | 3.743 | 3.603 | 3.724 | 3.614 | 1.79× |
| `get_spill` | 80,000 | 7.854 | 4.315 | 4.237 | 4.153 | 4.235 | 4.138 | 1.89× |
| `get_json` | 80,000 | 6.434 | 3.376 | 3.413 | 3.336 | 3.382 | 3.378 | 1.90× |
| `has` | 100,000 | 5.874 | 3.467 | 3.464 | 3.410 | 3.508 | 3.392 | 1.75× |
| `scan100` | 8,000 | 93.255 | 30.358 | 30.381 | 29.661 | 30.717 | 30.192 | 3.11× |
| `set_bytes` | 64 | 3442.103 | 3150.003 | 3177.277 | 3167.011 | 3119.795 | 3165.289 | 1.10× |
| `set_spill` | 64 | 3202.825 | 3184.292 | 3148.939 | 3170.817 | 3215.057 | 3195.309 | 0.99× |
| `cas` | 64 | 3496.848 | 3172.710 | 3151.550 | 3146.340 | 3147.158 | 3162.571 | 1.09× |
| `take_cycle` | 32 | 6515.181 | 6421.854 | 6272.185 | 6441.728 | 6306.884 | 6326.532 | 1.05× |
| `delete_cycle` | 32 | 6556.488 | 6317.762 | 6277.167 | 6299.695 | 6187.299 | 6319.480 | 1.03× |
| `counter_add` | 64 | 3463.204 | 3171.312 | 3161.215 | 3170.842 | 3140.437 | 3167.038 | 1.09× |
| `sql_get` | 100,000 | 5.156 | 3.028 | 3.065 | 2.989 | 3.006 | 3.024 | 1.72× |
| `set_64k` | 128 | 1737.107 | — | 2637.101 | — | — | 2637.657 | 0.66× |
| `setentry_bytes` | 64 | 3432.071 | — | 3093.311 | — | — | 3118.356 | 1.09× |
| `setentry_spill` | 64 | 3216.551 | — | 3146.753 | — | — | 3134.515 | 1.03× |

Paired attribution:

| Case | Old/normalized | Normalized/paths | Normalized/transactions | Normalized/combined |
|---|---:|---:|---:|---:|
| `set_bytes` | 0.994× | 0.999× | 1.016× | 1.011× |
| `set_spill` | 1.001× | 0.986× | 0.979× | 0.982× |
| `cas` | 1.007× | 1.000× | 0.990× | 0.985× |
| `take_cycle` | 1.007× | 0.965× | 1.003× | 1.003× |
| `delete_cycle` | 1.002× | 1.009× | 1.017× | 1.003× |
| `counter_add` | 0.999× | 0.998× | 1.001× | 0.998× |
| `get_bytes` | 0.998× | 1.043× | 1.002× | 1.031× |
| `scan100` | 1.003× | 1.022× | 0.984× | 0.998× |

The original 8 October 4 KiB result (0.72× Go/native) must not be compared directly with this round's near-parity row as an optimization gain. Its loop had 300 timed writes after 64 warmups; this round uses 64 timed writes after 64 warmups. Those are different WAL/checkpoint phases as well as different sessions. The old executable is rerun here precisely so the relevant causal comparison is old versus normalized on identical same-session counts. That comparison shows no useful speedup from removing the extra return copy.

Full per-pass distributions (μs/op), in pass order 0–5:

| Case | Variant | Passes |
|---|---|---|
| `get_bytes` | go | 6.632; 6.294; 6.488; 6.375; 6.381; 6.845 |
| `get_bytes` | old | 3.849; 3.647; 3.664; 3.691; 3.654; 4.100 |
| `get_bytes` | normalized | 3.835; 3.737; 3.670; 3.697; 3.761; 3.750 |
| `get_bytes` | paths | 3.625; 3.620; 3.532; 3.644; 3.585; 3.583 |
| `get_bytes` | transactions | 3.716; 3.673; 3.754; 3.726; 3.722; 3.776 |
| `get_bytes` | optimized | 3.611; 3.635; 3.617; 3.608; 3.572; 3.627 |
| `get_spill` | go | 7.928; 7.749; 7.687; 7.906; 7.803; 7.955 |
| `get_spill` | old | 4.344; 4.292; 4.205; 4.338; 4.279; 4.369 |
| `get_spill` | normalized | 4.248; 5.125; 4.146; 4.311; 4.225; 4.224 |
| `get_spill` | paths | 4.273; 4.221; 4.102; 4.138; 4.072; 4.169 |
| `get_spill` | transactions | 4.261; 4.200; 4.234; 4.235; 4.191; 4.241 |
| `get_spill` | optimized | 4.131; 4.190; 4.112; 4.109; 4.896; 4.144 |
| `get_json` | go | 6.338; 6.450; 6.428; 6.362; 6.441; 6.529 |
| `get_json` | old | 3.354; 3.391; 3.345; 3.455; 3.362; 3.469 |
| `get_json` | normalized | 3.446; 3.366; 3.303; 3.501; 3.578; 3.380 |
| `get_json` | paths | 3.332; 3.457; 3.324; 3.444; 3.339; 3.309 |
| `get_json` | transactions | 3.389; 3.359; 3.375; 3.472; 3.395; 3.365 |
| `get_json` | optimized | 3.362; 3.409; 3.280; 3.397; 3.393; 3.326 |
| `has` | go | 6.940; 5.871; 5.878; 5.901; 5.723; 5.871 |
| `has` | old | 3.537; 3.483; 3.432; 3.559; 3.446; 3.450 |
| `has` | normalized | 3.516; 3.460; 3.443; 3.566; 3.401; 3.469 |
| `has` | paths | 3.432; 3.362; 3.357; 3.470; 3.422; 3.398 |
| `has` | transactions | 3.538; 3.465; 3.429; 4.122; 3.513; 3.504 |
| `has` | optimized | 3.483; 3.347; 3.373; 3.411; 3.425; 3.354 |
| `scan100` | go | 93.966; 93.836; 90.827; 92.674; 91.209; 96.738 |
| `scan100` | old | 29.951; 29.740; 47.588; 30.765; 29.393; 31.497 |
| `scan100` | normalized | 29.616; 30.707; 30.056; 30.945; 28.897; 31.694 |
| `scan100` | paths | 29.636; 29.396; 29.135; 30.011; 29.686; 31.273 |
| `scan100` | transactions | 29.716; 30.918; 37.276; 30.517; 29.628; 33.070 |
| `scan100` | optimized | 30.099; 30.121; 30.264; 30.864; 29.388; 30.519 |
| `set_bytes` | go | 3192.732; 3573.071; 3430.198; 3491.419; 3394.185; 3454.008 |
| `set_bytes` | old | 3166.762; 3215.711; 3108.771; 3208.238; 3089.917; 3133.244 |
| `set_bytes` | normalized | 3170.125; 3184.429; 3074.322; 3260.995; 3123.120; 3238.150 |
| `set_bytes` | paths | 3159.596; 3200.912; 3093.566; 3212.968; 3153.855; 3174.425 |
| `set_bytes` | transactions | 3088.681; 3244.639; 3135.566; 3193.449; 3089.956; 3104.025 |
| `set_bytes` | optimized | 3168.215; 3162.362; 3644.915; 3188.520; 3078.868; 3115.643 |
| `set_spill` | go | 3253.350; 3410.048; 3132.645; 3152.300; 3143.717; 3298.344 |
| `set_spill` | old | 3111.369; 3214.805; 3206.000; 3097.579; 3162.583; 3216.824 |
| `set_spill` | normalized | 3096.468; 3298.321; 3160.212; 3108.909; 3282.657; 3137.666 |
| `set_spill` | paths | 3130.418; 3403.949; 3179.267; 3162.366; 3135.271; 3271.490 |
| `set_spill` | transactions | 3200.966; 3327.402; 3229.147; 3177.532; 3177.089; 3277.267 |
| `set_spill` | optimized | 3180.607; 4440.287; 3136.261; 3210.011; 3224.718; 3170.687 |
| `cas` | go | 3675.931; 3489.742; 3503.954; 3362.710; 3287.043; 3518.756 |
| `cas` | old | 3171.156; 3213.889; 3174.265; 3069.817; 3168.700; 3383.888 |
| `cas` | normalized | 3151.825; 3158.974; 3151.275; 3080.452; 3214.859; 3127.114 |
| `cas` | paths | 3143.109; 3149.570; 3162.498; 3116.264; 3083.829; 3191.474 |
| `cas` | transactions | 3129.945; 3752.600; 3159.787; 3134.529; 3078.038; 3231.675 |
| `cas` | optimized | 3188.871; 3421.545; 3123.716; 3136.271; 3055.027; 3200.716 |
| `take_cycle` | go | 6671.023; 6585.229; 6445.133; 6442.425; 6804.425; 6360.267 |
| `take_cycle` | old | 6454.561; 6389.146; 6187.453; 6121.349; 6667.973; 6524.871 |
| `take_cycle` | normalized | 6442.843; 6393.322; 6151.048; 6068.409; 6067.528; 6431.571 |
| `take_cycle` | paths | 6502.430; 6615.478; 6381.027; 6314.373; 6295.119; 6607.888 |
| `take_cycle` | transactions | 6390.131; 6362.001; 6286.873; 6066.923; 6258.835; 6326.896 |
| `take_cycle` | optimized | 6327.194; 6339.362; 6129.921; 6238.024; 6325.871; 6410.473 |
| `delete_cycle` | go | 6746.032; 6525.921; 6465.706; 6223.098; 6587.054; 6589.338 |
| `delete_cycle` | old | 6508.135; 6342.061; 6173.161; 6317.740; 6082.132; 6317.783 |
| `delete_cycle` | normalized | 6283.287; 6443.315; 6553.208; 6271.048; 6098.832; 6244.402 |
| `delete_cycle` | paths | 6407.415; 6336.513; 6354.863; 6262.876; 6189.720; 6080.427 |
| `delete_cycle` | transactions | 6288.005; 6613.049; 6335.264; 6078.805; 6086.592; 6039.463 |
| `delete_cycle` | optimized | 6322.750; 6435.286; 6408.425; 6245.753; 6316.209; 6034.959 |
| `counter_add` | go | 3390.236; 3570.067; 3574.668; 3329.893; 3463.573; 3462.835 |
| `counter_add` | old | 3171.571; 3171.053; 3196.531; 3163.895; 3185.179; 3146.022 |
| `counter_add` | normalized | 3167.492; 3178.804; 3074.201; 3056.982; 3198.335; 3154.939 |
| `counter_add` | paths | 3245.273; 3183.359; 3051.857; 3063.050; 3212.421; 3158.325 |
| `counter_add` | transactions | 3141.439; 3269.838; 3146.244; 3072.021; 3139.436; 3132.353 |
| `counter_add` | optimized | 3189.230; 3179.512; 3079.502; 3065.522; 3154.565; 3196.413 |
| `sql_get` | go | 5.336; 5.174; 5.138; 5.223; 5.131; 5.116 |
| `sql_get` | old | 3.006; 3.031; 3.629; 3.008; 3.121; 3.025 |
| `sql_get` | normalized | 3.059; 3.026; 3.056; 3.070; 3.097; 3.153 |
| `sql_get` | paths | 2.975; 3.003; 2.914; 3.034; 2.940; 3.023 |
| `sql_get` | transactions | 3.133; 3.047; 2.965; 3.081; 2.941; 2.966 |
| `sql_get` | optimized | 2.960; 3.028; 3.020; 3.033; 3.095; 2.963 |
| `set_64k` | normalized | 2671.223; 2602.979; 2594.750; 2740.711; 2693.475; 2580.667 |
| `set_64k` | optimized | 2690.679; 2584.635; 2494.881; 2538.469; 2827.306; 2909.620 |
| `set_64k` | go-extended | 1775.352; 1701.874; 1726.819; 1758.016; 1694.505; 1747.396 |
| `setentry_bytes` | normalized | 3218.591; 3127.408; 3012.457; 3033.079; 3224.473; 3059.213 |
| `setentry_bytes` | optimized | 3184.804; 3123.518; 3113.195; 3091.887; 3152.572; 3050.026 |
| `setentry_bytes` | go-extended | 3431.895; 3545.746; 3340.460; 3385.824; 3433.037; 3432.246 |
| `setentry_spill` | normalized | 3204.326; 3261.541; 3103.877; 3085.401; 3158.802; 3134.704 |
| `setentry_spill` | optimized | 3171.242; 3235.668; 3203.944; 3012.050; 3097.787; 3072.798 |
| `setentry_spill` | go-extended | 3276.278; 3238.974; 3113.905; 4319.133; 3194.128; 3193.224 |

## Write phase and allocation evidence

Phase timers run only in the separate diagnostic binary; they split transaction begin/savepoint, the write body, release and FULL commit. Its timings are excluded from the performance table. The profile build splits release/commit commands even in normalized mode so the boundary can be observed.

| Case | Variant | Begin/savepoint μs/op | Body | Release | Commit | Commit share |
|---|---|---:|---:|---:|---:|---:|
| `set_bytes` | normalized | 7.136 | 13.818 | 1.142 | 3034.148 | 99.3% |
| `set_bytes` | transactions | 5.716 | 13.709 | 0.849 | 2977.960 | 99.3% |
| `set_bytes` | optimized | 5.908 | 14.028 | 0.859 | 3054.609 | 99.3% |
| `set_spill` | normalized | 7.103 | 19.034 | 0.940 | 2792.010 | 99.0% |
| `set_spill` | transactions | 5.468 | 17.491 | 0.768 | 2767.562 | 99.1% |
| `set_spill` | optimized | 6.171 | 18.599 | 0.544 | 2877.083 | 99.1% |
| `set_64k` | normalized | 7.429 | 38.094 | 1.280 | 2609.139 | 98.2% |
| `set_64k` | transactions | 5.685 | 32.837 | 0.626 | 2552.180 | 98.5% |
| `set_64k` | optimized | 6.715 | 37.760 | 0.669 | 2758.945 | 98.4% |
| `counter_add` | normalized | 7.198 | 12.611 | 0.830 | 3161.771 | 99.4% |
| `counter_add` | transactions | 6.001 | 11.628 | 0.464 | 3148.127 | 99.4% |
| `counter_add` | optimized | 5.348 | 12.708 | 0.452 | 3132.186 | 99.4% |

Global allocator diagnostics (SQLite C allocations excluded for Rust), separately instrumented and never used for speed ratios:

| Case | Variant | Allocations/op | Allocated bytes/op |
|---|---|---:|---:|
| `get_bytes` | go | 30.00 | 1560.0 |
| `get_bytes` | old | 6.00 | 1831.0 |
| `get_bytes` | normalized | 6.00 | 1831.0 |
| `get_bytes` | paths | 4.00 | 1796.0 |
| `get_bytes` | transactions | 6.00 | 1831.0 |
| `get_bytes` | optimized | 4.00 | 1796.0 |
| `scan100` | go | 639.08 | 84861.0 |
| `scan100` | old | 314.00 | 50239.0 |
| `scan100` | normalized | 314.00 | 50239.0 |
| `scan100` | paths | 313.00 | 50227.0 |
| `scan100` | transactions | 314.00 | 50239.0 |
| `scan100` | optimized | 313.00 | 50227.0 |
| `set_bytes` | go | 76.03 | 5449.5 |
| `set_bytes` | old | 8.00 | 2111.0 |
| `set_bytes` | normalized | 6.00 | 1848.0 |
| `set_bytes` | paths | 4.00 | 1813.0 |
| `set_bytes` | transactions | 10.00 | 1968.0 |
| `set_bytes` | optimized | 8.00 | 1933.0 |
| `set_spill` | go | 90.06 | 9595.0 |
| `set_spill` | old | 10.00 | 6055.0 |
| `set_spill` | normalized | 8.00 | 1952.0 |
| `set_spill` | paths | 6.00 | 1917.0 |
| `set_spill` | transactions | 12.00 | 2072.0 |
| `set_spill` | optimized | 10.00 | 2037.0 |
| `counter_add` | go | 63.03 | 3377.5 |
| `counter_add` | old | 3.00 | 4688.0 |
| `counter_add` | normalized | 3.00 | 4688.0 |
| `counter_add` | paths | 3.00 | 4689.0 |
| `counter_add` | transactions | 7.00 | 4808.0 |
| `counter_add` | optimized | 7.00 | 4809.0 |
| `set_64k` | go-extended | 90.02 | 71033.0 |
| `set_64k` | normalized | 8.00 | 1952.0 |
| `set_64k` | optimized | 10.00 | 2037.0 |
| `setentry_bytes` | go-extended | 77.05 | 5458.2 |
| `setentry_bytes` | normalized | 7.00 | 1856.0 |
| `setentry_bytes` | optimized | 9.00 | 1941.0 |
| `setentry_spill` | go-extended | 91.02 | 9600.8 |
| `setentry_spill` | normalized | 9.00 | 1960.0 |
| `setentry_spill` | optimized | 11.00 | 2045.0 |

[syscalls.jsonl](data/kv-optimization-2026-10-09/syscalls.jsonl) retains separate strace sync/read/write counts. Ptrace alters timing; those elapsed times are not performance evidence. GNU time CPU/fault/context-switch/RSS values cover entire processes including connection setup, warmup and close, not isolated loop CPU/op. Native reader/writer cache counters and commit counts are retained per run.

The syscall controls each contain 128 timed writes plus 64 warmups, setup and close. Go's VFS issues `fdatasync`; stock native SQLite issues `fsync`. Native old/normalized/optimized have identical sync counts and nearly identical page I/O. This localizes the surviving difference to a commit/VFS/storage area worth separating next, rather than proving that fsync alone causes it. Setup/close and WAL checkpoint work are included, so these counts are not “syncs per timed operation.” Strace's own syscall-duration percentages are not the untraced transaction phase percentages.

| Whole diagnostic process | Go fdatasync | Native fsync | Go pwrite64 | Native pwrite64 |
|---|---:|---:|---:|---:|
| Set256 | 196 | 197 | 898 | 906 |
| Set4KiB | 199 | 200 | 3049 | 3057 |
| Set64KiB | 208 | 209 | 11832 | 11840 |

The 64 KiB workload's much greater page I/O and extra syncs expose a broader commit/checkpoint workload than 256-byte Set. No VFS, sync flag or checkpoint policy was changed to manufacture a win. These observations support an independently measured sync/VFS ablation, not a claim that copying 4 KiB explained the former loss or that every Rust operation should win.

## Correctness and limitations

All four tuning modes pass the 56-command normalized result/physical snapshot oracle and a second 56-command no-result API trace; 32 native-written typed/raw shapes reopen through Go. Guards retain exact float64/float32 NaN/-0 bits, uint64, empty/binary/string/JSON/spilled values, 1 MiB value/4 MiB page caps, failed-Take rollback, missing spills, stale CAS, counter overflow, DefaultTTL overwrite/recreate, Touch versions, marked Clear and reopen revisions. Borrowed SetEntry must reuse the original key/value addresses. Separate temporary mutants prove both bad-Take rollback and input-ownership regressions fail. Every timed mutating pair/triple/six-way group compares complete final committed rows, revisions, expiries, spill references and orphan counts.

All variants keep original KV schema and bounds, WAL/FULL durability and savepoints. Native Set has no allocation for a returned Entry; safe rusqlite still copies bound bytes according to its normal ownership rules. Prepared transaction statements are executed through safe rusqlite to DONE/reset; failed bodies roll back, and a commit failure attempts rollback and reports outcome unknown. This prototype still lacks concurrent grouped commits, directory lifecycle/locks, memory reservation/cancellation, migration bootstrap, full user transactions/views, background maintenance, sliding/relaxed counters, config/watch/once/quota/limiter and custom codecs/panics. UTF-8 text keys only; arbitrary-byte values remain exact. Those omissions prohibit treating these synchronous ratios as a finished native engine.

The new 64 KiB and explicit SetEntry modes compare new public Go runner/native variants only; the unchanged old executable has no such mode. Each starts from a fresh identical original fixture. Native allocation accounting excludes SQLite C mallocs; Go heap and native allocation bytes have different accounting domains. Payload/file/object sizes remain the original format, retained in storage.json; no compression or storage saving is claimed.

## Environment and reproduction

Collection UTC `2026-10-08T22:28:26.216027+00:00` to `2026-10-08T22:37:59.621567+00:00`. AMD Ryzen 7 7700, Docker Desktop WSL2, Linux volume stores/targets, taskset CPU 2, GOMAXPROCS=1, GOGC=100. Cgroup limits: `{'cpu.max': 'max 100000', 'memory.max': 'max', 'cpuset.cpus.effective': '0-15', 'pids.max': 'max'}`. Competing research builds/tests/measurements were paused by explicit coordination; desktop/background activity outside that coordination was uncontrolled. This is local Windows/WSL2 evidence, not bare-Linux latency or concurrent-throughput guidance.

Production source: `e307c48a40126aad0e2873b6bf3aaedef8115483`; previous research audit commit `4e82af66b38ee057e29d3bbc7b25abd32320d95e`. Go1.27.1, Rust1.99.0, stock rusqlite0.40.1, external official SQLite3.53.4. Source IDs and complete compile options/PRAGMAs are retained. Original native THREADSAFE/mutex/VFS/automatic-initialization differences from Go/Wasm remain. Initial/final original and new source/binary hashes must agree before run.py succeeds.

```sh
cd <repo>/tinystore/kv-opt-bench
python3 build.py
KV_TUNING=0 python3 check.py
KV_TUNING=1 python3 check.py
KV_TUNING=2 python3 check.py
KV_TUNING=3 python3 check.py
python3 guard_proof.py
python3 ownership_proof.py
python3 build.py --metadata-only
# In the coordinated quiet Linux window:
GOMAXPROCS=1 GOGC=100 taskset -c 2 python3 run.py
python3 report.py
```

[README](../kv-opt-bench/README.md) documents setup and API/runtime limits. [environment](data/kv-optimization-2026-10-09/environment.json), [runs](data/kv-optimization-2026-10-09/runs.jsonl), [summary](data/kv-optimization-2026-10-09/summary.json), [calibration](data/kv-optimization-2026-10-09/calibration.jsonl), [phases](data/kv-optimization-2026-10-09/phases.jsonl), [allocations](data/kv-optimization-2026-10-09/allocations.jsonl), [ownership mutant](data/kv-optimization-2026-10-09/ownership-proof.json) and the original failed-Take mutant proof are retained. Linux builds/formatting, three unit tests, all four integration modes and mutation proofs ran. Full production CI/race/SDK suites and Windows/macOS execution were not run; production is unchanged and the native slice lacks their runtime paths.

## Linux sync-policy supplement

This is a separate same-session causal ablation, with its own Go/current-native baseline. Its ratios are not combined with the earlier six-way collection. The candidate links the exact same final optimized Rust source against an isolated SQLite archive compiled with all original flags plus only `-DHAVE_FDATASYNC=1`. The shared archive, previous binaries and measured Rust/Go source remain unchanged.

The pinned amalgamation (`sqlite3.c` SHA-256 `b1dd5d74ec7f29055a6684fa06fb3c2f6821c87dd38f9a458dfd2e8a1db28189`, lines 43928–44041) aliases fdatasync to fsync unless configured; its Unix full_fsync chooses fdatasync on supported Linux systems when enabled. The option is explicitly documented in [SQLite platform configuration](https://sqlite.org/compile.html#have_fdatasync). This changes the Unix sync implementation; it keeps the WAL/FULL synchronization policy, savepoints, commit ordering and SQL. [SQLite synchronous](https://sqlite.org/pragma.html#pragma_synchronous) describes the commit sync required by FULL in WAL mode. This experiment does not reduce synchronous to NORMAL/OFF, skip sync calls or prove power-loss behavior on hardware. It is Linux-specific and does not propose this flag as a Windows/macOS default.

Archive SHA-256 `6a192c36a912ab6e72f61402395681c3b0cebb026e90a1fd6234ff88d65de0eb` versus original `da20c4eb920b6cd5fbd59324969a63c32b322dca253c0d242a2864c619f95bc1`; candidate executable `80f3ef1bc5c1e3840ccaf7142d05d055e40a6284d14e6fde2623e7e7a9959ab8`. Exact flags, C/header/Rust source hashes, crate and binary comparison, source IDs and reader/writer PRAGMA readback are in [build.json](data/kv-optimization-2026-10-09/fdatasync/build.json). Both native variants and production Go read back `synchronous=2`, `journal_mode=wal`, `wal_autocheckpoint=1000`; all native compile-option inventories match because this platform macro is not enumerated by compile_options. The source and observed syscall change, rather than that inventory alone, identify the ablation.

Collection UTC `2026-10-08T22:58:12.832529+00:00` to `2026-10-08T23:02:47.962440+00:00`. Six balanced triples alternate Go→fsync→fdatasync and the reverse. Every fresh closed fixture begins without a WAL, uses the exact `(iteration×997)` key permutation and 64 warmups; initial revision and final committed rows/versions/expiry/spills are retained and must match. Counts are fixed equally across candidates and cover repeated WAL/checkpoint work. Phase evidence from the earlier experiment explains the hypothesis; only this supplement's same-session baselines support its speed comparison.

| Case | Count/pass | Go median μs/op | Native fsync | Native fdatasync | fsync/fdatasync | Go/fdatasync |
|---|---:|---:|---:|---:|---:|---:|
| `set_bytes` | 512 | 3017.909 | 3175.415 | 3029.880 | 1.046× | 0.995× |
| `set_spill` | 512 | 1880.639 | 2696.499 | 1812.193 | 1.507× | 1.040× |
| `set_64k` | 256 | 1794.217 | 2758.706 | 1703.090 | 1.615× | 1.045× |
| `cas` | 512 | 3159.895 | 3221.975 | 3009.584 | 1.074× | 1.044× |
| `counter_add` | 512 | 2956.499 | 3160.221 | 2910.935 | 1.091× | 1.008× |

This separates the backend hypothesis from the earlier return-copy hypothesis. Native 4 KiB and 64 KiB write elapsed falls by about one third and two fifths respectively with the same source, call count, synchronization policy and physical state. Small Set remains essentially tied with Go (paired Go/fdatasync 0.995×); no claim follows that every native operation wins. The effect is established for this Linux/WSL2 filesystem environment and workload, not for bare Linux, Windows, macOS or every storage device.

Per-pass synchronous loop elapsed μs/op (0–5):

| Case | Variant | Passes |
|---|---|---|
| `set_bytes` | go | 3032.414; 3102.292; 2986.911; 2962.091; 3003.403; 3034.284 |
| `set_bytes` | fsync | 3141.952; 3319.677; 3144.326; 3189.675; 3171.209; 3179.621 |
| `set_bytes` | fdatasync | 3021.063; 2934.173; 2916.356; 3038.698; 3044.948; 3077.418 |
| `set_spill` | go | 1879.091; 1908.255; 1873.669; 1882.186; 1849.600; 1885.969 |
| `set_spill` | fsync | 2715.454; 2878.833; 2645.081; 2670.085; 2677.545; 2748.222 |
| `set_spill` | fdatasync | 1776.690; 1827.769; 1932.953; 1816.105; 1792.248; 1808.281 |
| `set_64k` | go | 1793.609; 1804.072; 1960.615; 1743.638; 1754.226; 1794.825 |
| `set_64k` | fsync | 2748.364; 2787.907; 2768.776; 2657.150; 2833.059; 2748.635 |
| `set_64k` | fdatasync | 1721.427; 1674.150; 1703.415; 1702.766; 1700.902; 1712.114 |
| `cas` | go | 3165.020; 3078.569; 3195.681; 3154.771; 3185.251; 3121.734 |
| `cas` | fsync | 3138.966; 3212.811; 3292.588; 3193.360; 3253.225; 3231.140 |
| `cas` | fdatasync | 2983.829; 3000.273; 3050.368; 3106.960; 3018.026; 3001.142 |
| `counter_add` | go | 2901.610; 2898.949; 2968.140; 2944.858; 3148.136; 2974.297 |
| `counter_add` | fsync | 3174.793; 3125.309; 3145.649; 3250.800; 3188.407; 3112.236 |
| `counter_add` | fdatasync | 2910.507; 2964.601; 2911.364; 2980.136; 2870.137; 2826.109 |

Separate whole-process strace counts include 512 timed writes, 64 warmups, setup and close; ptrace elapsed values are excluded from speed comparisons:

| Case | Variant | fsync calls | fdatasync calls | pwrite64 |
|---|---|---:|---:|---:|
| `set_bytes` | go | 0 | 583 | 2682 |
| `set_bytes` | fsync | 584 | 0 | 2690 |
| `set_bytes` | fdatasync | 0 | 584 | 2690 |
| `set_spill` | go | 0 | 592 | 9296 |
| `set_spill` | fsync | 593 | 0 | 9304 |
| `set_spill` | fdatasync | 0 | 593 | 9304 |
| `set_64k` | go | 0 | 616 | 35548 |
| `set_64k` | fsync | 617 | 0 | 35556 |
| `set_64k` | fdatasync | 0 | 617 | 35556 |

The same complete 56-command and 56-command no-result oracle, 32 typed cross-read/rewrites, ownership/bounds/Take/TTL/reopen tests and native unit tests pass for the relinked candidate. Committed state/checksum parity is also checked after every timed triple. Matching sync counts and FULL readback support unchanged synchronization policy; they are not a standalone crash/power-loss proof. All previous runtime omissions and the WSL2 scope still apply.

Reproduce after building the ordinary optimized binaries:

```sh
cd <repo>/tinystore/kv-opt-bench
python3 fdatasync_build.py
# Obtain a separate quiet research window before calibration/timing/strace:
GOMAXPROCS=1 GOGC=100 taskset -c 2 python3 fdatasync_run.py
python3 fdatasync_report.py
```

Supplement artifacts: [runs](data/kv-optimization-2026-10-09/fdatasync/runs.jsonl), [calibration](data/kv-optimization-2026-10-09/fdatasync/calibration.jsonl), [summary](data/kv-optimization-2026-10-09/fdatasync/summary.json), [syscalls](data/kv-optimization-2026-10-09/fdatasync/syscalls.jsonl), [environment and consistency](data/kv-optimization-2026-10-09/fdatasync/environment.json), [correctness](data/kv-optimization-2026-10-09/fdatasync/correctness.json).
