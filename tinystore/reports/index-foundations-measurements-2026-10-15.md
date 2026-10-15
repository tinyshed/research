# Follow-up measurement tables — 15 October 2026

Generated with `python3 tinystore/index-foundations/summarize.py`. Values separated by `;` are pass 1; pass 2, never an average. Units and workload groups must be retained when quoting a number. See the architecture reports for boundaries, interpretation and acceptance gates. Raw JSONL contains complete commands, revisions, UTC timestamps, counters and samples.

## Static build and full result-set queries

| Scenario | Build s | Query p50 ms | p95 ms | p99 ms | Query/s | Index MiB¹ | HWM MiB | Build WAL MiB |
|---|---|---|---|---|---|---|---|---|
| 100000-uniform-btree | 0.10; 0.06 | 0.17; 0.15 | 0.62; 0.34 | 1.58; 3.00 | 4436.39; 4361.90 | 2.14; 2.14 | 13.87; 13.54 | 2.16; 2.16 |
| 100000-uniform-postings | 0.21; 0.17 | 1.39; 1.31 | 2.18; 1.99 | 2.85; 3.10 | 669.97; 705.40 | 1.00; 1.00 | 10.82; 10.83 | 1.02; 1.02 |
| 100000-uniform-roaring | 0.04; 0.04 | 0.01; 0.01 | 0.07; 0.04 | 0.50; 0.16 | 29708.85; 49950.05 | 0.96; 0.96 | 11.91; 11.94 | 0.00; 0.00 |
| 100000-uniform-scan-secondary | 0.07; 0.08 | 8.10; 8.03 | 10.88; 10.39 | 13.95; 15.32 | 117.82; 119.92 | 0.00; 0.00 | 11.02; 10.93 | 0.00; 0.00 |
| 100000-uniform-geo | 1.22; 0.66 | 0.25; 0.23 | 41.07; 26.52 | 104.23; 30.85 | 112.11; 170.64 | 5.18; 5.18 | 11.21; 11.04 | 5.22; 5.22 |
| 100000-uniform-grid | 0.29; 0.26 | 0.37; 0.37 | 26.65; 30.61 | 39.79; 76.19 | 160.57; 133.54 | 2.79; 2.79 | 11.21; 11.41 | 2.82; 2.82 |
| 100000-uniform-rstar | 0.09; 0.08 | 0.00; 0.00 | 0.38; 0.40 | 0.78; 0.66 | 10963.71; 11570.95 | 0.00; 0.00 | 20.07; 20.02 | 0.00; 0.00 |
| 100000-uniform-scan-geo | 0.07; 0.07 | 10.73; 11.68 | 17.65; 28.51 | 26.99; 72.28 | 81.39; 67.98 | 0.00; 0.00 | 10.94; 10.73 | 0.00; 0.00 |
| 100000-uniform-fts | 0.43; 0.43 | 0.08; 0.08 | 0.20; 0.19 | 0.45; 0.41 | 9680.36; 9783.97 | 15.27; 15.27 | 12.80; 12.83 | 15.36; 15.36 |
| 100000-uniform-fts-external | 0.34; 0.33 | 0.08; 0.08 | 0.18; 0.14 | 0.38; 0.26 | 9703.02; 10701.37 | 5.00; 5.00 | 12.95; 13.00 | 5.43; 5.43 |
| 100000-uniform-fts-set | 0.31; 0.28 | 0.09; 0.09 | 0.16; 0.18 | 0.30; 0.36 | 10244.75; 9667.48 | 1.89; 1.89 | 12.93; 13.08 | 2.47; 2.47 |
| 100000-uniform-tantivy | 0.51; 0.53 | 0.08; 0.08 | 0.24; 0.17 | 0.55; 0.28 | 8540.91; 9815.22 | 1.93; 1.93 | 50.79; 50.77 | 0.00; 0.00 |
| 100000-uniform-scan-text | 0.07; 0.07 | 29.01; 27.50 | 47.83; 34.08 | 89.22; 41.70 | 31.45; 35.23 | 0.00; 0.00 | 10.73; 10.84 | 0.00; 0.00 |
| 100000-russian-fts | 1.46; 1.83 | 0.13; 0.13 | 0.50; 0.59 | 1.33; 1.09 | 5332.11; 5066.36 | 26.85; 26.85 | 20.38; 20.43 | 27.81; 27.81 |
| 100000-russian-fts-external | 1.22; 1.21 | 0.13; 0.13 | 0.42; 0.46 | 0.54; 0.76 | 6611.29; 6286.24 | 7.62; 7.62 | 20.35; 20.41 | 8.71; 8.71 |
| 100000-russian-fts-set | 1.55; 1.08 | 0.13; 0.13 | 0.45; 0.45 | 0.74; 0.68 | 6262.20; 6305.92 | 3.41; 3.41 | 20.43; 20.29 | 4.13; 4.13 |
| 100000-russian-tantivy | 0.75; 0.70 | 0.12; 0.12 | 0.54; 0.43 | 0.92; 0.84 | 6092.82; 6293.17 | 3.71; 3.71 | 56.72; 57.92 | 0.00; 0.00 |
| 100000-zipf-fts | 0.94; 0.96 | 0.02; 0.02 | 2.34; 2.08 | 3.37; 2.68 | 1427.36; 1579.19 | 30.37; 30.37 | 13.14; 13.12 | 30.98; 30.98 |
| 100000-zipf-fts-external | 0.81; 0.79 | 0.02; 0.02 | 2.10; 2.33 | 2.58; 3.12 | 1564.63; 1483.23 | 9.75; 9.75 | 13.66; 13.57 | 10.48; 10.48 |
| 100000-zipf-fts-set | 0.64; 0.63 | 0.02; 0.02 | 2.35; 2.13 | 3.03; 2.50 | 1464.44; 1539.06 | 3.84; 3.84 | 13.45; 13.29 | 4.28; 4.28 |
| 100000-zipf-tantivy | 0.66; 0.70 | 0.01; 0.01 | 2.23; 2.24 | 2.66; 2.88 | 1507.61; 1499.31 | 3.57; 3.57 | 52.58; 54.32 | 0.00; 0.00 |
| 100000-clustered-geo | 0.62; 0.68 | 1.56; 1.52 | 220.88; 211.30 | 358.20; 279.62 | 18.95; 19.45 | 5.16; 5.16 | 13.98; 13.91 | 5.20; 5.20 |
| 100000-clustered-grid | 0.19; 0.19 | 18.89; 19.42 | 40.87; 43.08 | 43.90; 62.98 | 42.12; 39.55 | 2.82; 2.82 | 13.98; 13.96 | 2.84; 2.84 |
| 100000-clustered-rstar | 0.05; 0.05 | 0.03; 0.02 | 3.32; 3.30 | 5.91; 4.46 | 1258.00; 1316.77 | 0.00; 0.00 | 20.10; 20.04 | 0.00; 0.00 |
| 1000000-uniform-btree | 0.77; 0.81 | 1.89; 1.98 | 3.21; 3.37 | 4.25; 4.39 | 515.53; 497.24 | 21.93; 21.93 | 14.07; 13.79 | 22.06; 22.06 |
| 1000000-uniform-postings | 1.74; 1.78 | 14.28; 14.13 | 17.63; 17.18 | 19.93; 21.39 | 68.89; 69.47 | 10.73; 10.73 | 11.16; 10.93 | 10.81; 10.81 |
| 1000000-uniform-roaring | 0.27; 0.26 | 0.18; 0.17 | 0.59; 0.53 | 1.02; 0.92 | 4123.60; 4431.77 | 9.55; 9.55 | 21.41; 21.58 | 0.00; 0.00 |
| 1000000-uniform-scan-secondary | 0.65; 0.82 | 93.33; 94.17 | 127.52; 164.56 | 214.97; 216.31 | 10.15; 9.71 | 0.00; 0.00 | 11.00; 10.81 | 0.00; 0.00 |
| 1000000-uniform-geo | 16.36; 16.08 | 3.23; 3.14 | 354.59; 359.13 | 408.05; 482.81 | 12.19; 11.90 | 51.35; 51.35 | 14.69; 14.49 | 51.66; 51.66 |
| 1000000-uniform-grid | 4.21; 5.25 | 4.52; 4.74 | 338.25; 343.99 | 386.56; 442.94 | 12.36; 11.85 | 28.87; 28.87 | 14.82; 14.63 | 29.05; 29.05 |
| 1000000-uniform-rstar | 0.59; 0.78 | 0.04; 0.04 | 4.52; 4.43 | 5.97; 6.35 | 991.93; 960.25 | 0.00; 0.00 | 106.47; 106.54 | 0.00; 0.00 |
| 1000000-uniform-scan-geo | 0.67; 0.83 | 118.43; 127.86 | 155.13; 199.07 | 183.17; 342.56 | 8.03; 7.35 | 0.00; 0.00 | 11.48; 11.50 | 0.00; 0.00 |
| 1000000-uniform-fts | 6.26; 5.82 | 0.85; 0.89 | 1.36; 1.61 | 2.51; 2.25 | 1075.58; 1001.95 | 152.66; 152.66 | 13.37; 13.32 | 153.57; 153.57 |
| 1000000-uniform-fts-external | 4.54; 4.18 | 0.85; 0.81 | 1.48; 1.20 | 2.56; 1.72 | 1056.70; 1153.77 | 49.67; 49.67 | 13.18; 13.21 | 50.25; 50.25 |
| 1000000-uniform-fts-set | 3.42; 3.12 | 0.84; 0.85 | 1.30; 1.55 | 1.94; 2.58 | 1090.50; 1046.00 | 18.74; 18.74 | 13.44; 13.09 | 19.79; 19.79 |
| 1000000-uniform-tantivy | 4.11; 4.23 | 1.48; 0.85 | 2.30; 1.74 | 3.98; 2.18 | 611.55; 1013.59 | 18.44; 18.44 | 74.73; 75.61 | 0.00; 0.00 |
| 1000000-uniform-scan-text | 0.72; 0.62 | 307.14; 299.54 | 387.80; 400.49 | 549.10; 573.35 | 3.11; 3.19 | 0.00; 0.00 | 10.84; 10.69 | 0.00; 0.00 |
| 1000000-russian-fts | 15.60; 17.61 | 1.37; 1.26 | 6.14; 4.62 | 11.94; 6.14 | 492.55; 668.18 | 264.29; 264.29 | 20.97; 20.93 | 267.13; 267.13 |
| 1000000-russian-fts-external | 16.30; 17.16 | 1.23; 1.28 | 4.32; 4.40 | 4.97; 5.64 | 703.88; 672.52 | 70.44; 70.44 | 20.65; 20.59 | 72.15; 72.15 |
| 1000000-russian-fts-set | 11.86; 14.84 | 1.28; 1.29 | 4.55; 4.55 | 6.17; 6.15 | 653.87; 651.97 | 29.99; 29.99 | 20.20; 20.09 | 30.28; 30.28 |
| 1000000-russian-tantivy | 9.88; 7.61 | 1.32; 1.79 | 4.82; 7.40 | 7.10; 10.05 | 616.67; 397.16 | 40.63; 39.61 | 100.35; 83.32 | 0.00; 0.00 |
| 1000000-zipf-fts | 13.62; 11.42 | 0.09; 0.09 | 20.34; 23.05 | 23.74; 33.38 | 161.12; 147.78 | 299.21; 299.21 | 15.69; 15.54 | 302.40; 302.40 |
| 1000000-zipf-fts-external | 10.69; 10.19 | 0.09; 0.09 | 21.26; 20.18 | 27.00; 23.06 | 155.48; 160.79 | 93.00; 93.00 | 15.22; 15.31 | 94.98; 94.98 |
| 1000000-zipf-fts-set | 9.96; 8.76 | 0.08; 0.09 | 20.23; 22.08 | 24.02; 25.34 | 160.36; 151.25 | 34.94; 34.94 | 15.27; 15.14 | 35.46; 35.46 |
| 1000000-zipf-tantivy | 6.89; 10.81 | 0.11; 0.10 | 22.26; 21.53 | 32.44; 24.32 | 142.53; 148.49 | 38.44; 38.59 | 98.30; 98.39 | 0.00; 0.00 |
| 1000000-clustered-geo | 13.46; 14.26 | 24.73; 25.11 | 3188.19; 3515.90 | 3903.41; 7647.68 | 1.37; 1.08 | 51.27; 51.27 | 22.37; 22.32 | 51.58; 51.58 |
| 1000000-clustered-grid | 2.23; 2.06 | 242.69; 210.46 | 746.73; 509.24 | 1086.77; 607.00 | 2.87; 3.54 | 28.90; 28.90 | 22.68; 22.82 | 29.08; 29.08 |
| 1000000-clustered-rstar | 0.52; 0.51 | 0.27; 0.26 | 44.66; 47.23 | 52.80; 58.79 | 94.50; 90.89 | 0.00; 0.00 | 106.34; 106.42 | 0.00; 0.00 |

