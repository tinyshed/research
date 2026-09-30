# 2026-09-30-wsl2

```text
started 2026-09-29T22:41:41Z
go version go1.27.1 linux/amd64
Redis server v=8.0.2 sha=00000000:0 malloc=jemalloc-5.3.0 bits=64 build=b52b02bf0759f5b4
postgres (PostgreSQL) 17.11 (Debian 17.11-0+deb13u1)
victoria-metrics-20260925-134715-tags-v1.153.0-0-g3acd30be34
6.18.33.2-microsoft-standard-WSL2
model name	: AMD Ryzen 7 7700 8-Core Processor
16
               total        used        free      shared  buff/cache   available
Mem:     16292409344  1430958080 10960052224    27267072  4174249984 14861451264
tinystore d7fb7ff
started 2026-09-29T22:54:42Z
go version go1.27.1 linux/amd64
Redis server v=8.0.2 sha=00000000:0 malloc=jemalloc-5.3.0 bits=64 build=b52b02bf0759f5b4
postgres (PostgreSQL) 17.11 (Debian 17.11-0+deb13u1)
victoria-metrics-20260925-134715-tags-v1.153.0-0-g3acd30be34
6.18.33.2-microsoft-standard-WSL2
model name	: AMD Ryzen 7 7700 8-Core Processor
16
               total        used        free      shared  buff/cache   available
Mem:     16292409344  1264214016 11049582592    27267072  4253421568 15028195328
tinystore d7fb7ff
finished 2026-09-30T01:01:12Z
night finished 2026-09-30T02:07:43Z
started 2026-09-30T03:03:01Z
go version go1.27.1 linux/amd64
Redis server v=8.0.2 sha=00000000:0 malloc=jemalloc-5.3.0 bits=64 build=b52b02bf0759f5b4
postgres (PostgreSQL) 17.11 (Debian 17.11-0+deb13u1)
victoria-metrics-20260925-134715-tags-v1.153.0-0-g3acd30be34
6.18.33.2-microsoft-standard-WSL2
model name	: AMD Ryzen 7 7700 8-Core Processor
16
               total        used        free      shared  buff/cache   available
Mem:     16292409344  1340792832 12032409600    27275264  3179290624 14951616512
tinystore d7fb7ff
finished 2026-09-30T03:07:59Z
```

## blobs-before-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 74.2 (74.1–75.4) | 2353.4 (2135.9–2380.9) | 0.03 (0.02–0.03) | 1 |
| files | 199.1 (194.7–257.7) | 3271.5 (2719.8–3526.7) | 0.00 (0.00–0.00) | 1 |
| sqlite | 147.9 (138.7–161.3) | 1665.2 (1644.0–1674.0) | 0.02 (0.02–0.02) | 1 |

| stage | tinystore | files | sqlite |
|---|---|---|---|
| put-4k×1 | 353 (349–353)/s, p99 6.8 ms, 300 µs CPU/op | 157 (154–160)/s, p99 8.4 ms, 306 µs CPU/op | 444 (441–444)/s, p99 4.7 ms, 203 µs CPU/op |
| get-4k×1 | 66 k (66 k–66 k)/s, p99 115 µs, 21 µs CPU/op | 135 k (135 k–136 k)/s, p99 31 µs, 10 µs CPU/op | 78 k (77 k–78 k)/s, p99 82 µs, 21 µs CPU/op |
| put-4k×64 | 6.3 k (6.2 k–6.4 k)/s, p99 23.1 ms, 77 µs CPU/op | 4.1 k (4.1 k–4.2 k)/s, p99 18.9 ms, 434 µs CPU/op | 456 (453–458)/s, p99 738.2 ms, 195 µs CPU/op |
| get-4k×64 | 90 k (85 k–91 k)/s, p99 2.6 ms, 48 µs CPU/op | 416 k (408 k–427 k)/s, p99 5.2 ms, 15 µs CPU/op | 82 k (77 k–83 k)/s, p99 4.7 ms, 78 µs CPU/op |
| put-1m×1 | 91 (86–93)/s, p99 15.7 ms, 1912 µs CPU/op | 136 (127–139)/s, p99 10.5 ms, 999 µs CPU/op | 154 (153–157)/s, p99 16.8 ms, 2318 µs CPU/op |
| get-1m×1 | 1.7 k (1.7 k–1.7 k)/s, p99 852 µs, 587 µs CPU/op | 7.7 k (6.6 k–7.8 k)/s, p99 524 µs, 287 µs CPU/op | 2.0 k (2.0 k–2.0 k)/s, p99 1.3 ms, 890 µs CPU/op |
| put-1m×8 | 374 (329–377)/s, p99 33.6 ms, 1825 µs CPU/op | 547 (431–602)/s, p99 21.0 ms, 1223 µs CPU/op | 173 (162–174)/s, p99 218.1 ms, 2180 µs CPU/op |
| get-1m×8 | 13 k (13 k–13 k)/s, p99 1.3 ms, 605 µs CPU/op | 14 k (14 k–14 k)/s, p99 1.7 ms, 469 µs CPU/op | 4.1 k (4.1 k–4.1 k)/s, p99 4.7 ms, 1284 µs CPU/op |

## blobs-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 73.7 (73.5–75.0) | 2212.4 (2209.7–2300.5) | 0.02 (0.02–0.03) | 1 |
| sqlite | 296.0 (271.1–306.9) | 1654.4 (1621.9–1669.4) | 0.02 (0.02–0.02) | 1 |

| stage | tinystore | sqlite |
|---|---|---|
| put-4k×1 | 360 (351–362)/s, p99 6.3 ms, 294 µs CPU/op | 443 (430–450)/s, p99 4.7 ms, 199 µs CPU/op |
| get-4k×1 | 67 k (66 k–67 k)/s, p99 115 µs, 21 µs CPU/op | 78 k (77 k–79 k)/s, p99 82 µs, 21 µs CPU/op |
| put-4k×64 | 6.3 k (6.3 k–6.4 k)/s, p99 21.0 ms, 75 µs CPU/op | 465 (461–473)/s, p99 738.2 ms, 196 µs CPU/op |
| get-4k×64 | 92 k (87 k–94 k)/s, p99 2.4 ms, 47 µs CPU/op | 138 k (138 k–141 k)/s, p99 2.9 ms, 54 µs CPU/op |
| put-1m×1 | 91 (89–92)/s, p99 15.7 ms, 1916 µs CPU/op | 154 (153–155)/s, p99 15.7 ms, 2316 µs CPU/op |
| get-1m×1 | 1.7 k (1.7 k–1.7 k)/s, p99 852 µs, 582 µs CPU/op | 2.0 k (1.8 k–2.1 k)/s, p99 1.2 ms, 879 µs CPU/op |
| put-1m×8 | 344 (341–359)/s, p99 33.6 ms, 1869 µs CPU/op | 170 (161–173)/s, p99 218.1 ms, 2276 µs CPU/op |
| get-1m×8 | 13 k (13 k–13 k)/s, p99 1.4 ms, 601 µs CPU/op | 4.2 k (4.2 k–4.4 k)/s, p99 4.7 ms, 1284 µs CPU/op |

## blobs-served

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 74.7 (74.3–75.0) | 2222.3 (2212.1–2309.2) | 0.02 (0.02–0.03) | 1 |
| tinystore-sidecar | 215.4 (212.5–217.3) | 1939.0 (1930.1–1991.2) | 0.05 (0.05–0.05) | 2 |
| tinystore-server | 194.5 (192.9–195.2) | 1963.8 (1940.2–1971.4) | 0.05 (0.05–0.05) | 2 |

| stage | tinystore | tinystore-sidecar | tinystore-server |
|---|---|---|---|
| put-4k×1 | 348 (335–364)/s, p99 6.8 ms, 305 µs CPU/op | 325 (314–331)/s, p99 6.8 ms, 505 µs CPU/op | 319 (318–324)/s, p99 6.8 ms, 489 µs CPU/op |
| get-4k×1 | 68 k (67 k–68 k)/s, p99 115 µs, 21 µs CPU/op | 5.2 k (5.2 k–5.2 k)/s, p99 295 µs, 258 µs CPU/op | 5.2 k (5.2 k–5.2 k)/s, p99 295 µs, 250 µs CPU/op |
| put-4k×64 | 6.3 k (6.2 k–6.4 k)/s, p99 21.0 ms, 76 µs CPU/op | 6.3 k (6.3 k–6.3 k)/s, p99 21.0 ms, 99 µs CPU/op | 6.2 k (6.2 k–6.4 k)/s, p99 21.0 ms, 97 µs CPU/op |
| get-4k×64 | 97 k (86 k–100 k)/s, p99 2.4 ms, 44 µs CPU/op | 148 k (136 k–154 k)/s, p99 1.7 ms, 58 µs CPU/op | 95 k (94 k–95 k)/s, p99 1.8 ms, 70 µs CPU/op |
| put-1m×1 | 90 (88–91)/s, p99 14.7 ms, 1907 µs CPU/op | 83 (80–83)/s, p99 16.8 ms, 4019 µs CPU/op | 82 (82–84)/s, p99 16.8 ms, 4010 µs CPU/op |
| get-1m×1 | 1.7 k (1.7 k–1.7 k)/s, p99 852 µs, 579 µs CPU/op | 1.1 k (1.0 k–1.1 k)/s, p99 1.4 ms, 2324 µs CPU/op | 904 (894–907)/s, p99 1.6 ms, 2306 µs CPU/op |
| put-1m×8 | 344 (343–364)/s, p99 37.7 ms, 1862 µs CPU/op | 291 (287–300)/s, p99 37.7 ms, 4368 µs CPU/op | 294 (290–295)/s, p99 41.9 ms, 4265 µs CPU/op |
| get-1m×8 | 13 k (13 k–13 k)/s, p99 1.3 ms, 594 µs CPU/op | 1.2 k (1.1 k–1.2 k)/s, p99 13.6 ms, 2938 µs CPU/op | 1.9 k (1.9 k–1.9 k)/s, p99 9.4 ms, 2102 µs CPU/op |

## blobs

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 74.2 (74.1–75.4) | 2353.4 (2135.9–2380.9) | 0.03 (0.02–0.03) | 1 |
| files | 199.1 (194.7–257.7) | 3271.5 (2719.8–3526.7) | 0.00 (0.00–0.00) | 1 |
| sqlite | 296.0 (271.1–306.9) | 1654.4 (1621.9–1669.4) | 0.02 (0.02–0.02) | 1 |

| stage | tinystore | files | sqlite |
|---|---|---|---|
| put-4k×1 | 353 (349–353)/s, p99 6.8 ms, 300 µs CPU/op | 157 (154–160)/s, p99 8.4 ms, 306 µs CPU/op | 443 (430–450)/s, p99 4.7 ms, 199 µs CPU/op |
| get-4k×1 | 66 k (66 k–66 k)/s, p99 115 µs, 21 µs CPU/op | 135 k (135 k–136 k)/s, p99 31 µs, 10 µs CPU/op | 78 k (77 k–79 k)/s, p99 82 µs, 21 µs CPU/op |
| put-4k×64 | 6.3 k (6.2 k–6.4 k)/s, p99 23.1 ms, 77 µs CPU/op | 4.1 k (4.1 k–4.2 k)/s, p99 18.9 ms, 434 µs CPU/op | 465 (461–473)/s, p99 738.2 ms, 196 µs CPU/op |
| get-4k×64 | 90 k (85 k–91 k)/s, p99 2.6 ms, 48 µs CPU/op | 416 k (408 k–427 k)/s, p99 5.2 ms, 15 µs CPU/op | 138 k (138 k–141 k)/s, p99 2.9 ms, 54 µs CPU/op |
| put-1m×1 | 91 (86–93)/s, p99 15.7 ms, 1912 µs CPU/op | 136 (127–139)/s, p99 10.5 ms, 999 µs CPU/op | 154 (153–155)/s, p99 15.7 ms, 2316 µs CPU/op |
| get-1m×1 | 1.7 k (1.7 k–1.7 k)/s, p99 852 µs, 587 µs CPU/op | 7.7 k (6.6 k–7.8 k)/s, p99 524 µs, 287 µs CPU/op | 2.0 k (1.8 k–2.1 k)/s, p99 1.2 ms, 879 µs CPU/op |
| put-1m×8 | 374 (329–377)/s, p99 33.6 ms, 1825 µs CPU/op | 547 (431–602)/s, p99 21.0 ms, 1223 µs CPU/op | 170 (161–173)/s, p99 218.1 ms, 2276 µs CPU/op |
| get-1m×8 | 13 k (13 k–13 k)/s, p99 1.3 ms, 605 µs CPU/op | 14 k (14 k–14 k)/s, p99 1.7 ms, 469 µs CPU/op | 4.2 k (4.2 k–4.4 k)/s, p99 4.7 ms, 1284 µs CPU/op |

## crash

