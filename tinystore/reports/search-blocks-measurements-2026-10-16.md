# Search from blocks: every pass — 16 October 2026

The tables of [search-blocks-2026-10-16](search-blocks-2026-10-16.md), written by
`summarize-blocks.py` from [the raw lines](data/search-blocks-2026-10-16/); a cell is
`round 1; round 2`.

## Builds

| backend | documents | store MiB | objects MiB | build s | postings | blocks | build HWM MiB |
|---|---|---|---|---|---|---|---|
| blocks | 1000000 | 1411 | docs 666, post 495, skip 163, terms 67, sqlite_autoindex_docs_1 20 | 131 | 87964148 | 5735478 | 1497 |
| fts | 1000000 | 1240 | docs 666, idx_data 511, sqlite_autoindex_docs_1 20, idx_docsize 16, idx_idx 2 | 122 |  |  | 728 |
| tantivy | 1000000 | 706 |  | 86 |  |  | 1780 |

## Checks against scoring every posting

| index | plan | cut | questions | differ | blocks opened | blocks, every posting | mean ms | mean ms, every posting |
|---|---|---|---|---|---|---|---|---|
| built | or | 0.05 | 600 | 0 | 229.3 | 1924.6 | 4.68 | 30.42 |
| built | pruned-or | 0.05 | 600 | 0 | 137.3 | 347.2 | 2.40 | 4.47 |
| built | pruned-or | 0.02 | 600 | 0 | 93.4 | 183.0 | 1.56 | 2.28 |
| after the sets | or | 0.05 | 600 | 0 | 298.8 | 1924.6 | 8.27 | 35.29 |
| after the sets | pruned-or | 0.05 | 600 | 0 | 159.7 | 347.2 | 3.55 | 5.11 |

## Queries, 600 questions

| backend | plan | cut | p50 ms | p99 ms | read back p99 ms | queries/s | nDCG@10 | Recall@10 | MRR@10 |
|---|---|---|---|---|---|---|---|---|---|
| blocks | or | - | 2.45; 2.65 | 23.99; 24.61 | 24.04; 24.69 | 227; 216 | 0.4318; 0.4318 | 0.5518; 0.5518 | 0.4720; 0.4720 |
| blocks | pruned-or | 0.02 | 1.24; 1.29 | 6.71; 6.62 | 6.83; 6.68 | 623; 606 | 0.4258; 0.4258 | 0.5488; 0.5488 | 0.4666; 0.4666 |
| blocks | pruned-or | 0.05 | 1.72; 1.73 | 11.49; 10.62 | 11.53; 10.68 | 413; 422 | 0.4312; 0.4312 | 0.5541; 0.5541 | 0.4709; 0.4709 |
| fts | or | - | 102.55; 107.16 | 674.26; 676.72 | 674.34; 676.78 | 5; 5 | 0.4177; 0.4177 | 0.5662; 0.5662 | 0.4326; 0.4326 |
| fts | pruned-or | 0.02 | 17.37; 16.45 | 63.90; 65.54 | 63.95; 65.65 | 48; 51 | 0.4065; 0.4065 | 0.5541; 0.5541 | 0.4163; 0.4163 |
| fts | pruned-or | 0.05 | 32.44; 33.11 | 118.04; 120.60 | 118.10; 120.66 | 25; 24 | 0.4177; 0.4177 | 0.5656; 0.5656 | 0.4336; 0.4336 |
| tantivy | or | - | 0.44; 0.48 | 4.55; 5.07 | 4.60; 5.12 | 1322; 1233 | 0.4353; 0.4353 | 0.5580; 0.5580 | 0.4760; 0.4760 |
| tantivy | pruned-or | 0.02 | 0.20; 0.19 | 1.23; 1.17 | 1.27; 1.20 | 3571; 3732 | 0.4298; 0.4298 | 0.5538; 0.5538 | 0.4706; 0.4706 |
| tantivy | pruned-or | 0.05 | 0.30; 0.30 | 1.84; 1.87 | 1.88; 1.91 | 2343; 2300 | 0.4370; 0.4370 | 0.5642; 0.5642 | 0.4755; 0.4755 |

## Queries after the sets