¹ Cache size for Roaring is serialized bitmaps plus score-array estimate, not durable disk. rstar has no standalone durable index bytes; its RAM is in HWM. Index objects exclude source. Native incremental work is excluded from build time, but process HWM spans the entire process. Scans create no secondary projection.

## Native incremental transactions (100 rows/transaction)

| Scenario | Operation | Rows | Batch p99 ms | Physical WAL bytes/row |
|---|---|---|---|---|
| 100000-uniform-btree | insert | 1000 | 9.37; 3.77 | 6999.9; 6999.9 |
| 100000-uniform-btree | update | 1000 | 5.96; 3.32 | 7939.2; 7939.2 |
| 100000-uniform-btree | delete | 1000 | 3.31; 2.47 | 4330.1; 4330.1 |
| 100000-uniform-postings | insert | 1000 | 3.10; 3.19 | 5005.8; 5005.8 |
| 100000-uniform-postings | update | 1000 | 2.27; 2.28 | 4515.5; 4515.5 |
| 100000-uniform-postings | delete | 1000 | 1.63; 2.43 | 4338.4; 4338.4 |
| 100000-uniform-scan-secondary | insert | 1000 | 4.49; 1.92 | 362.6; 362.6 |
| 100000-uniform-scan-secondary | update | 1000 | 3.03; 0.68 | 329.6; 329.6 |
| 100000-uniform-scan-secondary | delete | 1000 | 1.78; 0.67 | 206.0; 206.0 |
| 100000-uniform-geo | insert | 1000 | 9.74; 8.69 | 7926.9; 7926.9 |
| 100000-uniform-geo | update | 1000 | 5.74; 7.54 | 7086.4; 7086.4 |
| 100000-uniform-geo | delete | 1000 | 4.87; 5.97 | 6814.5; 6814.5 |
| 100000-uniform-grid | insert | 1000 | 2.78; 8.48 | 4688.6; 4688.6 |
| 100000-uniform-grid | update | 1000 | 3.59; 3.19 | 4132.4; 4132.4 |
| 100000-uniform-grid | delete | 1000 | 1.88; 4.30 | 3926.4; 3926.4 |
| 100000-uniform-scan-geo | insert | 1000 | 3.04; 4.73 | 362.6; 362.6 |
| 100000-uniform-scan-geo | update | 1000 | 0.93; 1.55 | 329.6; 329.6 |
| 100000-uniform-scan-geo | delete | 1000 | 0.60; 0.87 | 206.0; 206.0 |
| 100000-uniform-fts | insert | 1000 | 42.92; 56.02 | 7576.7; 7576.7 |
| 100000-uniform-fts | update | 1000 | 39.55; 41.52 | 4886.3; 4886.3 |
| 100000-uniform-fts | delete | 1000 | 22.56; 21.43 | 1145.4; 1145.4 |
| 100000-uniform-fts-external | insert | 1000 | 42.50; 42.51 | 7411.9; 7411.9 |
| 100000-uniform-fts-external | update | 1000 | 35.93; 42.17 | 4602.0; 4602.0 |
| 100000-uniform-fts-external | delete | 1000 | 13.71; 13.18 | 1009.4; 1009.4 |
| 100000-uniform-fts-set | insert | 1000 | 33.65; 32.37 | 3341.3; 3341.3 |
| 100000-uniform-fts-set | update | 1000 | 31.86; 39.65 | 3226.0; 3226.0 |
| 100000-uniform-fts-set | delete | 1000 | 18.06; 13.96 | 1067.1; 1067.1 |
| 100000-uniform-scan-text | insert | 1000 | 3.12; 2.86 | 362.6; 362.6 |
| 100000-uniform-scan-text | update | 1000 | 1.04; 1.73 | 329.6; 329.6 |
| 100000-uniform-scan-text | delete | 1000 | 0.52; 1.30 | 206.0; 206.0 |
| 100000-russian-fts | insert | 1000 | 75.26; 79.79 | 12285.8; 12285.8 |
| 100000-russian-fts | update | 1000 | 15.56; 26.51 | 1833.4; 1833.4 |
| 100000-russian-fts | delete | 1000 | 49.63; 65.95 | 8404.8; 8404.8 |
| 100000-russian-fts-external | insert | 1000 | 53.07; 44.23 | 12013.9; 12013.9 |
| 100000-russian-fts-external | update | 1000 | 19.57; 15.68 | 1586.2; 1586.2 |
| 100000-russian-fts-external | delete | 1000 | 90.56; 47.62 | 8095.8; 8095.8 |
| 100000-russian-fts-set | insert | 1000 | 56.24; 59.22 | 6546.7; 6546.7 |
| 100000-russian-fts-set | update | 1000 | 18.76; 12.47 | 1586.2; 1586.2 |
| 100000-russian-fts-set | delete | 1000 | 40.19; 38.14 | 4610.3; 4610.3 |
| 100000-zipf-fts | insert | 1000 | 78.36; 57.01 | 15841.4; 15841.4 |
| 100000-zipf-fts | update | 1000 | 15.68; 13.68 | 2088.8; 2088.8 |
| 100000-zipf-fts | delete | 1000 | 47.28; 49.94 | 11214.6; 11214.6 |
| 100000-zipf-fts-external | insert | 1000 | 59.65; 61.52 | 15618.9; 15618.9 |
| 100000-zipf-fts-external | update | 1000 | 36.06; 15.90 | 1726.3; 1726.3 |
| 100000-zipf-fts-external | delete | 1000 | 102.49; 53.45 | 11025.1; 11025.1 |
| 100000-zipf-fts-set | insert | 1000 | 56.48; 53.84 | 6789.8; 6789.8 |
| 100000-zipf-fts-set | update | 1000 | 14.69; 13.70 | 1837.5; 1837.5 |
| 100000-zipf-fts-set | delete | 1000 | 78.20; 65.26 | 5294.2; 5294.2 |
| 100000-clustered-geo | insert | 1000 | 6.13; 14.64 | 7514.9; 7514.9 |
| 100000-clustered-geo | update | 1000 | 8.39; 6.64 | 7852.7; 7852.7 |
| 100000-clustered-geo | delete | 1000 | 3.27; 5.54 | 6579.6; 6579.6 |
| 100000-clustered-grid | insert | 1000 | 2.55; 3.42 | 593.3; 593.3 |
| 100000-clustered-grid | update | 1000 | 1.23; 2.94 | 445.0; 445.0 |
| 100000-clustered-grid | delete | 1000 | 1.60; 1.07 | 424.4; 424.4 |
| 1000000-uniform-btree | insert | 10000 | 7.45; 11.54 | 7634.4; 7634.4 |
| 1000000-uniform-btree | update | 10000 | 3.42; 7.50 | 8768.6; 8768.6 |
| 1000000-uniform-btree | delete | 10000 | 6.39; 5.92 | 4515.9; 4515.9 |
| 1000000-uniform-postings | insert | 10000 | 3.38; 51.48 | 4875.2; 4875.2 |
| 1000000-uniform-postings | update | 10000 | 3.01; 2.57 | 4517.6; 4517.6 |
| 1000000-uniform-postings | delete | 10000 | 2.50; 157.11 | 4494.9; 4494.9 |
| 1000000-uniform-scan-secondary | insert | 10000 | 3.07; 7.76 | 372.9; 372.9 |
| 1000000-uniform-scan-secondary | update | 10000 | 18.37; 1.67 | 384.8; 384.8 |
| 1000000-uniform-scan-secondary | delete | 10000 | 1.48; 1.84 | 371.6; 371.6 |
| 1000000-uniform-geo | insert | 10000 | 12.37; 18.66 | 8476.5; 8476.5 |
| 1000000-uniform-geo | update | 10000 | 24.60; 23.79 | 10856.2; 10856.2 |
| 1000000-uniform-geo | delete | 10000 | 7.46; 16.62 | 9714.1; 9714.1 |
| 1000000-uniform-grid | insert | 10000 | 4.44; 157.61 | 5097.3; 5097.3 |
| 1000000-uniform-grid | update | 10000 | 15.22; 71.01 | 4534.9; 4534.9 |
| 1000000-uniform-grid | delete | 10000 | 13.15; 4.76 | 4561.3; 4561.3 |
| 1000000-uniform-scan-geo | insert | 10000 | 4.46; 5.95 | 372.9; 372.9 |
| 1000000-uniform-scan-geo | update | 10000 | 2.43; 5.54 | 384.8; 384.8 |
| 1000000-uniform-scan-geo | delete | 10000 | 3.29; 5.16 | 371.6; 371.6 |
| 1000000-uniform-fts | insert | 10000 | 110.93; 138.15 | 7935.5; 7935.5 |
| 1000000-uniform-fts | update | 10000 | 112.52; 141.06 | 5998.3; 5998.3 |
| 1000000-uniform-fts | delete | 10000 | 75.34; 18.55 | 1524.8; 1524.8 |
| 1000000-uniform-fts-external | insert | 10000 | 113.63; 107.13 | 7754.3; 7754.3 |
| 1000000-uniform-fts-external | update | 10000 | 105.11; 94.35 | 5748.2; 5748.2 |
| 1000000-uniform-fts-external | delete | 10000 | 29.53; 45.04 | 1287.9; 1287.9 |
| 1000000-uniform-fts-set | insert | 10000 | 106.40; 101.57 | 4813.8; 4813.8 |
| 1000000-uniform-fts-set | update | 10000 | 89.19; 83.20 | 3295.2; 3295.2 |
| 1000000-uniform-fts-set | delete | 10000 | 31.15; 118.09 | 1283.0; 1283.0 |
| 1000000-uniform-scan-text | insert | 10000 | 4.88; 3.47 | 372.9; 372.9 |
| 1000000-uniform-scan-text | update | 10000 | 2.21; 3.39 | 384.8; 384.8 |
| 1000000-uniform-scan-text | delete | 10000 | 2.52; 4.99 | 371.6; 371.6 |
| 1000000-russian-fts | insert | 10000 | 92.22; 186.94 | 15381.2; 15381.2 |
| 1000000-russian-fts | update | 10000 | 27.63; 46.35 | 2362.4; 2362.4 |
| 1000000-russian-fts | delete | 10000 | 49.50; 26.85 | 2099.1; 2099.1 |
| 1000000-russian-fts-external | insert | 10000 | 230.51; 73.07 | 15122.5; 15122.5 |
| 1000000-russian-fts-external | update | 10000 | 67.87; 27.43 | 2050.9; 2050.9 |
| 1000000-russian-fts-external | delete | 10000 | 25.69; 28.39 | 1850.3; 1850.3 |
| 1000000-russian-fts-set | insert | 10000 | 95.96; 126.11 | 8736.5; 8736.5 |
| 1000000-russian-fts-set | update | 10000 | 25.38; 141.68 | 1912.9; 1912.9 |
| 1000000-russian-fts-set | delete | 10000 | 37.47; 26.86 | 1789.7; 1789.7 |
| 1000000-zipf-fts | insert | 10000 | 76.90; 66.26 | 16559.9; 16559.9 |
| 1000000-zipf-fts | update | 10000 | 132.83; 28.59 | 2284.1; 2284.1 |
| 1000000-zipf-fts | delete | 10000 | 357.38; 31.97 | 2158.9; 2158.9 |
| 1000000-zipf-fts-external | insert | 10000 | 606.27; 68.22 | 16274.8; 16274.8 |
| 1000000-zipf-fts-external | update | 10000 | 193.93; 34.13 | 1931.0; 1931.0 |
| 1000000-zipf-fts-external | delete | 10000 | 30.89; 27.45 | 1907.1; 1907.1 |
| 1000000-zipf-fts-set | insert | 10000 | 136.24; 94.24 | 8141.5; 8141.5 |
| 1000000-zipf-fts-set | update | 10000 | 77.29; 91.67 | 3719.1; 3719.1 |
| 1000000-zipf-fts-set | delete | 10000 | 26.76; 24.24 | 1821.9; 1821.9 |
| 1000000-clustered-geo | insert | 10000 | 13.15; 11.59 | 8009.7; 8009.7 |
| 1000000-clustered-geo | update | 10000 | 13.68; 9.11 | 10806.3; 10806.3 |
| 1000000-clustered-geo | delete | 10000 | 9.27; 8.64 | 9721.1; 9721.1 |
| 1000000-clustered-grid | insert | 10000 | 11.90; 64.08 | 611.8; 611.8 |
| 1000000-clustered-grid | update | 10000 | 5.37; 5.64 | 509.2; 509.2 |
| 1000000-clustered-grid | delete | 10000 | 2.26; 2.48 | 580.5; 580.5 |