```json
[
 {
  "contender": "tinystore",
  "cycles": [
   {
    "acknowledged": 4977,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.000893814
   },
   {
    "acknowledged": 2698,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.001360627
   },
   {
    "acknowledged": 1677,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.002225565
   },
   {
    "acknowledged": 3120,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.001105517
   },
   {
    "acknowledged": 3889,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.001817896
   },
   {
    "acknowledged": 3660,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.002196901
   },
   {
    "acknowledged": 2397,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.001563274
   },
   {
    "acknowledged": 5118,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.001990746
   },
   {
    "acknowledged": 2273,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.001599144
   },
   {
    "acknowledged": 2843,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.002652048
   },
   {
    "acknowledged": 4144,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.002384913
   },
   {
    "acknowledged": 2763,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.002573471
   },
   {
    "acknowledged": 3419,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.00214195
   },
   {
    "acknowledged": 3434,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.002063624
   },
   {
    "acknowledged": 4291,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.002763709
   },
   {
    "acknowledged": 2550,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.002622903
   },
   {
    "acknowledged": 2295,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.002522055
   },
   {
    "acknowledged": 2063,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.001733011
   },
   {
    "acknowledged": 2943,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.001351473
   },
   {
    "acknowledged": 5113,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.002514644
   }
  ]
 },
 {
  "contender": "tinystore-sidecar",
  "cycles": [
   {
    "acknowledged": 3000,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.023963541
   },
   {
    "acknowledged": 3112,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.023759471
   },
   {
    "acknowledged": 3012,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.024472429
   },
   {
    "acknowledged": 2033,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.02311187
   },
   {
    "acknowledged": 2003,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.023108261
   },
   {
    "acknowledged": 1731,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.022692734
   },
   {
    "acknowledged": 3697,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.023578493
   },
   {
    "acknowledged": 1537,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.024062893
   },
   {
    "acknowledged": 3060,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.02372839
   },
   {
    "acknowledged": 3175,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.02229244
   },
   {
    "acknowledged": 1489,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.024536079
   },
   {
    "acknowledged": 3426,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.024663698
   },
   {
    "acknowledged": 3286,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.023773641
   },
   {
    "acknowledged": 1273,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.024006356
   },
   {
    "acknowledged": 2958,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.024062558
   },
   {
    "acknowledged": 2194,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.023729262
   },
   {
    "acknowledged": 2855,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.023833242
   },
   {
    "acknowledged": 1536,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.024498204
   },
   {
    "acknowledged": 2173,
    "lost": 0,
    "wrong": 0,
    "open_seconds": 0.022904521
   
```

## jobs-served

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 54.0 (53.8–54.1) | 5.7 (5.6–5.8) | 0.02 (0.02–0.02) | 1 |
| tinystore-sidecar | 87.3 (87.0–88.0) | 5.3 (4.6–5.4) | 0.05 (0.04–0.05) | 2 |
| tinystore-server | 86.9 (86.7–86.9) | 5.3 (5.3–5.5) | 0.05 (0.05–0.05) | 2 |

| stage | tinystore | tinystore-sidecar | tinystore-server |
|---|---|---|---|
| enqueue×1 | 359 (353–366)/s, p99 4.2 ms, 240 µs CPU/op | 333 (328–334)/s, p99 5.2 ms, 426 µs CPU/op | 328 (325–330)/s, p99 5.2 ms, 402 µs CPU/op |
| enqueue×8 | 1.6 k (1.6 k–1.6 k)/s, p99 10.5 ms, 69 µs CPU/op | 1.4 k (1.4 k–1.4 k)/s, p99 10.5 ms, 143 µs CPU/op | 1.4 k (1.4 k–1.4 k)/s, p99 10.5 ms, 148 µs CPU/op |
| enqueue×64 | 10 k (10.0 k–10 k)/s, p99 11.5 ms, 22 µs CPU/op | 9.7 k (7.8 k–9.8 k)/s, p99 13.6 ms, 37 µs CPU/op | 9.7 k (9.7 k–10.0 k)/s, p99 11.5 ms, 37 µs CPU/op |
| drain×8 | 2.7 k (2.4 k–2.8 k)/s, p99 0 µs, 57 µs CPU/op | 1.4 k (1.3 k–1.4 k)/s, p99 0 µs, 130 µs CPU/op | 1.4 k (1.4 k–1.4 k)/s, p99 0 µs, 147 µs CPU/op |

## jobs

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 54.1 (54.0–54.2) | 5.7 (5.7–5.8) | 0.02 (0.02–0.02) | 1 |
| goqite | 54.5 (53.7–54.8) | 1.6 (1.6–1.7) | 0.02 (0.02–0.02) | 1 |

| stage | tinystore | goqite |
|---|---|---|
| enqueue×1 | 365 (355–366)/s, p99 4.7 ms, 252 µs CPU/op | 363 (362–370)/s, p99 4.7 ms, 275 µs CPU/op |
| enqueue×8 | 1.6 k (1.6 k–1.7 k)/s, p99 9.4 ms, 70 µs CPU/op | 357 (356–366)/s, p99 92.3 ms, 284 µs CPU/op |
| enqueue×64 | 10 k (10 k–10 k)/s, p99 12.6 ms, 22 µs CPU/op | 356 (355–363)/s, p99 805.3 ms, 282 µs CPU/op |
| drain×8 | 2.8 k (2.8 k–2.8 k)/s, p99 0 µs, 57 µs CPU/op | 181 (180–182)/s, p99 0 µs, 537 µs CPU/op |

## kv-before-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 78.9 (78.2–78.9) | 16.8 (16.7–16.8) | 0.02 (0.02–0.03) | 1 |
| tinystore-sidecar | 113.8 (113.8–114.4) | 16.7 (16.7–16.8) | 0.04 (0.04–0.04) | 2 |
| tinystore-server | 112.7 (112.5–113.5) | 17.0 (16.8–17.0) | 0.05 (0.05–0.05) | 2 |
| sqlite | 150.9 (141.6–155.9) | 17.3 (17.2–17.3) | 0.02 (0.02–0.02) | 1 |
| bbolt | 85.0 (84.6–85.2) | 48.0 | 0.01 (0.01–0.01) | 1 |
| bbolt-borrowed | 85.0 (85.0–85.2) | 48.0 | 0.01 (0.01–0.01) | 1 |
| badger | 182.3 (182.3–255.8) | 17.6 (17.6–17.6) | 0.02 (0.02–0.02) | 1 |
| pebble | 65.0 (64.2–65.3) | 29.7 (29.7–29.7) | 0.02 (0.02–0.02) | 1 |
| redis | 95.0 (93.9–95.3) | 28.2 (27.6–28.3) | 0.10 (0.10–0.10) | 2 |
| redis-tcp | 94.1 (93.7–94.6) | 28.4 (28.3–28.4) | 0.10 (0.10–0.10) | 2 |

| stage | tinystore | tinystore-sidecar | tinystore-server | sqlite | bbolt | bbolt-borrowed | badger | pebble | redis | redis-tcp |
|---|---|---|---|---|---|---|---|---|---|---|
| get×1 | 116 k (116 k–118 k)/s, p99 27 µs, 10 µs CPU/op | 5.5 k (5.4 k–5.5 k)/s, p99 246 µs, 240 µs CPU/op | 5.7 k (5.7 k–5.7 k)/s, p99 246 µs, 222 µs CPU/op | 95 k (93 k–96 k)/s, p99 37 µs, 15 µs CPU/op | 1.02 M (1.00 M–1.04 M)/s, p99 4 µs, 1 µs CPU/op | 1.10 M (1.07 M–1.12 M)/s, p99 3 µs, 1 µs CPU/op | 647 k (634 k–659 k)/s, p99 9 µs, 2 µs CPU/op | 162 k (156 k–163 k)/s, p99 14 µs, 6 µs CPU/op | 14 k (14 k–14 k)/s, p99 123 µs, 88 µs CPU/op | 12 k (12 k–12 k)/s, p99 147 µs, 93 µs CPU/op |
| get×8 | 314 k (313 k–314 k)/s, p99 360 µs, 14 µs CPU/op | 47 k (45 k–47 k)/s, p99 328 µs, 74 µs CPU/op | 37 k (37 k–38 k)/s, p99 426 µs, 98 µs CPU/op | 125 k (123 k–129 k)/s, p99 655 µs, 28 µs CPU/op | 1.04 M (1.01 M–1.05 M)/s, p99 213 µs, 2 µs CPU/op | 1.14 M (1.10 M–1.15 M)/s, p99 197 µs, 2 µs CPU/op | 891 k (887 k–893 k)/s, p99 295 µs, 2 µs CPU/op | 316 k (310 k–351 k)/s, p99 229 µs, 9 µs CPU/op | 68 k (67 k–68 k)/s, p99 721 µs, 58 µs CPU/op | 64 k (63 k–64 k)/s, p99 492 µs, 66 µs CPU/op |
| get×64 | 199 k (196 k–200 k)/s, p99 2.4 ms, 21 µs CPU/op | 273 k (266 k–273 k)/s, p99 1.0 ms, 34 µs CPU/op | 142 k (113 k–143 k)/s, p99 1.3 ms, 45 µs CPU/op | 127 k (126 k–131 k)/s, p99 3.1 ms, 47 µs CPU/op | 1.08 M (1.08 M–1.12 M)/s, p99 1.4 ms, 3 µs CPU/op | 1.12 M (1.03 M–1.13 M)/s, p99 1.6 ms, 2 µs CPU/op | 927 k (915 k–930 k)/s, p99 2.1 ms, 3 µs CPU/op | 497 k (496 k–549 k)/s, p99 721 µs, 10 µs CPU/op | 182 k (178 k–187 k)/s, p99 2.1 ms, 26 µs CPU/op | 149 k (147 k–150 k)/s, p99 1.7 ms, 33 µs CPU/op |
| set×1 | 362 (352–369)/s, p99 4.2 ms, 277 µs CPU/op | 328 (244–328)/s, p99 6.8 ms, 451 µs CPU/op | 326 (267–327)/s, p99 4.2 ms, 442 µs CPU/op | 447 (426–450)/s, p99 4.2 ms, 183 µs CPU/op | 75 (75–75)/s, p99 14.7 ms, 347 µs CPU/op | 75 (75–75)/s, p99 14.7 ms, 319 µs CPU/op | 474 (404–479)/s, p99 3.9 ms, 135 µs CPU/op | 323 (317–326)/s, p99 3.9 ms, 143 µs CPU/op | 304 (302–305)/s, p99 4.2 ms, 204 µs CPU/op | 302 (301–310)/s, p99 4.7 ms, 185 µs CPU/op |
| set×8 | 1.5 k (1.5 k–1.5 k)/s, p99 12.6 ms, 102 µs CPU/op | 1.3 k (1.2 k–1.3 k)/s, p99 14.7 ms, 183 µs CPU/op | 1.3 k (1.2 k–1.3 k)/s, p99 14.7 ms, 189 µs CPU/op | 491 (474–497)/s, p99 83.9 ms, 179 µs CPU/op | 593 (589–593)/s, p99 16.8 ms, 51 µs CPU/op | 595 (595–596)/s, p99 15.7 ms, 50 µs CPU/op | 623 (613–626)/s, p99 25.2 ms, 112 µs CPU/op | 1.3 k (1.3 k–1.3 k)/s, p99 8.4 ms, 48 µs CPU/op | 1.2 k (1.2 k–1.2 k)/s, p99 8.4 ms, 113 µs CPU/op | 1.2 k (1.2 k–1.3 k)/s, p99 7.9 ms, 116 µs CPU/op |
| set×64 | 7.7 k (7.7 k–7.7 k)/s, p99 23.1 ms, 47 µs CPU/op | 7.3 k (7.2 k–7.3 k)/s, p99 21.0 ms, 59 µs CPU/op | 7.1 k (7.1 k–7.2 k)/s, p99 23.1 ms, 63 µs CPU/op | 511 (496–521)/s, p99 671.1 ms, 175 µs CPU/op | 4.4 k (4.4 k–4.4 k)/s, p99 15.7 ms, 21 µs CPU/op | 4.4 k (4.4 k–4.4 k)/s, p99 16.8 ms, 21 µs CPU/op | 683 (642–693)/s, p99 125.8 ms, 92 µs CPU/op | 20 k (20 k–20 k)/s, p99 5.8 ms, 9 µs CPU/op | 9.4 k (8.8 k–9.4 k)/s, p99 8.4 ms, 57 µs CPU/op | 9.5 k (9.5 k–9.5 k)/s, p99 9.4 ms, 66 µs CPU/op |
| mixed×1 | 3.3 k (3.2 k–3.4 k)/s, p99 3.4 ms, 41 µs CPU/op | 2.0 k (2.0 k–2.0 k)/s, p99 3.7 ms, 298 µs CPU/op | 2.0 k (1.7 k–2.0 k)/s, p99 3.7 ms, 269 µs CPU/op | 4.2 k (4.2 k–4.2 k)/s, p99 3.4 ms, 37 µs CPU/op | 752 (751–756)/s, p99 13.6 ms, 32 µs CPU/op | 755 (749–755)/s, p99 13.6 ms, 29 µs CPU/op | 4.9 k (4.9 k–4.9 k)/s, p99 2.4 ms, 17 µs CPU/op | 6.0 k (6.0 k–6.1 k)/s, p99 2.0 ms, 22 µs CPU/op | 2.6 k (2.5 k–2.6 k)/s, p99 3.7 ms, 106 µs CPU/op | 2.5 k (2.5 k–2.5 k)/s, p99 3.4 ms, 107 µs CPU/op |
| mixed×8 | 13 k (13 k–13 k)/s, p99 7.3 ms, 25 µs CPU/op | 12 k (11 k–12 k)/s, p99 6.8 ms, 138 µs CPU/op | 11 k (11 k–11 k)/s, p99 6.8 ms, 145 µs CPU/op | 4.8 k (4.7 k–5.0 k)/s, p99 41.9 ms, 41 µs CPU/op | 5.8 k (5.8 k–5.9 k)/s, p99 14.7 ms, 8 µs CPU/op | 5.8 k (5.8 k–5.9 k)/s, p99 14.7 ms, 8 µs CPU/op | 5.0 k (5.0 k–5.0 k)/s, p99 9.4 ms, 21 µs CPU/op | 26 k (26 k–26 k)/s, p99 3.1 ms, 18 µs CPU/op | 3.3 k (3.3 k–3.3 k)/s, p99 7.3 ms, 94 µs CPU/op | 3.3 k (3.3 k–3.3 k)/s, p99 7.3 ms, 99 µs CPU/op |
| mixed×64 | 63 k (63 k–63 k)/s, p99 11.5 ms, 24 µs CPU/op | 63 k (63 k–63 k)/s, p99 10.5 ms, 57 µs CPU/op | 61 k (60 k–61 k)/s, p99 9.4 ms, 70 µs CPU/op | 5.2 k (5.1 k–5.3 k)/s, p99 335.5 ms, 41 µs CPU/op | 43 k (42 k–43 k)/s, p99 15.7 ms, 5 µs CPU/op | 43 k (43 k–43 k)/s, p99 15.7 ms, 5 µs CPU/op | 6.4 k (6.3 k–6.4 k)/s, p99 29.4 ms, 18 µs CPU/op | 197 k (196 k–197 k)/s, p99 3.4 ms, 14 µs CPU/op | 10 k (10 k–10 k)/s, p99 9.4 ms, 46 µs CPU/op | 10 k (10 k–10 k)/s, p99 7.9 ms, 62 µs CPU/op |

