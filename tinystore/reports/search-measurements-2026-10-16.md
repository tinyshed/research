# Search measurement tables — 16 October 2026

Values separated by `;` are pass 1; pass 2. These are native research CLI measurements, not public Core/Wire/SDK rates. See the architecture report and raw ledger for corpus, semantics and durability boundaries.

## Stripped Linux CLI binaries

| Profile | Bytes | MiB | Delta from baseline MiB |
|---|---|---|---|
| baseline | 2577456 | 2.458 | 0.000 |
| icu | 5122704 | 4.885 | 2.427 |
| auto | 5204528 | 4.963 | 2.505 |
| detect | 5470352 | 5.217 | 2.759 |
| jieba | 8329296 | 7.943 | 5.485 |
| tantivy | 8897712 | 8.486 | 6.027 |
| full | 12281328 | 11.712 | 9.254 |

Whole reachable executables with the same release/thin-LTO profile. ICU segmentation data/models, Whatlang and Jieba dictionaries are embedded as enabled; this is not a measured increase to the TinyStore product binary.

## Retrieval build/top-10

| Scenario | Docs | Build s | Query p50 ms | p95 ms | p99 ms | Query/s | Index MiB | Source MiB | Build RSS MiB | Build WAL file MiB |
|---|---|---|---|---|---|---|---|---|---|---|
| ru-fts-unicode | 25086; 25086 | 1.901; 1.954 | 10.397; 10.403 | 36.978; 40.219 | 47.472; 47.135 | 62.069; 59.775 | 15.398; 15.398 | 40.844; 40.844 | 45.301; 45.387 | 16.601; 16.601 |
| ru-fts-icu | 25086; 25086 | 2.481; 2.887 | 9.142; 10.122 | 37.537; 37.917 | 45.894; 42.284 | 62.204; 61.766 | 15.414; 15.414 | 40.844; 40.844 | 47.957; 47.793 | 16.660; 16.660 |
| ru-tant-icu | 25086; 25086 | 2.576; 2.653 | 0.190; 0.186 | 0.500; 0.614 | 0.919; 0.872 | 4242.771; 4225.739 | 11.514; 11.514 | 40.844; 40.844 | 71.520; 71.406 | 0.000; 0.000 |
| ru-fts-auto | 25086; 25086 | 5.381; 5.696 | 21.375; 24.858 | 71.723; 81.572 | 90.494; 142.534 | 34.099; 28.968 | 27.227; 27.227 | 40.844; 40.844 | 47.887; 48.098 | 28.561; 28.561 |
| ru-tant-auto | 25086; 25086 | 5.156; 5.199 | 0.544; 0.630 | 1.650; 3.429 | 2.816; 5.027 | 1403.765; 951.307 | 20.725; 20.725 | 40.844; 40.844 | 72.090; 72.914 | 0.000; 0.000 |
| en-fts-unicode | 31263; 31263 | 1.074; 1.144 | 42.400; 47.746 | 73.460; 85.573 | 95.554; 175.015 | 24.888; 20.675 | 16.449; 16.449 | 27.387; 27.387 | 36.957; 37.145 | 17.284; 17.284 |
| en-fts-icu | 31263; 31263 | 2.394; 2.411 | 47.473; 45.571 | 88.212; 75.056 | 139.757; 102.050 | 21.456; 23.103 | 16.586; 16.586 | 27.387; 27.387 | 40.301; 40.297 | 17.253; 17.253 |
| en-tant-icu | 31263; 31263 | 2.795; 2.427 | 0.329; 0.356 | 0.827; 0.874 | 1.227; 1.667 | 2544.497; 2453.807 | 11.114; 11.114 | 27.387; 27.387 | 60.211; 60.348 | 0.000; 0.000 |
| en-fts-auto | 31263; 31263 | 7.084; 6.944 | 72.747; 82.430 | 162.834; 206.417 | 254.230; 301.905 | 13.198; 11.029 | 30.656; 30.656 | 27.387; 27.387 | 40.133; 40.137 | 32.498; 32.498 |
| en-tant-auto | 31263; 31263 | 7.280; 6.060 | 1.134; 1.166 | 2.747; 3.019 | 5.792; 5.583 | 734.956; 718.840 | 21.413; 21.413 | 27.387; 27.387 | 61.047; 60.449 | 0.000; 0.000 |
| zh-fts-unicode | 31826; 31826 | 0.798; 1.038 | 0.027; 0.028 | 0.055; 0.119 | 0.184; 0.768 | 29792.939; 19855.058 | 14.293; 14.293 | 17.098; 17.098 | 28.613; 28.750 | 15.332; 15.332 |
| zh-fts-icu | 31826; 31826 | 2.943; 2.688 | 16.029; 14.699 | 55.086; 49.034 | 80.254; 108.978 | 45.147; 45.740 | 13.176; 13.176 | 17.098; 17.098 | 32.012; 32.039 | 13.917; 13.917 |
| zh-tant-icu | 31826; 31826 | 2.770; 3.714 | 0.137; 0.148 | 0.426; 2.287 | 0.654; 4.662 | 5617.662; 2328.912 | 8.615; 8.615 | 17.098; 17.098 | 50.109; 50.027 | 0.000; 0.000 |
| zh-fts-auto | 31826; 31826 | 3.066; 2.927 | 14.702; 14.858 | 51.372; 47.526 | 112.415; 128.818 | 45.863; 45.447 | 13.551; 13.551 | 17.098; 17.098 | 32.012; 32.059 | 14.475; 14.475 |
| zh-tant-auto | 31826; 31826 | 3.240; 3.265 | 0.186; 0.178 | 0.689; 0.426 | 1.151; 1.025 | 3810.395; 4739.112 | 9.267; 9.267 | 17.098; 17.098 | 50.336; 50.418 | 0.000; 0.000 |
| zh-fts-jieba | 31826; 31826 | 3.256; 2.371 | 19.388; 16.822 | 52.751; 46.654 | 70.909; 57.054 | 43.271; 47.893 | 13.504; 13.504 | 17.098; 17.098 | 62.340; 61.883 | 14.420; 14.420 |
| zh-tant-jieba | 31826; 31826 | 3.084; 2.887 | 0.181; 0.184 | 0.501; 0.658 | 0.727; 1.189 | 4411.505; 3972.116 | 10.526; 10.526 | 17.098; 17.098 | 108.660; 108.633 | 0.000; 0.000 |