## Geo selectivity buckets (75 samples/bucket/pass)

| Scenario | Bucket | Matches | Query p99 ms |
|---|---|---|---|
| 100000-uniform-geo | 0 | 0; 0 | 0.60; 0.38 |
| 100000-uniform-geo | 1 | 0; 0 | 1.35; 0.18 |
| 100000-uniform-geo | 2 | 171; 171 | 1.75; 1.48 |
| 100000-uniform-geo | 3 | 12622; 12622 | 180.55; 38.08 |
| 100000-uniform-grid | 0 | 0; 0 | 0.15; 1.47 |
| 100000-uniform-grid | 1 | 0; 0 | 0.38; 0.78 |
| 100000-uniform-grid | 2 | 171; 171 | 3.90; 5.91 |
| 100000-uniform-grid | 3 | 12622; 12622 | 40.98; 163.38 |
| 100000-uniform-rstar | 0 | 0; 0 | 0.00; 0.03 |
| 100000-uniform-rstar | 1 | 0; 0 | 0.00; 0.03 |
| 100000-uniform-rstar | 2 | 171; 171 | 0.03; 0.07 |
| 100000-uniform-rstar | 3 | 12622; 12622 | 2.14; 1.35 |
| 100000-uniform-scan-geo | 0 | 0; 0 | 150.61; 72.28 |
| 100000-uniform-scan-geo | 1 | 0; 0 | 17.65; 118.53 |
| 100000-uniform-scan-geo | 2 | 171; 171 | 116.75; 41.65 |
| 100000-uniform-scan-geo | 3 | 12622; 12622 | 26.99; 62.11 |
| 100000-clustered-geo | 0 | 0; 0 | 1.17; 0.41 |
| 100000-clustered-geo | 1 | 10; 10 | 0.18; 0.34 |
| 100000-clustered-geo | 2 | 1036; 1036 | 7.59; 8.20 |
| 100000-clustered-geo | 3 | 100000; 100000 | 414.84; 392.49 |
| 100000-clustered-grid | 0 | 0; 0 | 22.07; 141.31 |
| 100000-clustered-grid | 1 | 10; 10 | 23.11; 30.30 |
| 100000-clustered-grid | 2 | 1036; 1036 | 26.73; 26.00 |
| 100000-clustered-grid | 3 | 100000; 100000 | 45.22; 160.03 |
| 100000-clustered-rstar | 0 | 0; 0 | 0.03; 0.00 |
| 100000-clustered-rstar | 1 | 10; 10 | 0.00; 0.00 |
| 100000-clustered-rstar | 2 | 1036; 1036 | 0.10; 0.31 |
| 100000-clustered-rstar | 3 | 100000; 100000 | 7.28; 5.05 |
| 1000000-uniform-geo | 0 | 0; 0 | 0.81; 0.97 |
| 1000000-uniform-geo | 1 | 12; 12 | 1.49; 1.97 |
| 1000000-uniform-geo | 2 | 1530; 1530 | 14.47; 31.64 |
| 1000000-uniform-geo | 3 | 125108; 125108 | 423.09; 506.25 |
| 1000000-uniform-grid | 0 | 0; 0 | 2.35; 1.03 |
| 1000000-uniform-grid | 1 | 12; 12 | 3.10; 1.05 |
| 1000000-uniform-grid | 2 | 1530; 1530 | 47.15; 136.97 |
| 1000000-uniform-grid | 3 | 125108; 125108 | 498.67; 506.82 |
| 1000000-uniform-rstar | 0 | 0; 0 | 0.04; 0.04 |
| 1000000-uniform-rstar | 1 | 12; 12 | 0.01; 0.17 |
| 1000000-uniform-rstar | 2 | 1530; 1530 | 0.17; 0.39 |
| 1000000-uniform-rstar | 3 | 125108; 125108 | 6.94; 10.13 |
| 1000000-uniform-scan-geo | 0 | 0; 0 | 181.16; 430.71 |
| 1000000-uniform-scan-geo | 1 | 12; 12 | 183.17; 401.66 |
| 1000000-uniform-scan-geo | 2 | 1530; 1530 | 157.74; 198.15 |
| 1000000-uniform-scan-geo | 3 | 125108; 125108 | 310.28; 342.56 |
| 1000000-clustered-geo | 0 | 1; 1 | 0.40; 2.22 |
| 1000000-clustered-geo | 1 | 112; 112 | 3.92; 11.77 |
| 1000000-clustered-geo | 2 | 9982; 9982 | 72.62; 167.48 |
| 1000000-clustered-geo | 3 | 1000000; 1000000 | 4727.67; 30260.41 |
| 1000000-clustered-grid | 0 | 1; 1 | 638.07; 359.36 |
| 1000000-clustered-grid | 1 | 112; 112 | 756.06; 427.17 |
| 1000000-clustered-grid | 2 | 9982; 9982 | 596.44; 304.99 |
| 1000000-clustered-grid | 3 | 1000000; 1000000 | 1508.25; 736.58 |
| 1000000-clustered-rstar | 0 | 1; 1 | 0.16; 0.02 |
| 1000000-clustered-rstar | 1 | 112; 112 | 0.07; 0.05 |
| 1000000-clustered-rstar | 2 | 9982; 9982 | 0.82; 1.00 |
| 1000000-clustered-rstar | 3 | 1000000; 1000000 | 63.35; 61.89 |