## kv-cold-before-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 59.5 (59.5–59.8) | 16.7 (16.6–16.7) | 0.02 (0.02–0.02) | 1 |
| sqlite | 54.2 (53.8–54.5) | 17.3 (17.2–17.3) | 0.02 (0.02–0.02) | 1 |
| bbolt | 78.2 (78.0–78.4) | 48.0 | 0.01 (0.01–0.01) | 1 |
| badger | 231.9 (227.4–234.7) | 15.6 (15.6–15.6) | 0.03 (0.02–0.03) | 1 |
| pebble | 69.6 (62.0–70.5) | 13.7 (13.7–13.7) | 0.03 (0.03–0.03) | 1 |

| stage | tinystore | sqlite | bbolt | badger | pebble |
|---|---|---|---|---|---|
| get-cold×1 | 47 k (45 k–52 k)/s, p99 246 µs, 14 µs CPU/op | 36 k (36 k–39 k)/s, p99 295 µs, 26 µs CPU/op | 19 k (19 k–22 k)/s, p99 229 µs, 10 µs CPU/op | 346 k (262 k–349 k)/s, p99 11 µs, 4 µs CPU/op | 54 k (53 k–55 k)/s, p99 164 µs, 7 µs CPU/op |
| get-warm×1 | 104 k (99 k–106 k)/s, p99 29 µs, 10 µs CPU/op | 86 k (86 k–87 k)/s, p99 37 µs, 17 µs CPU/op | 988 k (984 k–1.02 M)/s, p99 4 µs, 1 µs CPU/op | 383 k (334 k–396 k)/s, p99 12 µs, 4 µs CPU/op | 311 k (305 k–355 k)/s, p99 9 µs, 4 µs CPU/op |

## kv-cold-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 59.3 (59.2–59.6) | 16.7 (16.7–16.7) | 0.02 (0.02–0.02) | 1 |
| sqlite | 54.1 (53.9–54.4) | 17.2 (17.2–17.3) | 0.02 (0.02–0.02) | 1 |

| stage | tinystore | sqlite |
|---|---|---|
| get-cold×1 | 44 k (44 k–47 k)/s, p99 262 µs, 15 µs CPU/op | 35 k (34 k–40 k)/s, p99 328 µs, 26 µs CPU/op |
| get-warm×1 | 100 k (100 k–108 k)/s, p99 29 µs, 11 µs CPU/op | 85 k (80 k–87 k)/s, p99 41 µs, 17 µs CPU/op |

## kv-cold

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 59.5 (59.5–59.8) | 16.7 (16.6–16.7) | 0.02 (0.02–0.02) | 1 |
| bbolt | 78.2 (78.0–78.4) | 48.0 | 0.01 (0.01–0.01) | 1 |
| badger | 231.9 (227.4–234.7) | 15.6 (15.6–15.6) | 0.03 (0.02–0.03) | 1 |
| pebble | 69.6 (62.0–70.5) | 13.7 (13.7–13.7) | 0.03 (0.03–0.03) | 1 |
| sqlite | 54.1 (53.9–54.4) | 17.2 (17.2–17.3) | 0.02 (0.02–0.02) | 1 |

| stage | tinystore | bbolt | badger | pebble | sqlite |
|---|---|---|---|---|---|
| get-cold×1 | 47 k (45 k–52 k)/s, p99 246 µs, 14 µs CPU/op | 19 k (19 k–22 k)/s, p99 229 µs, 10 µs CPU/op | 346 k (262 k–349 k)/s, p99 11 µs, 4 µs CPU/op | 54 k (53 k–55 k)/s, p99 164 µs, 7 µs CPU/op | 35 k (34 k–40 k)/s, p99 328 µs, 26 µs CPU/op |
| get-warm×1 | 104 k (99 k–106 k)/s, p99 29 µs, 10 µs CPU/op | 988 k (984 k–1.02 M)/s, p99 4 µs, 1 µs CPU/op | 383 k (334 k–396 k)/s, p99 12 µs, 4 µs CPU/op | 311 k (305 k–355 k)/s, p99 9 µs, 4 µs CPU/op | 85 k (80 k–87 k)/s, p99 41 µs, 17 µs CPU/op |

## kv-latency-before-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 106.8 | 16.6 | 0.02 | 1 |
| sqlite | 91.0 | 17.2 | 0.02 | 1 |
| bbolt | 99.6 | 48.0 | 0.01 | 1 |
| badger | 190.3 | 18.0 | 0.02 | 1 |
| pebble | 102.1 | 29.7 | 0.02 | 1 |
| redis | 125.2 | 25.0 | 0.10 | 2 |

| stage | tinystore | sqlite | bbolt | badger | pebble | redis |
|---|---|---|---|---|---|---|
| mixed-max×64 | 61 k/s, p99 11.5 ms, 25 µs CPU/op | 4.1 k/s, p99 402.7 ms, 43 µs CPU/op | 43 k/s, p99 15.7 ms, 5 µs CPU/op | 5.9 k/s, p99 31.5 ms, 18 µs CPU/op | 196 k/s, p99 3.4 ms, 14 µs CPU/op | 10 k/s, p99 9.4 ms, 47 µs CPU/op |
| mixed-25%×1024 | 15 k/s, p99 7.3 ms, 42 µs CPU/op | 1.0 k/s, p99 5.8 ms, 132 µs CPU/op | 11 k/s, p99 14.7 ms, 12 µs CPU/op | 1.5 k/s, p99 5.2 ms, 53 µs CPU/op | 49 k/s, p99 3.7 ms, 23 µs CPU/op | 2.5 k/s, p99 12.6 ms, 101 µs CPU/op |
| mixed-50%×1024 | 31 k/s, p99 7.9 ms, 39 µs CPU/op | 2.0 k/s, p99 7.3 ms, 97 µs CPU/op | 22 k/s, p99 14.7 ms, 10 µs CPU/op | 3.0 k/s, p99 9.4 ms, 36 µs CPU/op | 98 k/s, p99 3.7 ms, 19 µs CPU/op | 5.1 k/s, p99 9.4 ms, 83 µs CPU/op |
| mixed-75%×1024 | 46 k/s, p99 12.6 ms, 37 µs CPU/op | 3.0 k/s, p99 21.0 ms, 95 µs CPU/op | 32 k/s, p99 14.7 ms, 8 µs CPU/op | 4.4 k/s, p99 21.0 ms, 30 µs CPU/op | 147 k/s, p99 3.7 ms, 18 µs CPU/op | 7.6 k/s, p99 8.4 ms, 67 µs CPU/op |
| mixed-90%×1024 | 55 k/s, p99 13.6 ms, 35 µs CPU/op | 3.6 k/s, p99 125.8 ms, 97 µs CPU/op | 39 k/s, p99 15.7 ms, 8 µs CPU/op | 5.3 k/s, p99 75.5 ms, 27 µs CPU/op | 177 k/s, p99 3.7 ms, 17 µs CPU/op | 9.1 k/s, p99 15.7 ms, 58 µs CPU/op |

## kv-latency-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 106.5 | 16.7 | 0.04 | 1 |
| sqlite | 106.8 | 17.3 | 0.02 | 1 |

| stage | tinystore | sqlite |
|---|---|---|
| mixed-max×64 | 49 k/s, p99 16.8 ms, 26 µs CPU/op | 1.9 k/s, p99 872.4 ms, 48 µs CPU/op |
| mixed-25%×1024 | 12 k/s, p99 7.3 ms, 45 µs CPU/op | 467/s, p99 9.4 ms, 189 µs CPU/op |
| mixed-50%×1024 | 25 k/s, p99 7.9 ms, 40 µs CPU/op | 933/s, p99 11.5 ms, 133 µs CPU/op |
| mixed-75%×1024 | 37 k/s, p99 9.4 ms, 38 µs CPU/op | 1.4 k/s, p99 5.8 ms, 108 µs CPU/op |
| mixed-90%×1024 | 44 k/s, p99 11.5 ms, 37 µs CPU/op | 1.7 k/s, p99 4.2 ms, 96 µs CPU/op |

## kv-latency

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 106.8 | 16.6 | 0.02 | 1 |
| bbolt | 99.6 | 48.0 | 0.01 | 1 |
| badger | 190.3 | 18.0 | 0.02 | 1 |
| pebble | 102.1 | 29.7 | 0.02 | 1 |
| redis | 125.2 | 25.0 | 0.10 | 2 |
| sqlite | 106.8 | 17.3 | 0.02 | 1 |

| stage | tinystore | bbolt | badger | pebble | redis | sqlite |
|---|---|---|---|---|---|---|
| mixed-max×64 | 61 k/s, p99 11.5 ms, 25 µs CPU/op | 43 k/s, p99 15.7 ms, 5 µs CPU/op | 5.9 k/s, p99 31.5 ms, 18 µs CPU/op | 196 k/s, p99 3.4 ms, 14 µs CPU/op | 10 k/s, p99 9.4 ms, 47 µs CPU/op | 1.9 k/s, p99 872.4 ms, 48 µs CPU/op |
| mixed-25%×1024 | 15 k/s, p99 7.3 ms, 42 µs CPU/op | 11 k/s, p99 14.7 ms, 12 µs CPU/op | 1.5 k/s, p99 5.2 ms, 53 µs CPU/op | 49 k/s, p99 3.7 ms, 23 µs CPU/op | 2.5 k/s, p99 12.6 ms, 101 µs CPU/op | 467/s, p99 9.4 ms, 189 µs CPU/op |
| mixed-50%×1024 | 31 k/s, p99 7.9 ms, 39 µs CPU/op | 22 k/s, p99 14.7 ms, 10 µs CPU/op | 3.0 k/s, p99 9.4 ms, 36 µs CPU/op | 98 k/s, p99 3.7 ms, 19 µs CPU/op | 5.1 k/s, p99 9.4 ms, 83 µs CPU/op | 933/s, p99 11.5 ms, 133 µs CPU/op |
| mixed-75%×1024 | 46 k/s, p99 12.6 ms, 37 µs CPU/op | 32 k/s, p99 14.7 ms, 8 µs CPU/op | 4.4 k/s, p99 21.0 ms, 30 µs CPU/op | 147 k/s, p99 3.7 ms, 18 µs CPU/op | 7.6 k/s, p99 8.4 ms, 67 µs CPU/op | 1.4 k/s, p99 5.8 ms, 108 µs CPU/op |
| mixed-90%×1024 | 55 k/s, p99 13.6 ms, 35 µs CPU/op | 39 k/s, p99 15.7 ms, 8 µs CPU/op | 5.3 k/s, p99 75.5 ms, 27 µs CPU/op | 177 k/s, p99 3.7 ms, 17 µs CPU/op | 9.1 k/s, p99 15.7 ms, 58 µs CPU/op | 1.7 k/s, p99 4.2 ms, 96 µs CPU/op |

## kv-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 78.8 (78.3–78.9) | 16.7 (16.7–16.7) | 0.02 (0.02–0.03) | 1 |
| sqlite | 308.7 (307.5–309.6) | 17.3 (17.3–17.3) | 0.01 (0.01–0.01) | 1 |

| stage | tinystore | sqlite |
|---|---|---|
| get×1 | 116 k (116 k–117 k)/s, p99 27 µs, 10 µs CPU/op | 94 k (94 k–96 k)/s, p99 33 µs, 15 µs CPU/op |
| get×8 | 313 k (303 k–314 k)/s, p99 360 µs, 14 µs CPU/op | 241 k (239 k–250 k)/s, p99 295 µs, 22 µs CPU/op |
| get×64 | 199 k (194 k–200 k)/s, p99 2.4 ms, 21 µs CPU/op | 142 k (141 k–142 k)/s, p99 2.4 ms, 40 µs CPU/op |
| set×1 | 371 (361–375)/s, p99 4.7 ms, 280 µs CPU/op | 436 (432–438)/s, p99 3.7 ms, 197 µs CPU/op |
| set×8 | 1.5 k (1.5 k–1.5 k)/s, p99 10.5 ms, 101 µs CPU/op | 482 (468–500)/s, p99 75.5 ms, 194 µs CPU/op |
| set×64 | 7.7 k (7.6 k–7.8 k)/s, p99 23.1 ms, 47 µs CPU/op | 506 (500–515)/s, p99 671.1 ms, 180 µs CPU/op |
| mixed×1 | 3.4 k (3.2 k–3.4 k)/s, p99 3.4 ms, 42 µs CPU/op | 4.2 k (4.1 k–4.3 k)/s, p99 3.4 ms, 40 µs CPU/op |
| mixed×8 | 13 k (13 k–14 k)/s, p99 7.3 ms, 25 µs CPU/op | 4.9 k (4.9 k–5.0 k)/s, p99 41.9 ms, 38 µs CPU/op |
| mixed×64 | 64 k (64 k–64 k)/s, p99 10.5 ms, 24 µs CPU/op | 5.1 k (5.1 k–5.1 k)/s, p99 335.5 ms, 38 µs CPU/op |

## kv

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 78.9 (78.2–78.9) | 16.8 (16.7–16.8) | 0.02 (0.02–0.03) | 1 |
| tinystore-sidecar | 113.8 (113.8–114.4) | 16.7 (16.7–16.8) | 0.04 (0.04–0.04) | 2 |
| tinystore-server | 112.7 (112.5–113.5) | 17.0 (16.8–17.0) | 0.05 (0.05–0.05) | 2 |
| bbolt | 85.0 (84.6–85.2) | 48.0 | 0.01 (0.01–0.01) | 1 |
| bbolt-borrowed | 85.0 (85.0–85.2) | 48.0 | 0.01 (0.01–0.01) | 1 |
| badger | 182.3 (182.3–255.8) | 17.6 (17.6–17.6) | 0.02 (0.02–0.02) | 1 |
| pebble | 65.0 (64.2–65.3) | 29.7 (29.7–29.7) | 0.02 (0.02–0.02) | 1 |
| redis | 95.0 (93.9–95.3) | 28.2 (27.6–28.3) | 0.10 (0.10–0.10) | 2 |
| redis-tcp | 94.1 (93.7–94.6) | 28.4 (28.3–28.4) | 0.10 (0.10–0.10) | 2 |
| sqlite | 308.7 (307.5–309.6) | 17.3 (17.3–17.3) | 0.01 (0.01–0.01) | 1 |

