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
| mixed-fts-unicode | 88175; 88175 | 4.905; 4.379 | 8.725; 8.360 | 75.001; 65.446 | 103.699; 89.102 | 44.378; 48.171 | 44.211; 44.211 | 85.965; 85.965 | 92.191; 91.734 | 46.030; 46.030 |
| mixed-fts-icu | 88175; 88175 | 9.451; 8.235 | 27.801; 24.593 | 81.469; 66.037 | 91.974; 85.961 | 31.456; 36.055 | 43.094; 43.094 | 85.965; 85.965 | 95.492; 94.957 | 44.293; 44.293 |
| mixed-tant-icu | 88175; 88175 | 7.968; 9.150 | 0.303; 0.291 | 1.160; 1.078 | 2.261; 2.038 | 2214.512; 2367.761 | 31.397; 31.397 | 85.965; 85.965 | 119.305; 119.441 | 0.000; 0.000 |
| mixed-fts-auto | 88175; 88175 | 20.150; 16.852 | 33.383; 32.149 | 124.766; 119.202 | 225.780; 168.015 | 22.105; 22.956 | 69.438; 69.438 | 85.965; 85.965 | 95.027; 95.176 | 71.444; 71.444 |
| mixed-tant-auto | 88175; 88175 | 15.275; 17.601 | 0.616; 0.640 | 3.408; 2.943 | 6.453; 4.009 | 872.014; 955.358 | 51.712; 51.712 | 85.965; 85.965 | 119.914; 120.574 | 0.000; 0.000 |
| mixed-fts-multi | 88175; 88175 | 31.083; 31.598 | 36.230; 38.260 | 165.406; 252.314 | 247.004; 785.807 | 18.745; 10.704 | 116.219; 116.219 | 85.965; 85.965 | 95.555; 95.359 | 118.255; 118.255 |
| mixed-tant-multi | 88175; 88175 | 31.217; 29.287 | 0.920; 0.544 | 12.103; 11.759 | 20.649; 24.046 | 303.950; 356.408 | 83.883; 83.883 | 85.965; 85.965 | 174.469; 170.953 | 0.000; 0.000 |
| mixed-fts-detect | 88175; 88175 | 27.060; 30.811 | 33.780; 39.209 | 115.143; 329.969 | 158.771; 770.836 | 23.793; 11.629 | 69.426; 69.426 | 85.965; 85.965 | 95.262; 95.332 | 71.420; 71.420 |
| mixed-fts-hybrid | 88175; 88175 | 28.009; 33.542 | 31.977; 32.062 | 135.199; 130.040 | 210.271; 178.332 | 22.466; 23.101 | 69.426; 69.426 | 85.965; 85.965 | 95.191; 95.234 | 71.420; 71.420 |
| mixed-tant-hybrid | 88175; 88175 | 60.563; 29.748 | 0.839; 0.635 | 4.856; 3.238 | 6.800; 5.643 | 672.420; 910.162 | 51.712; 51.712 | 85.965; 85.965 | 120.645; 120.465 | 0.000; 0.000 |

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
| mixed-fts-unicode | 0.367; 0.367 | 0.379; 0.379 | 0.470; 0.470 | 2.947; 2.947 | 4.315; 4.315 | 0.448; 0.448 | 0.618; 0.618 |
| mixed-fts-icu | 0.478; 0.478 | 0.484; 0.484 | 0.638; 0.638 | 4.198; 4.198 | 6.002; 6.002 | 0.632; 0.632 | 0.932; 0.932 |
| mixed-tant-icu | 0.476; 0.476 | 0.489; 0.489 | 0.634; 0.634 | 4.007; 4.007 | 5.738; 5.738 | 0.640; 0.640 | 0.929; 0.929 |
| mixed-fts-auto | 0.511; 0.511 | 0.500; 0.500 | 0.697; 0.697 | 4.740; 4.740 | 6.727; 6.727 | 0.636; 0.636 | 0.941; 0.941 |
| mixed-tant-auto | 0.504; 0.504 | 0.505; 0.505 | 0.684; 0.684 | 4.305; 4.305 | 6.178; 6.178 | 0.646; 0.646 | 0.946; 0.946 |
| mixed-fts-multi | 0.511; 0.511 | 0.501; 0.501 | 0.692; 0.692 | 4.725; 4.725 | 6.705; 6.705 | 0.638; 0.638 | 0.942; 0.942 |
| mixed-tant-multi | 0.500; 0.500 | 0.508; 0.508 | 0.672; 0.672 | 4.228; 4.228 | 6.072; 6.072 | 0.650; 0.650 | 0.947; 0.947 |
| mixed-fts-detect | 0.511; 0.511 | 0.500; 0.500 | 0.697; 0.697 | 4.740; 4.740 | 6.727; 6.727 | 0.636; 0.636 | 0.941; 0.941 |
| mixed-fts-hybrid | 0.511; 0.511 | 0.500; 0.500 | 0.697; 0.697 | 4.740; 4.740 | 6.727; 6.727 | 0.636; 0.636 | 0.941; 0.941 |
| mixed-tant-hybrid | 0.504; 0.504 | 0.505; 0.505 | 0.684; 0.684 | 4.305; 4.305 | 6.178; 6.178 | 0.646; 0.646 | 0.946; 0.946 |