## rebuilds: writer and build

| Scenario | Rebuild s | Write service p50 ms | p95 | p99 | Writer queue p99 ms | Scheduled p99 ms | Max builder hold ms² |
|---|---|---|---|---|---|---|---|
| 100000-russian-fts-none-0-0-selective-tx1-u10 | 0.00 | 0.90 | 5.57 | 16.60 | 0.00 | 1712.77 | 0.00 |
| 100000-russian-fts-monolithic-0-0-selective-tx1-u10 | 3.08 | 0.84 | 5.03 | 17.80 | 0.00 | 4641.18 | 3081.19 |
| 100000-russian-fts-frontier-1000-32-selective-tx1-u10 | 12.73 | 1.00 | 5.89 | 16.95 | 12.79 | 6116.15 | 61.32 |
| 100000-russian-fts-frontier-5000-256-selective-tx1-u10 | 8.27 | 0.74 | 5.01 | 22.69 | 30.54 | 7197.29 | 80.00 |
| 100000-russian-fts-seen-2000-64-selective-tx1-u10 | 15.51 | 0.96 | 4.85 | 17.39 | 4.30 | 1917.04 | 108.59 |
| 100000-russian-fts-delta-2000-64-selective-tx1-u10 | 10.84 | 0.89 | 3.76 | 20.65 | 10.31 | 3257.02 | 78.59 |
| 100000-russian-fts-frontier-2000-64-selective-tx1-u10 | 12.51 | 1.08 | 6.99 | 19.85 | 20.41 | 6599.21 | 115.63 |
| 100000-russian-fts-frontier-2000-64-full-tx1-u10 | 13.68 | 1.29 | 9.16 | 30.59 | 25.52 | 11363.04 | 71.13 |