| backend | plan | cut | p50 ms | p99 ms | read back p99 ms | queries/s | nDCG@10 | Recall@10 | MRR@10 |
|---|---|---|---|---|---|---|---|---|---|
| blocks | or | - | 4.34 | 59.64 | 59.79 | 119 | 0.4318 | 0.5518 | 0.4720 |
| blocks | pruned-or | 0.02 | 1.34 | 8.68 | 8.73 | 534 | 0.4258 | 0.5488 | 0.4666 |
| blocks | pruned-or | 0.05 | 2.22 | 20.95 | 21.04 | 272 | 0.4312 | 0.5541 | 0.4709 |
| fts | or | - | 126.14 | 907.21 | 907.30 | 4 | 0.4176 | 0.5662 | 0.4323 |
| fts | pruned-or | 0.02 | 21.41 | 77.22 | 77.27 | 40 | 0.4065 | 0.5541 | 0.4163 |
| fts | pruned-or | 0.05 | 39.85 | 144.81 | 144.87 | 20 | 0.4178 | 0.5656 | 0.4336 |
| tantivy | or | - | 0.43 | 3.95 | 4.02 | 1434 | 0.4358 | 0.5596 | 0.4762 |
| tantivy | pruned-or | 0.02 | 0.23 | 1.09 | 1.11 | 3383 | 0.4301 | 0.5542 | 0.4707 |
| tantivy | pruned-or | 0.05 | 0.31 | 1.64 | 1.67 | 2401 | 0.4372 | 0.5645 | 0.4755 |

## Query processes

| backend | cut | round | open ms | RSS before open MiB | HWM MiB |
|---|---|---|---|---|---|
| blocks | 0.05 | 1 | 0 | 59 | 68 |
| blocks | 0.02 | 1 | 0 | 59 | 68 |
| fts | 0.05 | 1 | 48 | 59 | 68 |
| fts | 0.02 | 1 | 49 | 59 | 68 |
| tantivy | 0.05 | 1 | 1 | 58 | 323 |
| tantivy | 0.02 | 1 | 0 | 59 | 309 |
| tantivy | 0.05 | 2 | 0 | 59 | 323 |
| tantivy | 0.02 | 2 | 0 | 59 | 309 |
| fts | 0.05 | 2 | 54 | 59 | 68 |
| fts | 0.02 | 2 | 47 | 59 | 68 |
| blocks | 0.05 | 2 | 0 | 59 | 68 |
| blocks | 0.02 | 2 | 0 | 59 | 68 |
| blocks | 0.05 | after | 1 | 59 | 68 |
| blocks | 0.02 | after | 1 | 58 | 68 |
| fts | 0.05 | after | 46 | 59 | 68 |
| fts | 0.02 | after | 48 | 59 | 68 |
| tantivy | 0.05 | after | 1 | 59 | 325 |
| tantivy | 0.02 | after | 1 | 59 | 311 |

## Sets

| backend | round | one set p50 ms | p90 ms | p99 ms | max ms | 100 sets a commit, ms each | HWM MiB |
|---|---|---|---|---|---|---|---|
| blocks | 1 | 4.13 | 5.24 | 9.64 | 9.91 | 43; 47; 62; 56; 78 | 43 |
| fts | 1 | 3.80 | 4.43 | 10.03 | 10.19 | 21; 30; 24; 43; 26 | 43 |
| tantivy | 1 | 66.55 | 98.49 | 117.23 | 118.87 | 91; 81; 80; 82; 80 | 65 |
| tantivy | 2 | 64.30 | 81.35 | 95.05 | 96.51 | 97; 77; 76; 76; 76 | 67 |
| fts | 2 | 3.60 | 4.05 | 9.44 | 10.49 | 33; 48; 40; 29; 51 | 43 |
| blocks | 2 | 4.78 | 12.98 | 15.02 | 16.38 | 72; 77; 86; 61; 79 | 43 |

## Executors, formats and maps, each alternated with the other

### window-queries.jsonl