Main query timings return IDs, not hydrated source documents. Both engines index full positions/frequencies. Ranked BM25 defaults, field normalization and synonym scoring differ; same analyzer does not imply identical ranks. Unicode61 is an analyzer baseline, not the same token semantics as ICU. Standalone source/index commits in the Tantivy arm are independent. Scale repeats the natural pool and limits to the first 60 queries (RU, over mixed RU/EN/ZH documents); its relevance values are not population-quality measurements.

## Scale build/top-10

| Scenario | Docs | Build s | Query p50 ms | p95 ms | p99 ms | Query/s | Index MiB | Source MiB | Build RSS MiB | Build WAL file MiB |
|---|---|---|---|---|---|---|---|---|---|---|
| mixed-100000-fts-auto | 100000; 100000 | 19.149; 17.725 | 41.987; 41.624 | 110.381; 123.975 | 286.020; 131.031 | 19.041; 19.328 | 81.473; 81.473 | 105.957; 105.957 | 193.188; 193.219 | 84.779; 84.779 |
| mixed-100000-tant-auto | 100000; 100000 | 19.712; 16.742 | 1.297; 1.473 | 3.441; 2.895 | 4.434; 6.587 | 671.321; 674.893 | 62.607; 62.607 | 105.957; 105.957 | 261.480; 219.820 | 0.000; 0.000 |