| stage | tinystore | tinystore-sidecar | tinystore-server | bbolt | bbolt-borrowed | badger | pebble | redis | redis-tcp | sqlite |
|---|---|---|---|---|---|---|---|---|---|---|
| get×1 | 116 k (116 k–118 k)/s, p99 27 µs, 10 µs CPU/op | 5.5 k (5.4 k–5.5 k)/s, p99 246 µs, 240 µs CPU/op | 5.7 k (5.7 k–5.7 k)/s, p99 246 µs, 222 µs CPU/op | 1.02 M (1.00 M–1.04 M)/s, p99 4 µs, 1 µs CPU/op | 1.10 M (1.07 M–1.12 M)/s, p99 3 µs, 1 µs CPU/op | 647 k (634 k–659 k)/s, p99 9 µs, 2 µs CPU/op | 162 k (156 k–163 k)/s, p99 14 µs, 6 µs CPU/op | 14 k (14 k–14 k)/s, p99 123 µs, 88 µs CPU/op | 12 k (12 k–12 k)/s, p99 147 µs, 93 µs CPU/op | 94 k (94 k–96 k)/s, p99 33 µs, 15 µs CPU/op |
| get×8 | 314 k (313 k–314 k)/s, p99 360 µs, 14 µs CPU/op | 47 k (45 k–47 k)/s, p99 328 µs, 74 µs CPU/op | 37 k (37 k–38 k)/s, p99 426 µs, 98 µs CPU/op | 1.04 M (1.01 M–1.05 M)/s, p99 213 µs, 2 µs CPU/op | 1.14 M (1.10 M–1.15 M)/s, p99 197 µs, 2 µs CPU/op | 891 k (887 k–893 k)/s, p99 295 µs, 2 µs CPU/op | 316 k (310 k–351 k)/s, p99 229 µs, 9 µs CPU/op | 68 k (67 k–68 k)/s, p99 721 µs, 58 µs CPU/op | 64 k (63 k–64 k)/s, p99 492 µs, 66 µs CPU/op | 241 k (239 k–250 k)/s, p99 295 µs, 22 µs CPU/op |
| get×64 | 199 k (196 k–200 k)/s, p99 2.4 ms, 21 µs CPU/op | 273 k (266 k–273 k)/s, p99 1.0 ms, 34 µs CPU/op | 142 k (113 k–143 k)/s, p99 1.3 ms, 45 µs CPU/op | 1.08 M (1.08 M–1.12 M)/s, p99 1.4 ms, 3 µs CPU/op | 1.12 M (1.03 M–1.13 M)/s, p99 1.6 ms, 2 µs CPU/op | 927 k (915 k–930 k)/s, p99 2.1 ms, 3 µs CPU/op | 497 k (496 k–549 k)/s, p99 721 µs, 10 µs CPU/op | 182 k (178 k–187 k)/s, p99 2.1 ms, 26 µs CPU/op | 149 k (147 k–150 k)/s, p99 1.7 ms, 33 µs CPU/op | 142 k (141 k–142 k)/s, p99 2.4 ms, 40 µs CPU/op |
| set×1 | 362 (352–369)/s, p99 4.2 ms, 277 µs CPU/op | 328 (244–328)/s, p99 6.8 ms, 451 µs CPU/op | 326 (267–327)/s, p99 4.2 ms, 442 µs CPU/op | 75 (75–75)/s, p99 14.7 ms, 347 µs CPU/op | 75 (75–75)/s, p99 14.7 ms, 319 µs CPU/op | 474 (404–479)/s, p99 3.9 ms, 135 µs CPU/op | 323 (317–326)/s, p99 3.9 ms, 143 µs CPU/op | 304 (302–305)/s, p99 4.2 ms, 204 µs CPU/op | 302 (301–310)/s, p99 4.7 ms, 185 µs CPU/op | 436 (432–438)/s, p99 3.7 ms, 197 µs CPU/op |
| set×8 | 1.5 k (1.5 k–1.5 k)/s, p99 12.6 ms, 102 µs CPU/op | 1.3 k (1.2 k–1.3 k)/s, p99 14.7 ms, 183 µs CPU/op | 1.3 k (1.2 k–1.3 k)/s, p99 14.7 ms, 189 µs CPU/op | 593 (589–593)/s, p99 16.8 ms, 51 µs CPU/op | 595 (595–596)/s, p99 15.7 ms, 50 µs CPU/op | 623 (613–626)/s, p99 25.2 ms, 112 µs CPU/op | 1.3 k (1.3 k–1.3 k)/s, p99 8.4 ms, 48 µs CPU/op | 1.2 k (1.2 k–1.2 k)/s, p99 8.4 ms, 113 µs CPU/op | 1.2 k (1.2 k–1.3 k)/s, p99 7.9 ms, 116 µs CPU/op | 482 (468–500)/s, p99 75.5 ms, 194 µs CPU/op |
| set×64 | 7.7 k (7.7 k–7.7 k)/s, p99 23.1 ms, 47 µs CPU/op | 7.3 k (7.2 k–7.3 k)/s, p99 21.0 ms, 59 µs CPU/op | 7.1 k (7.1 k–7.2 k)/s, p99 23.1 ms, 63 µs CPU/op | 4.4 k (4.4 k–4.4 k)/s, p99 15.7 ms, 21 µs CPU/op | 4.4 k (4.4 k–4.4 k)/s, p99 16.8 ms, 21 µs CPU/op | 683 (642–693)/s, p99 125.8 ms, 92 µs CPU/op | 20 k (20 k–20 k)/s, p99 5.8 ms, 9 µs CPU/op | 9.4 k (8.8 k–9.4 k)/s, p99 8.4 ms, 57 µs CPU/op | 9.5 k (9.5 k–9.5 k)/s, p99 9.4 ms, 66 µs CPU/op | 506 (500–515)/s, p99 671.1 ms, 180 µs CPU/op |
| mixed×1 | 3.3 k (3.2 k–3.4 k)/s, p99 3.4 ms, 41 µs CPU/op | 2.0 k (2.0 k–2.0 k)/s, p99 3.7 ms, 298 µs CPU/op | 2.0 k (1.7 k–2.0 k)/s, p99 3.7 ms, 269 µs CPU/op | 752 (751–756)/s, p99 13.6 ms, 32 µs CPU/op | 755 (749–755)/s, p99 13.6 ms, 29 µs CPU/op | 4.9 k (4.9 k–4.9 k)/s, p99 2.4 ms, 17 µs CPU/op | 6.0 k (6.0 k–6.1 k)/s, p99 2.0 ms, 22 µs CPU/op | 2.6 k (2.5 k–2.6 k)/s, p99 3.7 ms, 106 µs CPU/op | 2.5 k (2.5 k–2.5 k)/s, p99 3.4 ms, 107 µs CPU/op | 4.2 k (4.1 k–4.3 k)/s, p99 3.4 ms, 40 µs CPU/op |
| mixed×8 | 13 k (13 k–13 k)/s, p99 7.3 ms, 25 µs CPU/op | 12 k (11 k–12 k)/s, p99 6.8 ms, 138 µs CPU/op | 11 k (11 k–11 k)/s, p99 6.8 ms, 145 µs CPU/op | 5.8 k (5.8 k–5.9 k)/s, p99 14.7 ms, 8 µs CPU/op | 5.8 k (5.8 k–5.9 k)/s, p99 14.7 ms, 8 µs CPU/op | 5.0 k (5.0 k–5.0 k)/s, p99 9.4 ms, 21 µs CPU/op | 26 k (26 k–26 k)/s, p99 3.1 ms, 18 µs CPU/op | 3.3 k (3.3 k–3.3 k)/s, p99 7.3 ms, 94 µs CPU/op | 3.3 k (3.3 k–3.3 k)/s, p99 7.3 ms, 99 µs CPU/op | 4.9 k (4.9 k–5.0 k)/s, p99 41.9 ms, 38 µs CPU/op |
| mixed×64 | 63 k (63 k–63 k)/s, p99 11.5 ms, 24 µs CPU/op | 63 k (63 k–63 k)/s, p99 10.5 ms, 57 µs CPU/op | 61 k (60 k–61 k)/s, p99 9.4 ms, 70 µs CPU/op | 43 k (42 k–43 k)/s, p99 15.7 ms, 5 µs CPU/op | 43 k (43 k–43 k)/s, p99 15.7 ms, 5 µs CPU/op | 6.4 k (6.3 k–6.4 k)/s, p99 29.4 ms, 18 µs CPU/op | 197 k (196 k–197 k)/s, p99 3.4 ms, 14 µs CPU/op | 10 k (10 k–10 k)/s, p99 9.4 ms, 46 µs CPU/op | 10 k (10 k–10 k)/s, p99 7.9 ms, 62 µs CPU/op | 5.1 k (5.1 k–5.1 k)/s, p99 335.5 ms, 38 µs CPU/op |

## metrics

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 251.1 (248.8–251.7) | 6.4 | 0.02 (0.02–0.02) | 1 |
| prometheus | 309.2 (270.0–310.8) | 71.8 (71.5–71.8) | 0.01 (0.01–0.01) | 1 |
| victoria | 660.6 (655.9–664.6) | 8.7 (8.7–8.7) | 0.05 (0.05–0.11) | 2 |

| stage | tinystore | prometheus | victoria |
|---|---|---|---|
| ingest×1 | 828 k (821 k–840 k)/s, p99 251.7 ms, 1 µs CPU/op | 10.49 M (9.90 M–10.80 M)/s, p99 29.4 ms, 0 µs CPU/op | 7.08 M (6.67 M–7.18 M)/s, p99 100.7 ms, 0 µs CPU/op |
| settle×1 | 0/s, p99 0 µs, 0 µs CPU/op | 0/s, p99 0 µs, 0 µs CPU/op | 0/s, p99 0 µs, 0 µs CPU/op |
| read-series×1 | 8.7 k (8.6 k–9.2 k)/s, p99 295 µs, 153 µs CPU/op | 41 k (41 k–42 k)/s, p99 49 µs, 25 µs CPU/op | 2.3 k (2.2 k–2.3 k)/s, p99 852 µs, 665 µs CPU/op |
| read-series×8 | 11 k (11 k–12 k)/s, p99 1.3 ms, 185 µs CPU/op | 261 k (236 k–270 k)/s, p99 82 µs, 31 µs CPU/op | 12 k (12 k–12 k)/s, p99 1.8 ms, 720 µs CPU/op |
| read-wide×1 | — | 76 (76–78)/s, p99 16.8 ms, 13386 µs CPU/op | 11 (11–11)/s, p99 117.4 ms, 245273 µs CPU/op |

- tinystore #1 read-wide×1: 664 errors, first: metrics resource limit: output samples
- tinystore #2 read-wide×1: 645 errors, first: metrics resource limit: output samples
- tinystore #3 read-wide×1: 621 errors, first: metrics resource limit: output samples

## records-before-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 396.4 (394.4–397.3) | 12.3 (12.3–12.4) | 0.02 (0.02–0.02) | 1 |
| sqlite | 379.1 (375.8–379.3) | 164.4 | 0.02 (0.02–0.02) | 1 |
| jsonl | 471.1 (454.4–509.1) | 103.1 | 0.00 (0.00–0.00) | 1 |
| jsonl-zstd | 644.6 (641.5–655.9) | 8.4 (8.4–8.4) | 0.00 (0.00–0.00) | 1 |

| stage | tinystore | sqlite | jsonl | jsonl-zstd |
|---|---|---|---|---|
| append×1 | 218 k (216 k–219 k)/s, p99 10.5 ms, 2 µs CPU/op | 126 k (125 k–126 k)/s, p99 16.8 ms, 5 µs CPU/op | 24 k (24 k–25 k)/s, p99 62.9 ms, 3 µs CPU/op | 25 k (24 k–25 k)/s, p99 62.9 ms, 3 µs CPU/op |
| settle×1 | 0/s, p99 0 µs, 0 µs CPU/op | 0/s, p99 0 µs, 0 µs CPU/op | 0/s, p99 0 µs, 0 µs CPU/op | 0/s, p99 0 µs, 0 µs CPU/op |
| read-window×1 | 2.4 k (2.4 k–2.4 k)/s, p99 3.1 ms, 632 µs CPU/op | 45 k (45 k–45 k)/s, p99 328 µs, 29 µs CPU/op | 2.3 k (2.3 k–2.3 k)/s, p99 4.2 ms, 609 µs CPU/op | 802 (802–810)/s, p99 15.7 ms, 1984 µs CPU/op |
| read-window×8 | 5.2 k (5.2 k–5.3 k)/s, p99 7.3 ms, 525 µs CPU/op | 101 k (96 k–102 k)/s, p99 983 µs, 40 µs CPU/op | 10 k (10 k–11 k)/s, p99 7.9 ms, 925 µs CPU/op | 2.7 k (2.7 k–2.7 k)/s, p99 41.9 ms, 3547 µs CPU/op |

## records-failed-untimed

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| sqlite | 375.0 (371.1–378.2) | 164.6 | 0.02 (0.02–0.02) | 1 |
| jsonl | 452.7 (445.2–460.3) | 103.1 | 0.00 (0.00–0.00) | 1 |
| jsonl-zstd | 674.1 (637.2–681.3) | 8.4 (8.4–8.4) | 0.00 (0.00–0.00) | 1 |