## Natural query relevance (200 queries/language; mixed=600)

| Scenario | nDCG@10 | MRR@10 | Recall@10 | Known negatives/query | Judged docs/query | Judged-pool nDCG@10 | Judged-pool recall@10 |
|---|---|---|---|---|---|---|---|
| ru-fts-unicode | 0.532; 0.532 | 0.566; 0.566 | 0.671; 0.671 | 3.700; 3.700 | 5.440; 5.440 | 0.680; 0.680 | 0.915; 0.915 |
| ru-fts-icu | 0.447; 0.447 | 0.493; 0.493 | 0.576; 0.576 | 3.455; 3.455 | 4.975; 4.975 | 0.634; 0.634 | 0.900; 0.900 |
| ru-tant-icu | 0.439; 0.439 | 0.492; 0.492 | 0.560; 0.560 | 3.380; 3.380 | 4.825; 4.825 | 0.631; 0.631 | 0.896; 0.896 |
| ru-fts-auto | 0.534; 0.534 | 0.540; 0.540 | 0.700; 0.700 | 4.145; 4.145 | 6.095; 6.095 | 0.672; 0.672 | 0.958; 0.958 |
| ru-tant-auto | 0.508; 0.508 | 0.537; 0.537 | 0.671; 0.671 | 3.870; 3.870 | 5.660; 5.660 | 0.661; 0.661 | 0.953; 0.953 |
| en-fts-unicode | 0.576; 0.576 | 0.558; 0.558 | 0.773; 0.773 | 5.455; 5.455 | 7.925; 7.925 | 0.652; 0.652 | 0.927; 0.927 |
| en-fts-icu | 0.577; 0.577 | 0.557; 0.557 | 0.780; 0.780 | 5.410; 5.410 | 7.900; 7.900 | 0.651; 0.651 | 0.933; 0.933 |
| en-tant-icu | 0.598; 0.598 | 0.605; 0.605 | 0.782; 0.782 | 5.020; 5.020 | 7.390; 7.390 | 0.684; 0.684 | 0.924; 0.924 |
| en-fts-auto | 0.589; 0.589 | 0.563; 0.563 | 0.822; 0.822 | 6.170; 6.170 | 8.785; 8.785 | 0.627; 0.627 | 0.899; 0.899 |
| en-tant-auto | 0.615; 0.615 | 0.616; 0.616 | 0.822; 0.822 | 5.320; 5.320 | 7.760; 7.760 | 0.682; 0.682 | 0.924; 0.924 |
| zh-fts-unicode | 0.003; 0.003 | 0.003; 0.003 | 0.005; 0.005 | 0.005; 0.005 | 0.015; 0.015 | 0.003; 0.003 | 0.005; 0.005 |
| zh-fts-icu | 0.409; 0.409 | 0.377; 0.377 | 0.597; 0.597 | 3.995; 3.995 | 5.480; 5.480 | 0.598; 0.598 | 0.962; 0.962 |
| zh-tant-icu | 0.403; 0.403 | 0.387; 0.387 | 0.575; 0.575 | 3.685; 3.685 | 5.090; 5.090 | 0.604; 0.604 | 0.962; 0.962 |
| zh-fts-auto | 0.409; 0.409 | 0.377; 0.377 | 0.597; 0.597 | 3.995; 3.995 | 5.480; 5.480 | 0.598; 0.598 | 0.962; 0.962 |
| zh-tant-auto | 0.403; 0.403 | 0.382; 0.382 | 0.577; 0.577 | 3.680; 3.680 | 5.090; 5.090 | 0.604; 0.604 | 0.962; 0.962 |
| zh-fts-jieba | 0.274; 0.274 | 0.268; 0.268 | 0.398; 0.398 | 2.765; 2.765 | 3.650; 3.650 | 0.584; 0.584 | 0.953; 0.953 |
| zh-tant-jieba | 0.266; 0.266 | 0.266; 0.266 | 0.392; 0.392 | 2.365; 2.365 | 3.195; 3.195 | 0.592; 0.592 | 0.953; 0.953 |