Main query timings return IDs, not hydrated source documents. Both engines index full positions/frequencies. Ranked BM25 defaults, field normalization and synonym scoring differ; same analyzer does not imply identical ranks. Unicode61 is an analyzer baseline, not the same token semantics as ICU. Standalone source/index commits in the Tantivy arm are independent. Scale repeats the natural pool and limits to the first 60 queries (RU, over mixed RU/EN/ZH documents); its relevance values are not population-quality measurements.

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
| mixed-fts-unicode | insert | 87.012; 79.866 | 0.000; 0.000 | 21.437; 21.437 |
| mixed-fts-unicode | update | 80.976; 127.018 | 0.000; 0.000 | 24.188; 24.188 |
| mixed-fts-unicode | unrelated | 3.580; 7.717 | 0.000; 0.000 | 1.693; 1.693 |
| mixed-fts-unicode | delete | 96.190; 75.432 | 0.000; 0.000 | 22.318; 22.318 |
| mixed-fts-icu | insert | 70.000; 63.176 | 0.000; 0.000 | 20.569; 20.569 |
| mixed-fts-icu | update | 74.173; 69.405 | 0.000; 0.000 | 24.262; 24.262 |
| mixed-fts-icu | unrelated | 4.468; 3.611 | 0.000; 0.000 | 1.693; 1.693 |
| mixed-fts-icu | delete | 63.718; 63.092 | 0.000; 0.000 | 23.096; 23.096 |
| mixed-tant-icu | insert | 5.567; 2.895 | 385.403; 293.875 | 2.039; 2.039 |
| mixed-tant-icu | update | 3.223; 4.261 | 399.496; 412.526 | 1.843; 1.843 |
| mixed-tant-icu | unrelated | 1.953; 4.669 | 0.000; 0.000 | 1.693; 1.693 |
| mixed-tant-icu | delete | 2.070; 5.861 | 56.647; 65.881 | 0.644; 0.644 |
| mixed-fts-auto | insert | 83.470; 83.761 | 0.000; 0.000 | 24.805; 24.805 |
| mixed-fts-auto | update | 92.470; 104.343 | 0.000; 0.000 | 29.524; 29.524 |
| mixed-fts-auto | unrelated | 2.743; 4.620 | 0.000; 0.000 | 1.693; 1.693 |
| mixed-fts-auto | delete | 80.182; 93.050 | 0.000; 0.000 | 26.510; 26.510 |
| mixed-tant-auto | insert | 4.184; 11.189 | 471.648; 424.762 | 2.039; 2.039 |
| mixed-tant-auto | update | 7.947; 8.663 | 942.702; 815.967 | 1.843; 1.843 |
| mixed-tant-auto | unrelated | 12.060; 23.134 | 0.000; 0.000 | 1.693; 1.693 |
| mixed-tant-auto | delete | 11.554; 3.672 | 132.390; 94.168 | 0.644; 0.644 |
| mixed-fts-multi | insert | 96.077; 86.586 | 0.000; 0.000 | 30.683; 30.683 |
| mixed-fts-multi | update | 109.956; 113.243 | 0.000; 0.000 | 29.319; 29.319 |
| mixed-fts-multi | unrelated | 2.421; 3.684 | 0.000; 0.000 | 1.693; 1.693 |
| mixed-fts-multi | delete | 82.730; 73.897 | 0.000; 0.000 | 25.732; 25.732 |
| mixed-tant-multi | insert | 11.107; 4.855 | 480.385; 493.848 | 2.039; 2.039 |
| mixed-tant-multi | update | 11.241; 11.780 | 870.489; 780.401 | 1.843; 1.843 |
| mixed-tant-multi | unrelated | 4.273; 7.591 | 0.000; 0.000 | 1.693; 1.693 |
| mixed-tant-multi | delete | 24.088; 9.011 | 104.008; 56.630 | 0.644; 0.644 |
| mixed-fts-detect | insert | 86.634; 89.810 | 0.000; 0.000 | 24.813; 24.813 |
| mixed-fts-detect | update | 114.879; 127.758 | 0.000; 0.000 | 29.696; 29.696 |
| mixed-fts-detect | unrelated | 2.669; 3.444 | 0.000; 0.000 | 1.693; 1.693 |
| mixed-fts-detect | delete | 88.300; 96.737 | 0.000; 0.000 | 26.451; 26.451 |
| mixed-fts-hybrid | insert | 86.882; 82.072 | 0.000; 0.000 | 24.813; 24.813 |
| mixed-fts-hybrid | update | 129.362; 112.689 | 0.000; 0.000 | 29.696; 29.696 |
| mixed-fts-hybrid | unrelated | 3.561; 1.989 | 0.000; 0.000 | 1.693; 1.693 |
| mixed-fts-hybrid | delete | 87.365; 112.962 | 0.000; 0.000 | 26.451; 26.451 |
| mixed-tant-hybrid | insert | 9.686; 5.184 | 898.096; 523.127 | 2.039; 2.039 |
| mixed-tant-hybrid | update | 12.311; 15.502 | 1648.730; 873.104 | 1.843; 1.843 |
| mixed-tant-hybrid | unrelated | 15.403; 16.194 | 0.000; 0.000 | 1.693; 1.693 |
| mixed-tant-hybrid | delete | 5.333; 8.068 | 97.959; 85.809 | 0.644; 0.644 |

For Tantivy the first latency includes SQLite work and enqueue; its separate durable commits/reloads are the next column, a total over ten commits. Do not quote the first column as complete durable Search write latency or zero SQLite WAL as zero external I/O. The owned-store control below measures complete commit/visibility latency.

## Analyzer sample (every floor(N/5000)-th mixed document, both fields)

| Mode | Input MiB | Emitted tokens | MiB/s | Init ms | Init RSS delta MiB | HWM MiB |
|---|---|---|---|---|---|---|
| icu | 4.084; 4.084 | 557425; 557425 | 11.729; 14.290 | 0.066; 0.116 | 0.387; 0.387 | 89.203; 89.469 |
| auto | 4.084; 4.084 | 928943; 928943 | 8.289; 7.216 | 0.014; 0.151 | 0.371; 0.375 | 89.230; 89.473 |
| multi | 4.084; 4.084 | 1625027; 1625027 | 2.075; 4.010 | 0.044; 0.012 | 0.375; 0.250 | 89.641; 89.508 |
| detect | 4.084; 4.084 | 928943; 928943 | 3.152; 3.530 | 0.041; 0.065 | 0.418; 0.375 | 89.684; 89.598 |
| hybrid | 4.084; 4.084 | 928943; 928943 | 2.870; 3.553 | 0.066; 0.038 | 0.469; 0.188 | 89.672; 89.766 |
| jieba | 4.084; 4.084 | 2349376; 2349376 | 5.258; 4.954 | 234.774; 117.997 | 32.660; 32.578 | 121.066; 121.164 |