## rebuilds: reads and resources

| Scenario | Query/s | Readers p99 ms | Total WAL MiB | Peak logical WAL MiB | Peak DB+WAL MiB | Peak RSS MiB | Activation ms | Time/frame overruns |
|---|---|---|---|---|---|---|---|---|
| 100000-russian-fts-none-0-0-selective-tx1-u10 | 6342.96 | 3.10/4.26 | 52.76 | 33.79 | 107.82 | 17.09 | 0.00 | 0/0 |
| 100000-russian-fts-monolithic-0-0-selective-tx1-u10 | 5161.01 | 1.87/3.63 | 82.18 | 33.43 | 135.14 | 20.73 | 0.00 | 0/0 |
| 100000-russian-fts-frontier-1000-32-selective-tx1-u10 | 6066.93 | 1.45/2.57 | 265.92 | 35.50 | 132.62 | 17.80 | 0.40 | 2067/163 |
| 100000-russian-fts-frontier-5000-256-selective-tx1-u10 | 5143.32 | 2.15/5.59 | 146.91 | 34.32 | 136.87 | 18.81 | 0.24 | 438/9 |
| 100000-russian-fts-seen-2000-64-selective-tx1-u10 | 5720.83 | 2.83/3.62 | 287.64 | 34.48 | 133.81 | 17.83 | 0.62 | 1657/75 |
| 100000-russian-fts-delta-2000-64-selective-tx1-u10 | 5750.36 | 3.19/2.54 | 177.95 | 33.08 | 132.53 | 18.06 | 56.49 | 1014/80 |
| 100000-russian-fts-frontier-2000-64-selective-tx1-u10 | 5532.49 | 4.00/2.06 | 203.32 | 33.55 | 134.85 | 18.21 | 0.61 | 1113/65 |
| 100000-russian-fts-frontier-2000-64-full-tx1-u10 | 6351.93 | 1.36/9.88 | 254.71 | 35.90 | 137.00 | 18.31 | 0.65 | 1091/49 |