| stage | sqlite | jsonl | jsonl-zstd |
|---|---|---|---|
| append×1 | 119 k (114 k–120 k)/s, p99 18.9 ms, 5 µs CPU/op | 24 k (22 k–24 k)/s, p99 67.1 ms, 3 µs CPU/op | 24 k (22 k–24 k)/s, p99 62.9 ms, 3 µs CPU/op |
| settle×1 | 0/s, p99 0 µs, 0 µs CPU/op | 0/s, p99 0 µs, 0 µs CPU/op | 0/s, p99 0 µs, 0 µs CPU/op |
| read-window×1 | 44 k (42 k–44 k)/s, p99 360 µs, 30 µs CPU/op | 2.2 k (2.2 k–2.2 k)/s, p99 4.2 ms, 628 µs CPU/op | 773 (764–782)/s, p99 16.8 ms, 2019 µs CPU/op |
| read-window×8 | 93 k (93 k–99 k)/s, p99 1.0 ms, 43 µs CPU/op | 10 k (10.0 k–11 k)/s, p99 8.4 ms, 942 µs CPU/op | 2.5 k (2.5 k–2.5 k)/s, p99 41.9 ms, 3712 µs CPU/op |

- failed: tinystore, repeat 1: exit status 1
- failed: tinystore, repeat 2: exit status 1
- failed: tinystore, repeat 3: exit status 1

## records-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 394.1 (394.0–394.8) | 12.4 (12.3–12.4) | 0.02 (0.02–0.03) | 1 |
| sqlite | 379.3 (372.3–380.4) | 164.4 | 0.02 (0.02–0.02) | 1 |

| stage | tinystore | sqlite |
|---|---|---|
| append×1 | 216 k (157 k–218 k)/s, p99 10.5 ms, 2 µs CPU/op | 123 k (122 k–125 k)/s, p99 18.9 ms, 5 µs CPU/op |
| settle×1 | 0/s, p99 0 µs, 0 µs CPU/op | 0/s, p99 0 µs, 0 µs CPU/op |
| read-window×1 | 2.4 k (2.4 k–2.4 k)/s, p99 3.4 ms, 632 µs CPU/op | 45 k (42 k–45 k)/s, p99 328 µs, 29 µs CPU/op |
| read-window×8 | 5.3 k (5.3 k–5.3 k)/s, p99 7.3 ms, 522 µs CPU/op | 175 k (173 k–176 k)/s, p99 590 µs, 34 µs CPU/op |

## records

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 396.4 (394.4–397.3) | 12.3 (12.3–12.4) | 0.02 (0.02–0.02) | 1 |
| jsonl | 471.1 (454.4–509.1) | 103.1 | 0.00 (0.00–0.00) | 1 |
| jsonl-zstd | 644.6 (641.5–655.9) | 8.4 (8.4–8.4) | 0.00 (0.00–0.00) | 1 |
| sqlite | 379.3 (372.3–380.4) | 164.4 | 0.02 (0.02–0.02) | 1 |

| stage | tinystore | jsonl | jsonl-zstd | sqlite |
|---|---|---|---|---|
| append×1 | 218 k (216 k–219 k)/s, p99 10.5 ms, 2 µs CPU/op | 24 k (24 k–25 k)/s, p99 62.9 ms, 3 µs CPU/op | 25 k (24 k–25 k)/s, p99 62.9 ms, 3 µs CPU/op | 123 k (122 k–125 k)/s, p99 18.9 ms, 5 µs CPU/op |
| settle×1 | 0/s, p99 0 µs, 0 µs CPU/op | 0/s, p99 0 µs, 0 µs CPU/op | 0/s, p99 0 µs, 0 µs CPU/op | 0/s, p99 0 µs, 0 µs CPU/op |
| read-window×1 | 2.4 k (2.4 k–2.4 k)/s, p99 3.1 ms, 632 µs CPU/op | 2.3 k (2.3 k–2.3 k)/s, p99 4.2 ms, 609 µs CPU/op | 802 (802–810)/s, p99 15.7 ms, 1984 µs CPU/op | 45 k (42 k–45 k)/s, p99 328 µs, 29 µs CPU/op |
| read-window×8 | 5.2 k (5.2 k–5.3 k)/s, p99 7.3 ms, 525 µs CPU/op | 10 k (10 k–11 k)/s, p99 7.9 ms, 925 µs CPU/op | 2.7 k (2.7 k–2.7 k)/s, p99 41.9 ms, 3547 µs CPU/op | 175 k (173 k–176 k)/s, p99 590 µs, 34 µs CPU/op |

## sdk-kv-superseded-lcg

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| bun-redis | 141.6 (141.5–142.2) | 0.0 | 0.00 (0.00–0.00) | 2 |
| bun-redis-tcp | 110.7 (110.0–110.9) | 0.0 | 0.00 (0.00–0.00) | 2 |
| bun-tinystore | 150.4 (147.4–151.2) | 0.0 | 0.02 (0.02–0.02) | 2 |
| bun-tinystore-server | 141.3 (141.1–144.1) | 0.0 | 0.00 (0.00–0.00) | 2 |
| python-redis | 70.9 (70.5–71.1) | 0.0 | 0.05 (0.05–0.07) | 2 |
| python-redis-tcp | 70.8 (70.3–71.0) | 0.0 | 0.05 (0.05–0.05) | 2 |
| python-tinystore | 102.0 (101.9–102.9) | 0.0 | 0.02 (0.02–0.02) | 2 |
| python-tinystore-server | 100.7 (100.2–101.2) | 0.0 | 0.00 (0.00–0.00) | 2 |

| stage | bun-redis | bun-redis-tcp | bun-tinystore | bun-tinystore-server | python-redis | python-redis-tcp | python-tinystore | python-tinystore-server |
|---|---|---|---|---|---|---|---|---|
| get×1 | 21 k (21 k–21 k)/s, p99 92 µs, 64 µs CPU/op | 15 k (15 k–16 k)/s, p99 135 µs, 87 µs CPU/op | 6.8 k (6.8 k–6.9 k)/s, p99 289 µs, 203 µs CPU/op | 6.5 k (6.5 k–6.6 k)/s, p99 283 µs, 201 µs CPU/op | 11 k (11 k–11 k)/s, p99 147 µs, 100 µs CPU/op | 9.6 k (9.3 k–9.7 k)/s, p99 160 µs, 115 µs CPU/op | 4.6 k (4.5 k–4.7 k)/s, p99 361 µs, 251 µs CPU/op | 4.7 k (4.6 k–4.7 k)/s, p99 290 µs, 240 µs CPU/op |
| get×8 | 138 k (135 k–142 k)/s, p99 125 µs, 11 µs CPU/op | 84 k (83 k–85 k)/s, p99 200 µs, 19 µs CPU/op | 45 k (45 k–46 k)/s, p99 1.3 ms, 69 µs CPU/op | 37 k (36 k–38 k)/s, p99 915 µs, 89 µs CPU/op | 18 k (17 k–18 k)/s, p99 786 µs, 71 µs CPU/op | 16 k (15 k–16 k)/s, p99 747 µs, 84 µs CPU/op | 29 k (25 k–29 k)/s, p99 541 µs, 89 µs CPU/op | 27 k (26 k–27 k)/s, p99 487 µs, 104 µs CPU/op |
| get×64 | 327 k (325 k–344 k)/s, p99 379 µs, 5 µs CPU/op | 157 k (156 k–163 k)/s, p99 856 µs, 12 µs CPU/op | 132 k (126 k–133 k)/s, p99 3.0 ms, 34 µs CPU/op | 116 k (115 k–119 k)/s, p99 3.0 ms, 41 µs CPU/op | 19 k (18 k–19 k)/s, p99 4.4 ms, 68 µs CPU/op | 16 k (15 k–16 k)/s, p99 4.8 ms, 82 µs CPU/op | 58 k (56 k–59 k)/s, p99 1.6 ms, 45 µs CPU/op | 57 k (56 k–57 k)/s, p99 1.9 ms, 49 µs CPU/op |
| set×1 | 306 (305–307)/s, p99 4.5 ms, 170 µs CPU/op | 298 (221–307)/s, p99 5.4 ms, 175 µs CPU/op | 329 (323–334)/s, p99 4.6 ms, 426 µs CPU/op | 328 (320–328)/s, p99 4.7 ms, 415 µs CPU/op | 297 (294–302)/s, p99 4.1 ms, 189 µs CPU/op | 306 (306–310)/s, p99 3.7 ms, 209 µs CPU/op | 320 (319–324)/s, p99 4.8 ms, 463 µs CPU/op | 319 (318–330)/s, p99 4.4 ms, 533 µs CPU/op |
| set×8 | 1.2 k (1.2 k–1.3 k)/s, p99 7.9 ms, 40 µs CPU/op | 1.2 k (1.2 k–1.2 k)/s, p99 8.0 ms, 49 µs CPU/op | 1.3 k (1.3 k–1.3 k)/s, p99 15.2 ms, 183 µs CPU/op | 1.3 k (1.2 k–1.3 k)/s, p99 14.3 ms, 190 µs CPU/op | 1.2 k (1.2 k–1.2 k)/s, p99 7.9 ms, 71 µs CPU/op | 1.2 k (1.2 k–1.2 k)/s, p99 7.9 ms, 80 µs CPU/op | 1.3 k (1.2 k–1.3 k)/s, p99 14.9 ms, 198 µs CPU/op | 1.3 k (1.3 k–1.3 k)/s, p99 14.6 ms, 198 µs CPU/op |
| set×64 | 9.9 k (9.7 k–10 k)/s, p99 8.6 ms, 10 µs CPU/op | 9.6 k (9.6 k–9.6 k)/s, p99 8.3 ms, 12 µs CPU/op | 7.2 k (7.2 k–7.3 k)/s, p99 21.7 ms, 70 µs CPU/op | 7.2 k (7.1 k–7.2 k)/s, p99 22.0 ms, 69 µs CPU/op | 9.3 k (9.3 k–9.4 k)/s, p99 8.8 ms, 31 µs CPU/op | 9.4 k (9.3 k–9.5 k)/s, p99 8.6 ms, 39 µs CPU/op | 7.3 k (5.9 k–7.4 k)/s, p99 22.4 ms, 73 µs CPU/op | 7.2 k (7.2 k–7.2 k)/s, p99 21.3 ms, 69 µs CPU/op |
| mixed×1 | 1.5 k (1.4 k–1.5 k)/s, p99 3.5 ms, 93 µs CPU/op | 1.4 k (1.4 k–1.4 k)/s, p99 3.6 ms, 114 µs CPU/op | 1.3 k (1.2 k–1.3 k)/s, p99 3.7 ms, 277 µs CPU/op | 1.3 k (1.2 k–1.3 k)/s, p99 3.7 ms, 275 µs CPU/op | 1.4 k (1.4 k–1.4 k)/s, p99 3.5 ms, 128 µs CPU/op | 1.3 k (1.3 k–9.3 k)/s, p99 3.5 ms, 145 µs CPU/op | 5.4 k (1.2 k–5.4 k)/s, p99 307 µs, 218 µs CPU/op | 1.2 k (1.2 k–1.2 k)/s, p99 3.7 ms, 306 µs CPU/op |
| mixed×8 | 2.1 k (2.1 k–2.1 k)/s, p99 7.0 ms, 31 µs CPU/op | 2.1 k (2.1 k–2.1 k)/s, p99 6.9 ms, 39 µs CPU/op | 6.2 k (6.1 k–6.4 k)/s, p99 7.0 ms, 144 µs CPU/op | 6.1 k (6.1 k–6.2 k)/s, p99 6.9 ms, 161 µs CPU/op | 2.0 k (1.9 k–2.0 k)/s, p99 7.3 ms, 70 µs CPU/op | 2.0 k (2.0 k–15 k)/s, p99 7.2 ms, 86 µs CPU/op | 29 k (6.0 k–29 k)/s, p99 546 µs, 90 µs CPU/op | 6.0 k (5.9 k–6.0 k)/s, p99 6.8 ms, 168 µs CPU/op |
| mixed×64 | 10 k (10 k–10 k)/s, p99 8.0 ms, 10 µs CPU/op | 10 k (10 k–10 k)/s, p99 8.1 ms, 13 µs CPU/op | 36 k (35 k–36 k)/s, p99 18.1 ms, 61 µs CPU/op | 35 k (26 k–35 k)/s, p99 18.9 ms, 74 µs CPU/op | 9.7 k (9.6 k–10 k)/s, p99 9.1 ms, 33 µs CPU/op | 9.8 k (9.4 k–16 k)/s, p99 8.2 ms, 38 µs CPU/op | 56 k (35 k–56 k)/s, p99 1.8 ms, 46 µs CPU/op | 35 k (35 k–36 k)/s, p99 18.1 ms, 71 µs CPU/op |

## sdk-kv

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| bun-redis | 145.6 (145.2–145.7) | 0.0 | 0.00 (0.00–0.00) | 2 |
| bun-redis-tcp | 110.5 (109.2–111.5) | 0.0 | 0.00 (0.00–0.00) | 2 |
| bun-tinystore | 150.0 (146.6–150.3) | 0.0 | 0.02 (0.02–0.02) | 2 |
| bun-tinystore-server | 141.5 (140.9–143.5) | 0.0 | 0.00 (0.00–0.00) | 2 |
| python-redis | 70.6 (70.5–71.0) | 0.0 | 0.05 (0.05–0.05) | 2 |
| python-redis-tcp | 70.9 (70.8–70.9) | 0.0 | 0.05 (0.05–0.05) | 2 |
| python-tinystore | 100.6 (100.6–101.3) | 0.0 | 0.02 (0.02–0.02) | 2 |
| python-tinystore-server | 100.9 (100.5–101.3) | 0.0 | 0.00 (0.00–0.00) | 2 |