## Concurrent SQLite Search: 100k docs, 1000 tx, 10% indexed updates, rollback every 33rd

| Scenario | Indexed ingest s | Write p50 ms | p95 ms | p99 ms | Scheduled write p99 ms | Reader p99 ms each | CPU s | WAL MiB |
|---|---|---|---|---|---|---|---|---|
| fts-auto-100k-readers0 | 34.143; 31.376 | 0.461; 0.432 | 2.387; 1.273 | 4.241; 2.279 | 35.516; 3.809 | ;  | 0.271; 0.204 | 7.862; 7.862 |
| fts-auto-100k-readers2 | 32.809; 34.967 | 0.661; 0.667 | 2.814; 3.154 | 6.092; 6.618 | 171.694; 117.839 | 263.066,208.633; 241.827,231.119 | 2.327; 2.231 | 7.862; 7.862 |

## Authoritative standalone collections: actual stored fields hydrated, 60 queries

| Scenario | Import s | Query p50 ms | p95 ms | p99 ms | Query/s | Backend MiB | WAL MiB | Build RSS MiB | After churn MiB |
|---|---|---|---|---|---|---|---|---|---|
| 100000-fts-owned-hydrated | 28.144; 24.321 | 43.957; 45.689 | 122.968; 109.640 | 135.066; 118.623 | 19.540; 20.208 | 193.434; 193.434 | 194.602; 194.602 | 192.430; 192.516 | 193.434; 193.434 |
| 100000-tant-owned-hydrated | 19.884; 25.230 | 1.399; 1.943 | 3.953; 4.866 | 4.257; 5.758 | 638.808; 450.775 | 116.068; 116.068 | 0.000; 0.000 | 380.152; 384.883 | 150.667; 139.554 |
| 1000000-fts-owned-hydrated | 303.721; 247.646 | 363.173; 402.857 | 1228.455; 1068.141 | 1474.205; 1144.953 | 1.874; 2.227 | 1777.750; 1777.750 | 1788.194; 1788.194 | 1030.852; 1031.035 | 1777.750; 1777.750 |
| 1000000-tant-owned-hydrated | 286.947; 216.555 | 22.816; 7.600 | 61.164; 25.141 | 67.579; 47.364 | 40.243; 105.014 | 1014.096; 1014.094 | 0.000; 0.000 | 2169.383; 2168.777 | 1012.959; 1012.957 |

## Owned-store process memory and validation (includes retained parsed input)

| Scenario | Idle RSS MiB | Loaded RSS MiB | Build RSS MiB | End RSS MiB | Process HWM MiB | Reopen + integrity s | Post-import checkpoint s |
|---|---|---|---|---|---|---|---|
| 100000-fts-owned-hydrated | 3.312; 3.508 | 183.516; 183.711 | 192.430; 192.516 | 193.223; 193.371 | 193.223; 193.371 | 14.485; 13.167 | 1.141; 0.766 |
| 100000-tant-owned-hydrated | 3.395; 3.250 | 183.598; 183.578 | 380.152; 384.883 | 456.914; 440.578 | 458.898; 442.773 | 0.006; 0.009 | 0.000; 0.000 |
| 1000000-fts-owned-hydrated | 3.316; 3.391 | 1018.984; 1019.184 | 1030.852; 1031.035 | 1031.906; 1031.961 | 1031.926; 1031.961 | 136.812; 159.919 | 9.786; 6.611 |
| 1000000-tant-owned-hydrated | 3.441; 3.406 | 1019.125; 1019.074 | 2169.383; 2168.777 | 2454.090; 2439.160 | 2474.219; 2469.137 | 0.008; 0.004 | 0.000; 0.000 |

## Owned-store individual commits: 100 operations/pass, commit and visibility included

| Scenario | Operation | p50 ms | p95 ms | p99 ms | Observed Doc/s |
|---|---|---|---|---|---|
| 100000-fts-owned-hydrated | set | 1.051; 1.017 | 3.614; 4.894 | 22.145; 23.769 | 601.395; 583.734 |
| 100000-fts-owned-hydrated | delete | 1.186; 0.764 | 2.798; 1.507 | 19.945; 20.711 | 653.637; 981.210 |
| 100000-tant-owned-hydrated | set | 24.367; 15.979 | 145.707; 36.397 | 246.224; 52.784 | 25.879; 53.735 |
| 100000-tant-owned-hydrated | delete | 6.225; 4.899 | 48.768; 10.635 | 235.658; 19.347 | 82.075; 178.524 |
| 1000000-fts-owned-hydrated | set | 1.400; 1.260 | 4.102; 3.206 | 26.880; 25.365 | 461.757; 488.234 |
| 1000000-fts-owned-hydrated | delete | 1.336; 1.068 | 3.494; 2.564 | 26.038; 28.046 | 488.229; 589.119 |
| 1000000-tant-owned-hydrated | set | 41.219; 20.555 | 165.387; 45.839 | 213.078; 99.301 | 18.784; 40.675 |
| 1000000-tant-owned-hydrated | delete | 9.437; 4.264 | 19.026; 7.397 | 23.115; 11.297 | 101.339; 215.297 |