² Maximum across setup, chunk/monolithic build, checkpoint and activation holds. Foreground service excludes queue time; scheduled latency includes offered-workload backlog. Fixed main scenarios perform the same minimum 2000 transactions; long builds can outlast that write window. `continuous-*` keeps writes through publication. Different tx sizes, update ratios, caches and distributions are separate scenarios.

## Actual Core lease admission

| Scenario | Grant/s | Attempts/grant | Completed wait p99 ms | Jain fairness | Deadline cancels | CPU s | HWM MiB |
|---|---|---|---|---|---|---|---|

## Real Bun receivers

| Scenario | Publish/s | Delivered | Undelivered at drain | Gap missed | p50 ms | p99 ms | Client subscription delta MiB | Client end RSS MiB | Client CPU s |
|---|---|---|---|---|---|---|---|---|---|

## SQLite source with native term index or durable Tantivy outbox

| Scenario | Build s | Build write p99 ms | Steady write p99 ms | Query p50 ms | Query p99 ms | Scheduled query p99 ms | WAL MiB | External files MiB | HWM MiB |
|---|---|---|---|---|---|---|---|---|---|

## Instrumented WAL/cache/order sensitivity

| Scenario | Build+observer s | WAL frames | Sum unique pages/commit | Total WAL MiB | Peak WAL MiB | Sampled temp MiB |
|---|---|---|---|---|---|---|