| stage | bun-redis | bun-redis-tcp | bun-tinystore | bun-tinystore-server | python-redis | python-redis-tcp | python-tinystore | python-tinystore-server |
|---|---|---|---|---|---|---|---|---|
| get×1 | 23 k (23 k–23 k)/s, p99 80 µs, 60 µs CPU/op | 16 k (15 k–16 k)/s, p99 100 µs, 84 µs CPU/op | 6.8 k (6.7 k–6.9 k)/s, p99 214 µs, 202 µs CPU/op | 6.6 k (6.6 k–6.7 k)/s, p99 209 µs, 199 µs CPU/op | 11 k (11 k–11 k)/s, p99 153 µs, 100 µs CPU/op | 9.6 k (9.5 k–9.6 k)/s, p99 164 µs, 116 µs CPU/op | 4.8 k (4.7 k–4.8 k)/s, p99 287 µs, 241 µs CPU/op | 4.9 k (4.7 k–5.0 k)/s, p99 266 µs, 230 µs CPU/op |
| get×8 | 142 k (142 k–144 k)/s, p99 105 µs, 11 µs CPU/op | 89 k (88 k–89 k)/s, p99 165 µs, 18 µs CPU/op | 47 k (46 k–47 k)/s, p99 1.1 ms, 68 µs CPU/op | 38 k (37 k–38 k)/s, p99 938 µs, 88 µs CPU/op | 18 k (18 k–18 k)/s, p99 648 µs, 70 µs CPU/op | 16 k (16 k–16 k)/s, p99 715 µs, 83 µs CPU/op | 28 k (28 k–28 k)/s, p99 505 µs, 90 µs CPU/op | 27 k (26 k–27 k)/s, p99 531 µs, 105 µs CPU/op |
| get×64 | 345 k (338 k–350 k)/s, p99 369 µs, 5 µs CPU/op | 160 k (159 k–161 k)/s, p99 905 µs, 12 µs CPU/op | 132 k (131 k–134 k)/s, p99 3.0 ms, 34 µs CPU/op | 119 k (119 k–120 k)/s, p99 2.9 ms, 41 µs CPU/op | 18 k (18 k–19 k)/s, p99 4.3 ms, 68 µs CPU/op | 16 k (16 k–16 k)/s, p99 4.9 ms, 83 µs CPU/op | 57 k (57 k–58 k)/s, p99 1.9 ms, 45 µs CPU/op | 58 k (57 k–58 k)/s, p99 1.8 ms, 48 µs CPU/op |
| set×1 | 313 (305–318)/s, p99 4.4 ms, 153 µs CPU/op | 305 (303–309)/s, p99 3.9 ms, 184 µs CPU/op | 329 (286–332)/s, p99 4.6 ms, 419 µs CPU/op | 326 (206–332)/s, p99 5.9 ms, 398 µs CPU/op | 307 (299–308)/s, p99 4.1 ms, 195 µs CPU/op | 305 (297–311)/s, p99 4.1 ms, 203 µs CPU/op | 332 (324–338)/s, p99 3.9 ms, 469 µs CPU/op | 322 (322–339)/s, p99 4.4 ms, 521 µs CPU/op |
| set×8 | 1.2 k (1.2 k–1.3 k)/s, p99 7.5 ms, 42 µs CPU/op | 1.3 k (1.2 k–1.3 k)/s, p99 7.7 ms, 50 µs CPU/op | 1.2 k (821–1.3 k)/s, p99 13.2 ms, 196 µs CPU/op | 1.3 k (1.2 k–1.3 k)/s, p99 14.3 ms, 193 µs CPU/op | 1.3 k (1.3 k–1.3 k)/s, p99 7.9 ms, 70 µs CPU/op | 1.2 k (1.2 k–1.2 k)/s, p99 8.2 ms, 82 µs CPU/op | 1.3 k (1.3 k–1.3 k)/s, p99 14.1 ms, 179 µs CPU/op | 1.3 k (1.3 k–1.3 k)/s, p99 11.8 ms, 196 µs CPU/op |
| set×64 | 9.9 k (9.9 k–10 k)/s, p99 7.9 ms, 10 µs CPU/op | 9.8 k (9.6 k–9.8 k)/s, p99 8.9 ms, 12 µs CPU/op | 7.3 k (5.2 k–7.3 k)/s, p99 21.9 ms, 71 µs CPU/op | 7.2 k (7.2 k–7.3 k)/s, p99 22.1 ms, 68 µs CPU/op | 9.4 k (9.2 k–9.4 k)/s, p99 8.8 ms, 31 µs CPU/op | 9.5 k (8.9 k–9.6 k)/s, p99 8.7 ms, 39 µs CPU/op | 7.4 k (7.4 k–7.4 k)/s, p99 21.8 ms, 75 µs CPU/op | 7.2 k (7.1 k–7.4 k)/s, p99 21.7 ms, 74 µs CPU/op |
| mixed×1 | 2.7 k (2.5 k–2.7 k)/s, p99 3.4 ms, 81 µs CPU/op | 2.5 k (2.5 k–2.5 k)/s, p99 3.4 ms, 105 µs CPU/op | 2.1 k (2.0 k–2.2 k)/s, p99 3.5 ms, 255 µs CPU/op | 2.1 k (2.1 k–2.1 k)/s, p99 3.5 ms, 244 µs CPU/op | 2.3 k (2.3 k–2.4 k)/s, p99 3.5 ms, 116 µs CPU/op | 2.4 k (2.3 k–2.4 k)/s, p99 3.4 ms, 130 µs CPU/op | 1.8 k (1.8 k–1.9 k)/s, p99 3.6 ms, 278 µs CPU/op | 1.9 k (1.9 k–1.9 k)/s, p99 3.6 ms, 266 µs CPU/op |
| mixed×8 | 3.6 k (2.2 k–3.6 k)/s, p99 6.6 ms, 24 µs CPU/op | 3.4 k (3.4 k–3.5 k)/s, p99 6.8 ms, 33 µs CPU/op | 12 k (12 k–12 k)/s, p99 6.6 ms, 125 µs CPU/op | 11 k (11 k–12 k)/s, p99 6.6 ms, 142 µs CPU/op | 3.1 k (3.0 k–3.2 k)/s, p99 7.1 ms, 70 µs CPU/op | 3.1 k (3.0 k–3.2 k)/s, p99 7.1 ms, 83 µs CPU/op | 11 k (11 k–11 k)/s, p99 6.6 ms, 136 µs CPU/op | 11 k (10 k–11 k)/s, p99 6.6 ms, 149 µs CPU/op |
| mixed×64 | 11 k (11 k–11 k)/s, p99 8.3 ms, 9 µs CPU/op | 11 k (11 k–11 k)/s, p99 7.9 ms, 13 µs CPU/op | 64 k (63 k–64 k)/s, p99 9.4 ms, 50 µs CPU/op | 60 k (59 k–61 k)/s, p99 9.3 ms, 60 µs CPU/op | 10 k (9.8 k–10 k)/s, p99 9.4 ms, 36 µs CPU/op | 9.9 k (9.7 k–9.9 k)/s, p99 9.2 ms, 38 µs CPU/op | 48 k (48 k–48 k)/s, p99 8.3 ms, 52 µs CPU/op | 48 k (48 k–48 k)/s, p99 8.3 ms, 57 µs CPU/op |

## sqldb-before-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 73.0 (72.8–73.2) | 54.2 (53.8–55.0) | 0.02 (0.02–0.02) | 1 |
| sqlite | 111.8 (107.9–131.5) | 33.8 (33.6–33.9) | 0.02 (0.01–0.02) | 1 |
| postgres | 113.7 (113.1–121.5) | 231.8 (230.8–235.6) | 0.44 (0.41–0.45) | 10 |
| ncruces | 85.3 (85.0–85.7) | 35.2 (35.1–35.2) | 0.02 (0.02–0.02) | 1 |

| stage | tinystore | sqlite | postgres | ncruces |
|---|---|---|---|---|
| get×1 | 121 k (121 k–123 k)/s, p99 29 µs, 13 µs CPU/op | 99 k (98 k–101 k)/s, p99 37 µs, 15 µs CPU/op | 7.7 k (7.6 k–7.9 k)/s, p99 180 µs, 169 µs CPU/op | 108 k (108 k–109 k)/s, p99 31 µs, 13 µs CPU/op |
| get×8 | 279 k (279 k–286 k)/s, p99 360 µs, 18 µs CPU/op | 179 k (176 k–179 k)/s, p99 459 µs, 23 µs CPU/op | 52 k (51 k–53 k)/s, p99 492 µs, 83 µs CPU/op | 137 k (136 k–137 k)/s, p99 852 µs, 45 µs CPU/op |
| get×64 | 239 k (236 k–252 k)/s, p99 1.3 ms, 20 µs CPU/op | 158 k (157 k–160 k)/s, p99 3.1 ms, 40 µs CPU/op | 93 k (89 k–95 k)/s, p99 16.8 ms, 42 µs CPU/op | 124 k (118 k–125 k)/s, p99 10.5 ms, 62 µs CPU/op |
| insert×1 | 367 (364–378)/s, p99 3.9 ms, 229 µs CPU/op | 375 (370–382)/s, p99 3.9 ms, 208 µs CPU/op | 602 (596–608)/s, p99 2.4 ms, 219 µs CPU/op | 642 (642–654)/s, p99 2.1 ms, 103 µs CPU/op |
| insert×8 | 1.7 k (1.7 k–1.7 k)/s, p99 7.3 ms, 64 µs CPU/op | 373 (372–377)/s, p99 83.9 ms, 219 µs CPU/op | 2.5 k (2.5 k–2.5 k)/s, p99 4.7 ms, 104 µs CPU/op | 636 (627–639)/s, p99 54.5 ms, 141 µs CPU/op |
| insert×64 | 11 k (10 k–11 k)/s, p99 13.6 ms, 20 µs CPU/op | 376 (373–379)/s, p99 805.3 ms, 221 µs CPU/op | 18 k (17 k–18 k)/s, p99 8.4 ms, 100 µs CPU/op | 637 (633–640)/s, p99 469.8 ms, 142 µs CPU/op |
| mixed×1 | 4.0 k (3.9 k–4.1 k)/s, p99 3.4 ms, 36 µs CPU/op | 4.0 k (4.0 k–4.0 k)/s, p99 3.4 ms, 36 µs CPU/op | 3.5 k (3.5 k–3.5 k)/s, p99 2.0 ms, 180 µs CPU/op | 6.9 k (6.8 k–6.9 k)/s, p99 1.8 ms, 32 µs CPU/op |
| mixed×8 | 16 k (16 k–17 k)/s, p99 6.8 ms, 23 µs CPU/op | 4.7 k (4.6 k–4.8 k)/s, p99 41.9 ms, 36 µs CPU/op | 21 k (21 k–21 k)/s, p99 3.4 ms, 80 µs CPU/op | 8.3 k (8.2 k–8.3 k)/s, p99 23.1 ms, 32 µs CPU/op |
| mixed×64 | 79 k (79 k–80 k)/s, p99 8.4 ms, 23 µs CPU/op | 4.7 k (4.5 k–4.7 k)/s, p99 335.5 ms, 38 µs CPU/op | 64 k (63 k–66 k)/s, p99 10.5 ms, 55 µs CPU/op | 7.9 k (7.9 k–8.0 k)/s, p99 201.3 ms, 34 µs CPU/op |

## sqldb-cold-before-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 53.3 (53.0–53.5) | 31.4 (31.3–31.8) | 0.02 (0.02–0.02) | 1 |
| sqlite | 54.3 (54.0–55.3) | 31.9 (31.8–31.9) | 0.02 (0.02–0.03) | 1 |
| postgres | 128.3 (128.2–129.0) | 122.2 (121.9–123.0) | 0.44 (0.40–0.44) | 9 |

| stage | tinystore | sqlite | postgres |
|---|---|---|---|
| get-cold×1 | 28 k (23 k–28 k)/s, p99 213 µs, 29 µs CPU/op | 30 k (22 k–31 k)/s, p99 213 µs, 29 µs CPU/op | 6.5 k (6.5 k–6.7 k)/s, p99 426 µs, 178 µs CPU/op |
| get-warm×1 | 113 k (111 k–115 k)/s, p99 31 µs, 14 µs CPU/op | 94 k (91 k–95 k)/s, p99 37 µs, 15 µs CPU/op | 7.7 k (7.7 k–7.8 k)/s, p99 180 µs, 168 µs CPU/op |

## sqldb-cold-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 53.1 (52.7–53.1) | 31.4 (31.2–32.3) | 0.03 (0.02–0.03) | 1 |
| sqlite | 54.3 (54.0–54.8) | 31.7 (31.7–32.0) | 0.03 (0.01–0.03) | 1 |
| postgres | 126.9 (126.9–127.7) | 103.2 (103.1–103.2) | 0.50 (0.46–0.52) | 9 |

| stage | tinystore | sqlite | postgres |
|---|---|---|---|
| get-cold×1 | 24 k (22 k–24 k)/s, p99 229 µs, 32 µs CPU/op | 25 k (21 k–26 k)/s, p99 229 µs, 32 µs CPU/op | 6.6 k (6.6 k–6.9 k)/s, p99 393 µs, 176 µs CPU/op |
| get-warm×1 | 114 k (111 k–114 k)/s, p99 31 µs, 14 µs CPU/op | 91 k (91 k–95 k)/s, p99 37 µs, 16 µs CPU/op | 7.9 k (7.6 k–7.9 k)/s, p99 197 µs, 165 µs CPU/op |

## sqldb-cold

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 53.3 (53.0–53.5) | 31.4 (31.3–31.8) | 0.02 (0.02–0.02) | 1 |
| sqlite | 54.3 (54.0–54.8) | 31.7 (31.7–32.0) | 0.03 (0.01–0.03) | 1 |
| postgres | 126.9 (126.9–127.7) | 103.2 (103.1–103.2) | 0.50 (0.46–0.52) | 9 |

| stage | tinystore | sqlite | postgres |
|---|---|---|---|
| get-cold×1 | 28 k (23 k–28 k)/s, p99 213 µs, 29 µs CPU/op | 25 k (21 k–26 k)/s, p99 229 µs, 32 µs CPU/op | 6.6 k (6.6 k–6.9 k)/s, p99 393 µs, 176 µs CPU/op |
| get-warm×1 | 113 k (111 k–115 k)/s, p99 31 µs, 14 µs CPU/op | 91 k (91 k–95 k)/s, p99 37 µs, 16 µs CPU/op | 7.9 k (7.6 k–7.9 k)/s, p99 197 µs, 165 µs CPU/op |

