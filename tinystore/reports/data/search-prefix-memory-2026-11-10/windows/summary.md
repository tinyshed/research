# search-prefix-memory: measure.jsonl, a realloc counted as copy

## Indexes

| index      | documents | words a document | postings   | keys   | rows counting otherwise | search.db | built, s |
|------------|-----------|------------------|------------|--------|-------------------------|-----------|----------|
| 1000000x50 | 1,000,000 | 50               | 42,376,849 | 50,000 | 0                       | 646.5 MiB | 50       |

## Prefixes: the postings the 64 words each begins hold, over every prefix of the index

| index      | letters | read as a prefix | prefixes | least     | median    | p90       | most      |
|------------|---------|------------------|----------|-----------|-----------|-----------|-----------|
| 1000000x50 | 1       | no               | 8        | 2,023,819 | 2,729,442 | 3,763,243 | 4,182,423 |
| 1000000x50 | 2       | yes              | 64       | 193,595   | 381,255   | 1,006,900 | 1,509,713 |
| 1000000x50 | 4       | yes              | 4096     | 204       | 4,536     | 15,368    | 993,479   |

## last 1, no budget: the heap above the same search without

| index      | postings of its words | heap, off | heap, on | above | bytes a posting | SQLite's most, on | reserved, on | answered | ms, off          | ms, on           |
|------------|-----------------------|-----------|----------|-------|-----------------|-------------------|--------------|----------|------------------|------------------|
| 1000000x50 | 4,182,423             | 339 KiB   | 339 KiB  | 0 B   | —               | 64 B              | 0            | ok       | 0.86 (0.54–1.69) | 0.88 (0.56–0.92) |

## last 2, no budget: the heap above the same search without

| index      | postings of its words | heap, off | heap, on | above    | bytes a posting | SQLite's most, on | reserved, on | answered | ms, off          | ms, on              |
|------------|-----------------------|-----------|----------|----------|-----------------|-------------------|--------------|----------|------------------|---------------------|
| 1000000x50 | 1,509,713             | 339 KiB   | 46.6 MiB | 46.3 MiB | 32.1            | 97 KiB            | 0            | ok       | 0.94 (0.71–1.65) | 85.54 (77.60–90.61) |

## last 4, no budget: the heap above the same search without

| index      | postings of its words | heap, off | heap, on | above    | bytes a posting | SQLite's most, on | reserved, on | answered | ms, off          | ms, on              |
|------------|-----------------------|-----------|----------|----------|-----------------|-------------------|--------------|----------|------------------|---------------------|
| 1000000x50 | 993,479               | 339 KiB   | 34.3 MiB | 34.0 MiB | 35.9            | 96 KiB            | 0            | ok       | 1.41 (1.06–2.55) | 52.23 (44.47–83.86) |

## every 1, no budget: the heap above the same search without

| index      | postings of its words | heap, off | heap, on | above | bytes a posting | SQLite's most, on | reserved, on | answered | ms, off          | ms, on           |
|------------|-----------------------|-----------|----------|-------|-----------------|-------------------|--------------|----------|------------------|------------------|
| 1000000x50 | 22,286,782            | 3 KiB     | 3 KiB    | 0 B   | —               | 0 B               | 0            | ok       | 0.03 (0.02–0.10) | 0.04 (0.03–0.09) |

## every 2, no budget: the heap above the same search without

| index      | postings of its words | heap, off | heap, on  | above     | bytes a posting | SQLite's most, on | reserved, on | answered | ms, off          | ms, on                 |
|------------|-----------------------|-----------|-----------|-----------|-----------------|-------------------|--------------|----------|------------------|------------------------|
| 1000000x50 | 10,094,660            | 3 KiB     | 157.1 MiB | 157.1 MiB | 16.3            | 1.8 MiB           | 0            | ok       | 0.15 (0.12–0.29) | 758.91 (725.51–942.57) |

## every 4, no budget: the heap above the same search without

| index      | postings of its words | heap, off | heap, on  | above     | bytes a posting | SQLite's most, on | reserved, on | answered | ms, off          | ms, on                 |
|------------|-----------------------|-----------|-----------|-----------|-----------------|-------------------|--------------|----------|------------------|------------------------|
| 1000000x50 | 5,458,460             | 3 KiB     | 126.7 MiB | 126.7 MiB | 24.3            | 220 KiB           | 0            | ok       | 0.15 (0.11–0.18) | 392.73 (360.09–421.65) |

## 8 common words, whole, no budget

| index      | heap    | SQLite's most | ms                  |
|------------|---------|---------------|---------------------|
| 1000000x50 | 1.6 MiB | 512 B         | 44.19 (35.36–45.93) |

## With a budget of 16.0 MiB: the most heap a search held against it

| index      | search  | heap, most | of the budget | heap without a budget | reserved | answered | ms                     |
|------------|---------|------------|---------------|-----------------------|----------|----------|------------------------|
| 1000000x50 | last 2  | 46.6 MiB   | 2.91          | 46.6 MiB              | 0        | ok       | 73.93 (65.41–86.30)    |
| 1000000x50 | last 4  | 34.3 MiB   | 2.15          | 34.3 MiB              | 0        | ok       | 40.70 (36.94–50.01)    |
| 1000000x50 | every 2 | 157.1 MiB  | 9.82          | 157.1 MiB             | 0        | ok       | 675.75 (598.32–835.56) |
| 1000000x50 | every 4 | 126.7 MiB  | 7.92          | 126.7 MiB             | 0        | ok       | 336.53 (285.64–406.19) |

## Held at rest, every search run once

| index      | heap, no budget | reserved, no budget | heap, 16.0 MiB | reserved, 16.0 MiB |
|------------|-----------------|---------------------|----------------|--------------------|
| 1000000x50 | 5.9 MiB         | 3.9 MiB             | 5.9 MiB        | 3.9 MiB            |