Main query timings return IDs, not hydrated source documents. Both engines index full positions/frequencies. Ranked BM25 defaults, field normalization and synonym scoring differ; same analyzer does not imply identical ranks. Unicode61 is an analyzer baseline, not the same token semantics as ICU. Standalone source/index commits in the Tantivy arm are independent. Scale repeats the natural pool and limits to 60 queries; its relevance values are not population-quality measurements.

## Source-backed incremental phases: 1000 rows, 100 rows/commit

| Scenario | Operation | SQL/enqueue batch p99 ms | External commit/reload total ms | SQLite WAL MiB |
|---|---|---|---|---|
| ru-fts-unicode | insert | 49.620; 179.453 | 0.000; 0.000 | 20.274; 20.274 |
| ru-fts-unicode | update | 62.924; 132.937 | 0.000; 0.000 | 18.361; 18.361 |
| ru-fts-unicode | unrelated | 2.085; 4.160 | 0.000; 0.000 | 1.693; 1.693 |
| ru-fts-unicode | delete | 31.776; 109.956 | 0.000; 0.000 | 5.670; 5.670 |
| ru-fts-icu | insert | 50.943; 55.135 | 0.000; 0.000 | 20.239; 20.239 |
| ru-fts-icu | update | 72.498; 80.903 | 0.000; 0.000 | 18.487; 18.487 |
| ru-fts-icu | unrelated | 2.479; 3.207 | 0.000; 0.000 | 1.693; 1.693 |
| ru-fts-icu | delete | 40.893; 36.550 | 0.000; 0.000 | 5.839; 5.839 |
| ru-tant-icu | insert | 4.243; 8.819 | 417.847; 351.934 | 2.020; 2.020 |
| ru-tant-icu | update | 4.815; 18.568 | 439.618; 343.448 | 1.855; 1.855 |
| ru-tant-icu | unrelated | 4.516; 6.479 | 0.000; 0.000 | 1.693; 1.693 |
| ru-tant-icu | delete | 7.392; 15.279 | 103.392; 42.881 | 0.338; 0.338 |
| ru-fts-auto | insert | 75.673; 171.770 | 0.000; 0.000 | 23.465; 23.465 |
| ru-fts-auto | update | 85.392; 216.744 | 0.000; 0.000 | 27.394; 27.394 |
| ru-fts-auto | unrelated | 2.106; 2.486 | 0.000; 0.000 | 1.693; 1.693 |
| ru-fts-auto | delete | 75.782; 144.253 | 0.000; 0.000 | 12.679; 12.679 |
| ru-tant-auto | insert | 3.166; 5.526 | 414.523; 572.887 | 2.020; 2.020 |
| ru-tant-auto | update | 25.235; 4.850 | 654.714; 897.788 | 1.855; 1.855 |
| ru-tant-auto | unrelated | 3.582; 9.057 | 0.000; 0.000 | 1.693; 1.693 |
| ru-tant-auto | delete | 2.345; 14.938 | 61.723; 114.117 | 0.338; 0.338 |
| en-fts-unicode | insert | 74.634; 76.411 | 0.000; 0.000 | 22.137; 22.137 |
| en-fts-unicode | update | 45.526; 48.625 | 0.000; 0.000 | 12.224; 12.224 |
| en-fts-unicode | unrelated | 2.446; 2.238 | 0.000; 0.000 | 0.927; 0.927 |
| en-fts-unicode | delete | 52.755; 60.499 | 0.000; 0.000 | 14.848; 14.848 |
| en-fts-icu | insert | 82.753; 115.889 | 0.000; 0.000 | 21.870; 21.870 |
| en-fts-icu | update | 64.413; 59.471 | 0.000; 0.000 | 13.410; 13.410 |
| en-fts-icu | unrelated | 1.630; 2.747 | 0.000; 0.000 | 0.927; 0.927 |
| en-fts-icu | delete | 55.450; 51.382 | 0.000; 0.000 | 14.978; 14.978 |
| en-tant-icu | insert | 3.061; 13.706 | 233.574; 479.313 | 1.214; 1.214 |
| en-tant-icu | update | 3.093; 6.326 | 333.452; 309.006 | 0.974; 0.974 |
| en-tant-icu | unrelated | 3.346; 4.197 | 0.000; 0.000 | 0.927; 0.927 |
| en-tant-icu | delete | 6.491; 4.365 | 75.550; 51.326 | 0.326; 0.326 |
| en-fts-auto | insert | 126.703; 68.305 | 0.000; 0.000 | 19.241; 19.241 |
| en-fts-auto | update | 89.761; 97.113 | 0.000; 0.000 | 22.298; 22.298 |
| en-fts-auto | unrelated | 2.204; 1.731 | 0.000; 0.000 | 0.927; 0.927 |
| en-fts-auto | delete | 82.571; 75.748 | 0.000; 0.000 | 19.756; 19.756 |
| en-tant-auto | insert | 3.150; 9.098 | 522.126; 414.944 | 1.214; 1.214 |
| en-tant-auto | update | 6.229; 7.945 | 465.874; 447.504 | 0.974; 0.974 |
| en-tant-auto | unrelated | 24.164; 3.395 | 0.000; 0.000 | 0.927; 0.927 |
| en-tant-auto | delete | 3.500; 7.740 | 111.979; 84.342 | 0.326; 0.326 |
| zh-fts-unicode | insert | 34.932; 44.675 | 0.000; 0.000 | 19.017; 19.017 |
| zh-fts-unicode | update | 50.837; 39.362 | 0.000; 0.000 | 12.593; 12.593 |
| zh-fts-unicode | unrelated | 4.410; 2.830 | 0.000; 0.000 | 0.625; 0.625 |
| zh-fts-unicode | delete | 36.565; 37.981 | 0.000; 0.000 | 10.106; 10.106 |
| zh-fts-icu | insert | 66.258; 64.455 | 0.000; 0.000 | 18.302; 18.302 |
| zh-fts-icu | update | 75.750; 63.548 | 0.000; 0.000 | 12.778; 12.778 |
| zh-fts-icu | unrelated | 4.979; 4.300 | 0.000; 0.000 | 0.625; 0.625 |
| zh-fts-icu | delete | 73.497; 54.556 | 0.000; 0.000 | 7.611; 7.611 |
| zh-tant-icu | insert | 2.207; 9.015 | 291.009; 420.474 | 0.864; 0.864 |
| zh-tant-icu | update | 2.387; 26.948 | 302.864; 574.528 | 0.695; 0.695 |
| zh-tant-icu | unrelated | 1.121; 1.480 | 0.000; 0.000 | 0.625; 0.625 |
| zh-tant-icu | delete | 1.235; 2.318 | 43.802; 53.314 | 0.409; 0.409 |
| zh-fts-auto | insert | 117.980; 50.929 | 0.000; 0.000 | 18.408; 18.408 |
| zh-fts-auto | update | 87.977; 167.109 | 0.000; 0.000 | 13.222; 13.222 |
| zh-fts-auto | unrelated | 6.947; 2.185 | 0.000; 0.000 | 0.625; 0.625 |
| zh-fts-auto | delete | 85.287; 55.343 | 0.000; 0.000 | 8.031; 8.031 |
| zh-tant-auto | insert | 3.428; 2.772 | 296.910; 322.090 | 0.864; 0.864 |
| zh-tant-auto | update | 3.838; 3.341 | 330.674; 401.064 | 0.695; 0.695 |
| zh-tant-auto | unrelated | 1.638; 1.213 | 0.000; 0.000 | 0.625; 0.625 |
| zh-tant-auto | delete | 2.164; 1.205 | 49.510; 42.869 | 0.409; 0.409 |
| zh-fts-jieba | insert | 64.397; 58.194 | 0.000; 0.000 | 19.331; 19.331 |
| zh-fts-jieba | update | 68.463; 72.561 | 0.000; 0.000 | 12.243; 12.243 |
| zh-fts-jieba | unrelated | 2.015; 5.049 | 0.000; 0.000 | 0.625; 0.625 |
| zh-fts-jieba | delete | 53.494; 65.483 | 0.000; 0.000 | 7.426; 7.426 |
| zh-tant-jieba | insert | 4.650; 5.575 | 232.184; 252.044 | 0.864; 0.864 |
| zh-tant-jieba | update | 4.452; 6.543 | 302.975; 501.410 | 0.695; 0.695 |
| zh-tant-jieba | unrelated | 1.689; 3.545 | 0.000; 0.000 | 0.625; 0.625 |
| zh-tant-jieba | delete | 1.624; 9.268 | 49.056; 71.203 | 0.409; 0.409 |

For Tantivy the first latency includes SQLite work and enqueue; its separate durable commits/reloads are the next column, a total over ten commits. Do not quote the first column as complete durable Search write latency or zero SQLite WAL as zero external I/O. The owned-store control below measures complete commit/visibility latency.

Bulk import is one durable commit through upsert; incremental batch=1 and batch=100 are distinct workloads. SQLite checkpoints occur outside operation latency; Tantivy uses synchronous commit/reload and its normal background merge behavior. These timings do not establish TinyStore grouped-commit integration or cross-engine transactions.

## Raw inventory

| File | Records | Nonzero process exits |
|---|---|---|
| correctness.jsonl | 3 | 1 |
| gold.jsonl | 12 | 0 |
| retrieval.jsonl | 34 | 0 |