## sqldb-mattn-before-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| mattn | 96.8 (94.4–100.9) | 33.7 (33.5–33.8) | 0.02 (0.02–0.02) | 1 |

| stage | mattn |
|---|---|
| get×1 | 127 k (125 k–129 k)/s, p99 29 µs, 12 µs CPU/op |
| get×8 | 190 k (189 k–195 k)/s, p99 492 µs, 31 µs CPU/op |
| get×64 | 140 k (135 k–144 k)/s, p99 4.7 ms, 81 µs CPU/op |
| insert×1 | 381 (376–382)/s, p99 4.2 ms, 189 µs CPU/op |
| insert×8 | 376 (366–378)/s, p99 92.3 ms, 196 µs CPU/op |
| insert×64 | 381 (365–382)/s, p99 805.3 ms, 193 µs CPU/op |
| mixed×1 | 4.0 k (3.9 k–4.1 k)/s, p99 3.4 ms, 36 µs CPU/op |
| mixed×8 | 4.7 k (4.6 k–4.7 k)/s, p99 41.9 ms, 32 µs CPU/op |
| mixed×64 | 4.7 k (4.7 k–4.8 k)/s, p99 335.5 ms, 36 µs CPU/op |

## sqldb-mattn-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| mattn | 195.1 (193.7–196.9) | 33.8 (33.4–33.8) | 0.02 (0.02–0.02) | 1 |

| stage | mattn |
|---|---|
| get×1 | 125 k (125 k–129 k)/s, p99 29 µs, 12 µs CPU/op |
| get×8 | 292 k (290 k–296 k)/s, p99 393 µs, 20 µs CPU/op |
| get×64 | 429 k (403 k–432 k)/s, p99 2.0 ms, 21 µs CPU/op |
| insert×1 | 375 (327–389)/s, p99 3.7 ms, 192 µs CPU/op |
| insert×8 | 369 (367–378)/s, p99 92.3 ms, 199 µs CPU/op |
| insert×64 | 372 (369–372)/s, p99 805.3 ms, 203 µs CPU/op |
| mixed×1 | 4.0 k (3.9 k–4.0 k)/s, p99 3.4 ms, 37 µs CPU/op |
| mixed×8 | 4.6 k (4.6 k–4.7 k)/s, p99 41.9 ms, 33 µs CPU/op |
| mixed×64 | 4.7 k (4.6 k–4.7 k)/s, p99 335.5 ms, 30 µs CPU/op |

## sqldb-mattn

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| mattn | 195.1 (193.7–196.9) | 33.8 (33.4–33.8) | 0.02 (0.02–0.02) | 1 |

| stage | mattn |
|---|---|
| get×1 | 125 k (125 k–129 k)/s, p99 29 µs, 12 µs CPU/op |
| get×8 | 292 k (290 k–296 k)/s, p99 393 µs, 20 µs CPU/op |
| get×64 | 429 k (403 k–432 k)/s, p99 2.0 ms, 21 µs CPU/op |
| insert×1 | 375 (327–389)/s, p99 3.7 ms, 192 µs CPU/op |
| insert×8 | 369 (367–378)/s, p99 92.3 ms, 199 µs CPU/op |
| insert×64 | 372 (369–372)/s, p99 805.3 ms, 203 µs CPU/op |
| mixed×1 | 4.0 k (3.9 k–4.0 k)/s, p99 3.4 ms, 37 µs CPU/op |
| mixed×8 | 4.6 k (4.6 k–4.7 k)/s, p99 41.9 ms, 33 µs CPU/op |
| mixed×64 | 4.7 k (4.6 k–4.7 k)/s, p99 335.5 ms, 30 µs CPU/op |

## sqldb-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 72.7 (72.3–73.8) | 54.5 (54.3–54.8) | 0.02 (0.02–0.02) | 1 |
| sqlite | 308.9 (308.4–309.2) | 33.7 (33.6–34.1) | 0.01 (0.01–0.01) | 1 |
| ncruces | 215.2 (214.4–215.3) | 35.3 (35.1–35.5) | 0.02 (0.02–0.02) | 1 |
| postgres | 264.7 (263.8–265.4) | 246.5 (246.5–246.9) | 0.38 (0.37–0.38) | 72 |

| stage | tinystore | sqlite | ncruces | postgres |
|---|---|---|---|---|
| get×1 | 120 k (118 k–120 k)/s, p99 29 µs, 13 µs CPU/op | 98 k (97 k–99 k)/s, p99 33 µs, 15 µs CPU/op | 106 k (104 k–109 k)/s, p99 31 µs, 13 µs CPU/op | 7.8 k (7.6 k–7.9 k)/s, p99 180 µs, 167 µs CPU/op |
| get×8 | 276 k (273 k–290 k)/s, p99 360 µs, 18 µs CPU/op | 283 k (280 k–284 k)/s, p99 295 µs, 20 µs CPU/op | 317 k (310 k–318 k)/s, p99 360 µs, 22 µs CPU/op | 58 k (57 k–59 k)/s, p99 393 µs, 124 µs CPU/op |
| get×64 | 249 k (244 k–255 k)/s, p99 1.3 ms, 19 µs CPU/op | 203 k (200 k–205 k)/s, p99 2.0 ms, 33 µs CPU/op | 436 k (415 k–437 k)/s, p99 2.6 ms, 28 µs CPU/op | 268 k (263 k–272 k)/s, p99 1.4 ms, 48 µs CPU/op |
| insert×1 | 370 (366–376)/s, p99 3.9 ms, 233 µs CPU/op | 369 (368–380)/s, p99 3.9 ms, 217 µs CPU/op | 650 (643–655)/s, p99 2.4 ms, 105 µs CPU/op | 590 (498–599)/s, p99 2.9 ms, 227 µs CPU/op |
| insert×8 | 1.7 k (1.7 k–1.7 k)/s, p99 7.9 ms, 65 µs CPU/op | 362 (352–365)/s, p99 92.3 ms, 231 µs CPU/op | 621 (618–635)/s, p99 54.5 ms, 148 µs CPU/op | 2.5 k (2.5 k–2.5 k)/s, p99 4.2 ms, 212 µs CPU/op |
| insert×64 | 11 k (11 k–11 k)/s, p99 11.5 ms, 19 µs CPU/op | 361 (359–364)/s, p99 805.3 ms, 230 µs CPU/op | 621 (570–623)/s, p99 469.8 ms, 145 µs CPU/op | 20 k (20 k–20 k)/s, p99 4.7 ms, 162 µs CPU/op |
| mixed×1 | 4.0 k (3.9 k–4.0 k)/s, p99 3.4 ms, 36 µs CPU/op | 3.9 k (3.9 k–3.9 k)/s, p99 3.4 ms, 39 µs CPU/op | 6.7 k (4.6 k–6.8 k)/s, p99 1.8 ms, 32 µs CPU/op | 3.4 k (3.4 k–3.5 k)/s, p99 2.0 ms, 186 µs CPU/op |
| mixed×8 | 16 k (16 k–16 k)/s, p99 6.8 ms, 23 µs CPU/op | 4.6 k (4.6 k–4.6 k)/s, p99 41.9 ms, 35 µs CPU/op | 8.3 k (8.2 k–8.4 k)/s, p99 23.1 ms, 28 µs CPU/op | 21 k (21 k–21 k)/s, p99 3.4 ms, 133 µs CPU/op |
| mixed×64 | 79 k (77 k–80 k)/s, p99 8.4 ms, 23 µs CPU/op | 4.6 k (3.1 k–4.6 k)/s, p99 369.1 ms, 34 µs CPU/op | 8.0 k (7.9 k–8.2 k)/s, p99 201.3 ms, 30 µs CPU/op | 134 k (133 k–136 k)/s, p99 3.9 ms, 79 µs CPU/op |

## sqldb-served

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 72.9 (72.6–73.0) | 54.5 (52.1–54.5) | 0.02 (0.02–0.02) | 1 |
| tinystore-sidecar | 108.2 (107.9–108.6) | 46.8 (46.5–46.9) | 0.04 (0.04–0.04) | 2 |
| tinystore-server | 109.3 (109.3–109.8) | 50.6 (50.1–51.0) | 0.04 (0.04–0.04) | 2 |

| stage | tinystore | tinystore-sidecar | tinystore-server |
|---|---|---|---|
| get×1 | 122 k (120 k–122 k)/s, p99 29 µs, 13 µs CPU/op | 5.5 k (5.4 k–5.5 k)/s, p99 262 µs, 269 µs CPU/op | 4.3 k (4.2 k–4.3 k)/s, p99 360 µs, 356 µs CPU/op |
| get×8 | 278 k (277 k–278 k)/s, p99 360 µs, 18 µs CPU/op | 48 k (48 k–49 k)/s, p99 393 µs, 77 µs CPU/op | 24 k (24 k–24 k)/s, p99 852 µs, 203 µs CPU/op |
| get×64 | 251 k (245 k–254 k)/s, p99 1.2 ms, 19 µs CPU/op | 227 k (224 k–228 k)/s, p99 1.2 ms, 40 µs CPU/op | 58 k (58 k–58 k)/s, p99 2.6 ms, 149 µs CPU/op |
| insert×1 | 365 (365–375)/s, p99 4.2 ms, 230 µs CPU/op | 341 (327–346)/s, p99 4.2 ms, 428 µs CPU/op | 314 (310–321)/s, p99 5.8 ms, 516 µs CPU/op |
| insert×8 | 1.7 k (1.7 k–1.7 k)/s, p99 7.3 ms, 64 µs CPU/op | 1.0 k (1.0 k–1.0 k)/s, p99 15.7 ms, 192 µs CPU/op | 1.2 k (1.2 k–1.2 k)/s, p99 13.6 ms, 300 µs CPU/op |
| insert×64 | 11 k (9.3 k–11 k)/s, p99 12.6 ms, 19 µs CPU/op | 7.1 k (7.0 k–7.1 k)/s, p99 21.0 ms, 52 µs CPU/op | 9.1 k (9.0 k–9.2 k)/s, p99 14.7 ms, 140 µs CPU/op |
| mixed×1 | 4.0 k (2.5 k–4.0 k)/s, p99 3.4 ms, 35 µs CPU/op | 2.3 k (2.2 k–2.3 k)/s, p99 3.7 ms, 310 µs CPU/op | 2.0 k (2.0 k–2.0 k)/s, p99 3.7 ms, 396 µs CPU/op |
| mixed×8 | 16 k (16 k–17 k)/s, p99 6.8 ms, 23 µs CPU/op | 11 k (11 k–11 k)/s, p99 10.5 ms, 144 µs CPU/op | 11 k (11 k–11 k)/s, p99 8.4 ms, 234 µs CPU/op |
| mixed×64 | 79 k (79 k–80 k)/s, p99 8.4 ms, 22 µs CPU/op | 61 k (60 k–61 k)/s, p99 15.7 ms, 62 µs CPU/op | 44 k (44 k–44 k)/s, p99 13.6 ms, 170 µs CPU/op |

## sqldb

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 73.0 (72.8–73.2) | 54.2 (53.8–55.0) | 0.02 (0.02–0.02) | 1 |
| sqlite | 308.9 (308.4–309.2) | 33.7 (33.6–34.1) | 0.01 (0.01–0.01) | 1 |
| ncruces | 215.2 (214.4–215.3) | 35.3 (35.1–35.5) | 0.02 (0.02–0.02) | 1 |
| postgres | 264.7 (263.8–265.4) | 246.5 (246.5–246.9) | 0.38 (0.37–0.38) | 72 |

| stage | tinystore | sqlite | ncruces | postgres |
|---|---|---|---|---|
| get×1 | 121 k (121 k–123 k)/s, p99 29 µs, 13 µs CPU/op | 98 k (97 k–99 k)/s, p99 33 µs, 15 µs CPU/op | 106 k (104 k–109 k)/s, p99 31 µs, 13 µs CPU/op | 7.8 k (7.6 k–7.9 k)/s, p99 180 µs, 167 µs CPU/op |
| get×8 | 279 k (279 k–286 k)/s, p99 360 µs, 18 µs CPU/op | 283 k (280 k–284 k)/s, p99 295 µs, 20 µs CPU/op | 317 k (310 k–318 k)/s, p99 360 µs, 22 µs CPU/op | 58 k (57 k–59 k)/s, p99 393 µs, 124 µs CPU/op |
| get×64 | 239 k (236 k–252 k)/s, p99 1.3 ms, 20 µs CPU/op | 203 k (200 k–205 k)/s, p99 2.0 ms, 33 µs CPU/op | 436 k (415 k–437 k)/s, p99 2.6 ms, 28 µs CPU/op | 268 k (263 k–272 k)/s, p99 1.4 ms, 48 µs CPU/op |
| insert×1 | 367 (364–378)/s, p99 3.9 ms, 229 µs CPU/op | 369 (368–380)/s, p99 3.9 ms, 217 µs CPU/op | 650 (643–655)/s, p99 2.4 ms, 105 µs CPU/op | 590 (498–599)/s, p99 2.9 ms, 227 µs CPU/op |
| insert×8 | 1.7 k (1.7 k–1.7 k)/s, p99 7.3 ms, 64 µs CPU/op | 362 (352–365)/s, p99 92.3 ms, 231 µs CPU/op | 621 (618–635)/s, p99 54.5 ms, 148 µs CPU/op | 2.5 k (2.5 k–2.5 k)/s, p99 4.2 ms, 212 µs CPU/op |
| insert×64 | 11 k (10 k–11 k)/s, p99 13.6 ms, 20 µs CPU/op | 361 (359–364)/s, p99 805.3 ms, 230 µs CPU/op | 621 (570–623)/s, p99 469.8 ms, 145 µs CPU/op | 20 k (20 k–20 k)/s, p99 4.7 ms, 162 µs CPU/op |
| mixed×1 | 4.0 k (3.9 k–4.1 k)/s, p99 3.4 ms, 36 µs CPU/op | 3.9 k (3.9 k–3.9 k)/s, p99 3.4 ms, 39 µs CPU/op | 6.7 k (4.6 k–6.8 k)/s, p99 1.8 ms, 32 µs CPU/op | 3.4 k (3.4 k–3.5 k)/s, p99 2.0 ms, 186 µs CPU/op |
| mixed×8 | 16 k (16 k–17 k)/s, p99 6.8 ms, 23 µs CPU/op | 4.6 k (4.6 k–4.6 k)/s, p99 41.9 ms, 35 µs CPU/op | 8.3 k (8.2 k–8.4 k)/s, p99 23.1 ms, 28 µs CPU/op | 21 k (21 k–21 k)/s, p99 3.4 ms, 133 µs CPU/op |
| mixed×64 | 79 k (79 k–80 k)/s, p99 8.4 ms, 23 µs CPU/op | 4.6 k (3.1 k–4.6 k)/s, p99 369.1 ms, 34 µs CPU/op | 8.0 k (7.9 k–8.2 k)/s, p99 201.3 ms, 30 µs CPU/op | 134 k (133 k–136 k)/s, p99 3.9 ms, 79 µs CPU/op |