| executor | plan | cut | p50 ms | p99 ms | nDCG@10 |
|---|---|---|---|---|---|
| cursor | or | - | 2.65; 2.62 | 26.56; 24.64 | 0.4318; 0.4318 |
| cursor | pruned-or | 0.02 | 1.24; 1.30 | 5.93; 7.20 | 0.4258; 0.4258 |
| cursor | pruned-or | 0.05 | 1.88; 1.77 | 11.56; 11.19 | 0.4312; 0.4312 |
| window | or | - | 1.68; 1.68 | 11.43; 11.17 | 0.4318; 0.4318 |
| window | pruned-or | 0.02 | 1.04; 1.00 | 3.89; 3.75 | 0.4259; 0.4259 |
| window | pruned-or | 0.05 | 1.27; 1.28 | 5.16; 5.38 | 0.4312; 0.4312 |

| executor | reader HWM MiB |
|---|---|
| cursor | 68 |
| window | 68 |

### packed-queries.jsonl

| format | plan | cut | p50 ms | p99 ms | nDCG@10 |
|---|---|---|---|---|---|
| packed | or | - | 1.40; 1.57 | 9.41; 10.78 | 0.4318; 0.4318 |
| packed | pruned-or | 0.02 | 1.01; 1.02 | 4.13; 3.92 | 0.4259; 0.4259 |
| packed | pruned-or | 0.05 | 1.16; 1.28 | 5.41; 5.49 | 0.4312; 0.4312 |
| rows | or | - | 1.86; 1.85 | 11.77; 11.88 | 0.4318; 0.4318 |
| rows | pruned-or | 0.02 | 0.90; 1.03 | 3.36; 3.51 | 0.4259; 0.4259 |
| rows | pruned-or | 0.05 | 1.24; 1.30 | 5.20; 5.96 | 0.4312; 0.4312 |

| format | reader HWM MiB |
|---|---|
| packed | 68 |
| rows | 68 |

### mmap-queries.jsonl

| mmap | plan | cut | p50 ms | p99 ms | nDCG@10 |
|---|---|---|---|---|---|
| 0 | or | - | 1.59; 1.57 | 9.95; 9.95 | 0.4318; 0.4318 |
| 0 | pruned-or | 0.02 | 0.98; 0.99 | 3.83; 3.78 | 0.4259; 0.4259 |
| 0 | pruned-or | 0.05 | 1.32; 1.33 | 5.24; 6.12 | 0.4312; 0.4312 |
| 4294967296 | or | - | 1.39; 1.46 | 11.11; 10.72 | 0.4318; 0.4318 |
| 4294967296 | pruned-or | 0.02 | 0.88; 0.86 | 3.56; 3.84 | 0.4259; 0.4259 |
| 4294967296 | pruned-or | 0.05 | 1.15; 1.15 | 5.42; 5.98 | 0.4312; 0.4312 |

| mmap | reader HWM MiB |
|---|---|
| 0 | 68 |
| 4294967296 | 675 |
| 4294967296 | 697 |

### window-checks.jsonl

| executor | plan | cut | differ | blocks opened | mean ms | directories ms | opening ms | every posting ms |
|---|---|---|---|---|---|---|---|---|
| cursor | or | 0.05 | 0 | 229.3 | 4.86 | 0.78 | 1.21 | 32.12 |
| cursor | pruned-or | 0.05 | 0 | 137.3 | 2.51 | 0.39 | 0.72 | 4.35 |
| window | or | 0.05 | 0 | 231.8 | 3.04 | 0.87 | 1.29 | 31.86 |
| window | pruned-or | 0.05 | 0 | 141.9 | 1.73 | 0.38 | 0.72 | 4.50 |

### packed-checks.jsonl

| executor | plan | cut | differ | blocks opened | mean ms | directories ms | opening ms | every posting ms |
|---|---|---|---|---|---|---|---|---|
| cursor | or | 0.05 | 0 | 229.3 | 4.52 | 0.55 | 1.27 | 31.30 |
| cursor | pruned-or | 0.05 | 0 | 137.3 | 2.64 | 0.37 | 0.85 | 4.56 |
| window | or | 0.05 | 0 | 231.8 | 2.84 | 0.61 | 1.34 | 31.11 |
| window | pruned-or | 0.05 | 0 | 141.9 | 1.69 | 0.32 | 0.74 | 4.36 |