## Certified geo same-connection diagnostic (8/bucket, lower median)

| Scenario | Bucket | Matches | Plain ms | Certified ms |
|---|---|---|---|---|

## Large process-crash recovery (oracle validation excluded from resume time)

| Scenario | Resume ms | WAL after resume MiB | Oracle |
|---|---|---|---|

## 64 MiB cgroup / no swap correctness

| Scenario | Exit | OOM killed | Source rows after reopen |
|---|---|---|---|

## Raw inventory

| File | Records | Failed process exits | Revisions |
|---|---|---|---|
| core-sql.jsonl | 2 | 0 | 2f47fb3c3747 |
| correctness.jsonl | 52 | 0 | 2f47fb3c3747, f1856d9b658f |
| crashes.jsonl | 144 | 0 | 2f47fb3c3747, f1856d9b658f |
| lease-correctness.jsonl | 4 | 0 | 14bfbe819eac |
| preparations.jsonl | 13 | 0 | 2f47fb3c3747, 8d4d7b79baaa, b08e6f8df1fc, f1856d9b658f |
| pressure.jsonl | 4 | 0 | 14bfbe819eac |
| rebuilds.jsonl | 8 | 0 | 2f47fb3c3747 |
| static.jsonl | 96 | 0 | f1856d9b658f |