## stack-before-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 108.5 (105.8–110.0) | 54.6 (54.3–54.9) | 0.12 (0.12–0.16) | 1 |
| tinystore-sidecar | 146.7 (143.6–148.4) | 43.6 (43.4–44.2) | 0.10 (0.10–0.11) | 2 |
| tinystore-server | 146.7 (145.7–147.1) | 38.5 (38.2–38.6) | 0.11 (0.11–0.12) | 2 |
| services | 151.3 (151.3–151.8) | 72.6 (70.8–74.0) | 0.54 (0.54–0.57) | 12 |

| stage | tinystore | tinystore-sidecar | tinystore-server | services |
|---|---|---|---|---|
| request×8 | 4.3 k (4.0 k–4.3 k)/s, p99 18.9 ms, 71 µs CPU/op | 3.5 k (3.5 k–3.6 k)/s, p99 25.2 ms, 417 µs CPU/op | 3.5 k (3.5 k–3.5 k)/s, p99 23.1 ms, 528 µs CPU/op | 4.8 k (4.7 k–4.9 k)/s, p99 18.9 ms, 293 µs CPU/op |
| request×64 | 29 k (29 k–29 k)/s, p99 23.1 ms, 53 µs CPU/op | 22 k (22 k–22 k)/s, p99 37.7 ms, 174 µs CPU/op | 19 k (19 k–19 k)/s, p99 33.6 ms, 333 µs CPU/op | 5.8 k (5.1 k–6.1 k)/s, p99 41.9 ms, 479 µs CPU/op |
| backup-bytes×1 | 0/s, p99 0 µs, 0 µs CPU/op | — | — | 0/s, p99 0 µs, 0 µs CPU/op |
| restore×1 | 0/s, p99 0 µs, 160 µs CPU/op | — | — | 0/s, p99 0 µs, 1470 µs CPU/op |

## stack-latency-before-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 166.8 | 281.8 | 0.13 | 1 |
| tinystore-sidecar | 228.3 | 216.9 | 0.10 | 2 |
| tinystore-server | 213.2 | 176.0 | 0.11 | 2 |
| services | 195.0 | 202.3 | 0.63 | 12 |

| stage | tinystore | tinystore-sidecar | tinystore-server | services |
|---|---|---|---|---|
| request-max×64 | 29 k/s, p99 23.1 ms, 53 µs CPU/op | 22 k/s, p99 37.7 ms, 171 µs CPU/op | 18 k/s, p99 37.7 ms, 331 µs CPU/op | 8.0 k/s, p99 37.7 ms, 357 µs CPU/op |
| request-25%×1024 | 7.3 k/s, p99 23.1 ms, 92 µs CPU/op | 5.6 k/s, p99 31.5 ms, 404 µs CPU/op | 4.5 k/s, p99 27.3 ms, 557 µs CPU/op | 2.0 k/s, p99 13.6 ms, 455 µs CPU/op |
| request-50%×1024 | 15 k/s, p99 23.1 ms, 78 µs CPU/op | 11 k/s, p99 33.6 ms, 279 µs CPU/op | 9.1 k/s, p99 29.4 ms, 449 µs CPU/op | 4.0 k/s, p99 18.9 ms, 431 µs CPU/op |
| request-75%×1024 | 22 k/s, p99 25.2 ms, 70 µs CPU/op | 17 k/s, p99 37.7 ms, 226 µs CPU/op | 14 k/s, p99 33.6 ms, 390 µs CPU/op | 6.0 k/s, p99 41.9 ms, 434 µs CPU/op |
| request-90%×1024 | 26 k/s, p99 25.2 ms, 66 µs CPU/op | 20 k/s, p99 33.6 ms, 203 µs CPU/op | 16 k/s, p99 41.9 ms, 364 µs CPU/op | 7.2 k/s, p99 41.9 ms, 369 µs CPU/op |

## stack-latency-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 162.8 | 285.4 | 0.12 | 1 |
| services | 324.7 | 751.4 | 0.65 | 74 |

| stage | tinystore | services |
|---|---|---|
| request-max×64 | 30 k/s, p99 23.1 ms, 52 µs CPU/op | 39 k/s, p99 21.0 ms, 183 µs CPU/op |
| request-25%×1024 | 7.4 k/s, p99 23.1 ms, 90 µs CPU/op | 9.7 k/s, p99 18.9 ms, 273 µs CPU/op |
| request-50%×1024 | 15 k/s, p99 31.5 ms, 78 µs CPU/op | 19 k/s, p99 21.0 ms, 241 µs CPU/op |
| request-75%×1024 | 22 k/s, p99 25.2 ms, 70 µs CPU/op | 29 k/s, p99 21.0 ms, 216 µs CPU/op |
| request-90%×1024 | 27 k/s, p99 27.3 ms, 67 µs CPU/op | 35 k/s, p99 21.0 ms, 200 µs CPU/op |

## stack-latency-superseded-0308

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 163.6 | 284.2 | 0.12 | 1 |
| tinystore-sidecar | 222.6 | 216.8 | 0.10 | 2 |
| tinystore-server | 215.2 | 185.7 | 0.11 | 2 |
| services | 193.5 | 148.7 | 0.63 | 12 |

| stage | tinystore | tinystore-sidecar | tinystore-server | services |
|---|---|---|---|---|
| request-max×64 | 30 k/s, p99 23.1 ms, 52 µs CPU/op | 22 k/s, p99 37.7 ms, 169 µs CPU/op | 19 k/s, p99 33.6 ms, 319 µs CPU/op | 5.7 k/s, p99 41.9 ms, 474 µs CPU/op |
| request-25%×1024 | 7.4 k/s, p99 23.1 ms, 91 µs CPU/op | 5.6 k/s, p99 31.5 ms, 402 µs CPU/op | 4.8 k/s, p99 27.3 ms, 544 µs CPU/op | 1.4 k/s, p99 13.6 ms, 469 µs CPU/op |
| request-50%×1024 | 15 k/s, p99 23.1 ms, 77 µs CPU/op | 11 k/s, p99 33.6 ms, 276 µs CPU/op | 9.6 k/s, p99 31.5 ms, 440 µs CPU/op | 2.9 k/s, p99 15.7 ms, 448 µs CPU/op |
| request-75%×1024 | 22 k/s, p99 41.9 ms, 69 µs CPU/op | 17 k/s, p99 50.3 ms, 224 µs CPU/op | 14 k/s, p99 33.6 ms, 378 µs CPU/op | 4.3 k/s, p99 23.1 ms, 438 µs CPU/op |
| request-90%×1024 | 27 k/s, p99 25.2 ms, 68 µs CPU/op | 20 k/s, p99 33.6 ms, 201 µs CPU/op | 17 k/s, p99 33.6 ms, 353 µs CPU/op | — |

- services #1 request-90%×1024: 1811 errors, first: failed to connect to `user=postgres database=postgres`: /data/compare-services-4202591656/postgres/.s.PGSQL.5432 (/data/compare-services-4202591656/postgres): dial error: timeout: dial unix /data/comp

## stack-latency

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 166.8 | 281.8 | 0.13 | 1 |
| tinystore-sidecar | 228.3 | 216.9 | 0.10 | 2 |
| tinystore-server | 213.2 | 176.0 | 0.11 | 2 |
| services | 324.7 | 751.4 | 0.65 | 74 |

| stage | tinystore | tinystore-sidecar | tinystore-server | services |
|---|---|---|---|---|
| request-max×64 | 29 k/s, p99 23.1 ms, 53 µs CPU/op | 22 k/s, p99 37.7 ms, 171 µs CPU/op | 18 k/s, p99 37.7 ms, 331 µs CPU/op | 39 k/s, p99 21.0 ms, 183 µs CPU/op |
| request-25%×1024 | 7.3 k/s, p99 23.1 ms, 92 µs CPU/op | 5.6 k/s, p99 31.5 ms, 404 µs CPU/op | 4.5 k/s, p99 27.3 ms, 557 µs CPU/op | 9.7 k/s, p99 18.9 ms, 273 µs CPU/op |
| request-50%×1024 | 15 k/s, p99 23.1 ms, 78 µs CPU/op | 11 k/s, p99 33.6 ms, 279 µs CPU/op | 9.1 k/s, p99 29.4 ms, 449 µs CPU/op | 19 k/s, p99 21.0 ms, 241 µs CPU/op |
| request-75%×1024 | 22 k/s, p99 25.2 ms, 70 µs CPU/op | 17 k/s, p99 37.7 ms, 226 µs CPU/op | 14 k/s, p99 33.6 ms, 390 µs CPU/op | 29 k/s, p99 21.0 ms, 216 µs CPU/op |
| request-90%×1024 | 26 k/s, p99 25.2 ms, 66 µs CPU/op | 20 k/s, p99 33.6 ms, 203 µs CPU/op | 16 k/s, p99 41.9 ms, 364 µs CPU/op | 35 k/s, p99 21.0 ms, 200 µs CPU/op |

## stack-pools

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 108.1 (107.8–108.7) | 55.8 (55.6–55.8) | 0.12 (0.12–0.12) | 1 |
| services | 264.3 (256.3–264.7) | 155.2 (154.8–155.6) | 0.59 (0.58–0.59) | 74 |

| stage | tinystore | services |
|---|---|---|
| request×8 | 4.4 k (4.3 k–4.4 k)/s, p99 18.9 ms, 71 µs CPU/op | 5.7 k (5.6 k–5.8 k)/s, p99 18.9 ms, 273 µs CPU/op |
| request×64 | 30 k (30 k–30 k)/s, p99 23.1 ms, 52 µs CPU/op | 39 k (39 k–39 k)/s, p99 21.0 ms, 184 µs CPU/op |
| backup-bytes×1 | 0/s, p99 0 µs, 0 µs CPU/op | 0/s, p99 0 µs, 0 µs CPU/op |
| restore×1 | 0/s, p99 0 µs, 130 µs CPU/op | 0/s, p99 0 µs, 4550 µs CPU/op |

## stack

|  | peak MiB | disk MiB | ready s | processes |
|---|---|---|---|---|
| tinystore | 108.5 (105.8–110.0) | 54.6 (54.3–54.9) | 0.12 (0.12–0.16) | 1 |
| tinystore-sidecar | 146.7 (143.6–148.4) | 43.6 (43.4–44.2) | 0.10 (0.10–0.11) | 2 |
| tinystore-server | 146.7 (145.7–147.1) | 38.5 (38.2–38.6) | 0.11 (0.11–0.12) | 2 |
| services | 264.3 (256.3–264.7) | 155.2 (154.8–155.6) | 0.59 (0.58–0.59) | 74 |

| stage | tinystore | tinystore-sidecar | tinystore-server | services |
|---|---|---|---|---|
| request×8 | 4.3 k (4.0 k–4.3 k)/s, p99 18.9 ms, 71 µs CPU/op | 3.5 k (3.5 k–3.6 k)/s, p99 25.2 ms, 417 µs CPU/op | 3.5 k (3.5 k–3.5 k)/s, p99 23.1 ms, 528 µs CPU/op | 5.7 k (5.6 k–5.8 k)/s, p99 18.9 ms, 273 µs CPU/op |
| request×64 | 29 k (29 k–29 k)/s, p99 23.1 ms, 53 µs CPU/op | 22 k (22 k–22 k)/s, p99 37.7 ms, 174 µs CPU/op | 19 k (19 k–19 k)/s, p99 33.6 ms, 333 µs CPU/op | 39 k (39 k–39 k)/s, p99 21.0 ms, 184 µs CPU/op |
| backup-bytes×1 | 0/s, p99 0 µs, 0 µs CPU/op | — | — | 0/s, p99 0 µs, 0 µs CPU/op |
| restore×1 | 0/s, p99 0 µs, 160 µs CPU/op | — | — | 0/s, p99 0 µs, 4550 µs CPU/op |

## weight

```json
[
 {
  "program": "badger",
  "bytes": 8315040,
  "added_bytes": 7008256
 },
 {
  "program": "bbolt",
  "bytes": 1953952,
  "added_bytes": 647168
 },
 {
  "program": "empty",
  "bytes": 1306784,
  "added_bytes": 0
 },
 {
  "program": "mattn",
  "bytes": 3780488,
  "added_bytes": 2473704,
  "cgo": true
 },
 {
  "program": "pebble",
  "bytes": 13680800,
  "added_bytes": 12374016
 },
 {
  "program": "redis",
  "bytes": 7225504,
  "added_bytes": 5918720
 },
 {
  "program": "sqlite",
  "bytes": 6656160,
  "added_bytes": 5349376
 },
 {
  "program": "tinystore",
  "bytes": 7979168,
  "added_bytes": 6672384
 }
]
```

## failures

```text
00:47:31 failed: /tmp/compare run -engine records -repeats 3 -seconds 5 -dir /data -out results/2026-09-30-wsl2/records.json
```