## One 100-document commit/pass: observed batch time, not latency quantiles

| Scenario | Operation | Complete commit ms | Amortized Doc/s |
|---|---|---|---|
| 100000-fts-owned-hydrated | set | 50.705; 55.022 | 1972.192; 1817.455 |
| 100000-fts-owned-hydrated | delete | 24.659; 17.979 | 4055.314; 5562.045 |
| 100000-tant-owned-hydrated | set | 85.231; 42.247 | 1173.282; 2367.032 |
| 100000-tant-owned-hydrated | delete | 5.017; 9.922 | 19932.230; 10078.613 |
| 1000000-fts-owned-hydrated | set | 63.434; 57.512 | 1576.442; 1738.768 |
| 1000000-fts-owned-hydrated | delete | 42.092; 20.257 | 2375.748; 4936.565 |
| 1000000-tant-owned-hydrated | set | 78.878; 40.667 | 1267.781; 2458.996 |
| 1000000-tant-owned-hydrated | delete | 6.011; 6.171 | 16636.167; 16204.829 |

The owned-store query sample is the first 60 natural RU queries over mixed RU/EN/ZH documents; it is not a balanced multilingual latency distribution. Mutations change the first 100 RU documents. Bulk import is one durable commit through upsert; incremental batch=1 and batch=100 are distinct workloads. Batch=100 has one sample/pass: its amortized rate is not steady-state throughput. SQLite checkpoints occur outside operation latency; Tantivy uses synchronous commit/reload and its normal background merge behavior. These timings do not establish TinyStore grouped-commit integration or cross-engine transactions.

Quantiles use the recorded nearest upper index ceil((n-1)*p): at 60 queries and 100 mutations p99 equals the observed maximum. These small samples do not estimate a production tail SLO. FTS validation includes a full integrity scan, so its duration must not be compared with Tantivy metadata reopen as ordinary startup cost. Process RSS includes retained input; a low idle-before-load value is not steady-state index RAM.

## Untimed 100k FTS file attribution after churn

| Object | Pages | MiB | Payload MiB | Unused MiB |
|---|---|---|---|---|
| docs_config | 1 | 0.004 | 0.000 | 0.004 |
| docs_content | 26666 | 104.164 | 86.695 | 16.397 |
| docs_data | 20626 | 80.570 | 79.214 | 0.764 |
| docs_docsize | 298 | 1.164 | 0.597 | 0.005 |
| docs_idx | 162 | 0.633 | 0.462 | 0.094 |
| keys | 562 | 2.195 | 1.611 | 0.019 |
| sqlite_autoindex_keys_1 | 609 | 2.379 | 1.865 | 0.220 |
| sqlite_schema | 1 | 0.004 | 0.001 | 0.003 |
| freelist | 594 | 2.320 | — | — |

## Untimed 100k Tantivy file attribution after churn

| File type | MiB |
|---|---|
| .del | 0.002 |
| .fast | 0.003 |
| .fieldnorm | 0.459 |
| .idx | 40.416 |
| .json | 0.005 |
| .lock | 0.000 |
| .pos | 32.678 |
| .store | 54.010 |
| .term | 21.102 |

This additional storage inspection is one structural sample per backend after mutation/restore, not a timing pass or a decomposition of the earlier initial-import totals. Full filenames and SQLite object pages/payload/unused bytes are retained in storage-layout.json. Do not extrapolate its split to one million rows.

## Raw inventory

| File | Records | Nonzero process exits |
|---|---|---|
| analyzer.jsonl | 12 | 0 |
| concurrent.jsonl | 4 | 0 |
| correctness.jsonl | 3 | 1 |
| gold.jsonl | 12 | 0 |
| owned-store.jsonl | 8 | 0 |
| retrieval.jsonl | 54 | 0 |
| scale.jsonl | 4 | 0 |

