## At rest

| contender          | PSS, all processes | program | store or services                                                             | page cache | open, ms | ready, s                                                                 |
|--------------------|--------------------|---------|-------------------------------------------------------------------------------|------------|----------|--------------------------------------------------------------------------|
| bun                | 38.2               | 38.2    |                                                                               | 79.4       | 0.33     |                                                                          |
| tinystore-embedded | 55.6               | 55.6    |                                                                               | 94.8       | 126      |                                                                          |
| tinystore-sidecar  | 60.5               | 46.4    | tinystore 14.1                                                                | 98.8       | 134      |                                                                          |
| tinystore-serve    | 59.1               | 44.5    | tinystore 14.7                                                                | 98.9       | 112      | tinystore 0.02                                                           |
| services           | 445.1              | 42.8    | loki 133.0 + postgres 126.0 + minio 108.0 + victoria-metrics 26.4 + redis 8.6 | 544.6      | 28       | postgres 0.07, redis 0.01, minio 0.37, victoria-metrics 0.06, loki 15.51 |
| valkey             | 48.8               | 39.3    | valkey 9.4                                                                    | 92.2       | 1.27     | valkey 0.06                                                              |
| prometheus         | 139.8              | 39.1    | prometheus 100.5                                                              | 182.2      | 1.52     | prometheus 0.20                                                          |

## An application

| contender                 | stage       | calls/s | p50 µs | p99 µs | peak PSS MiB | CPU µs/call |
|---------------------------|-------------|---------|--------|--------|--------------|-------------|
| tinystore-embedded        | request@1   | 788     | 60     | 14252  | 108.6        | 2183        |
| tinystore-embedded        | request@8   | 4.0 k   | 71     | 19182  | 140.4        | 448         |
| tinystore-embedded        | request@64  | 28 k    | 288    | 23850  | 195.8        | 86          |
| tinystore-embedded        | request@256 | 39 k    | 1967   | 54789  | 211.6        | 67          |
| tinystore-embedded-joined | request@1   | 1.1 k   | 59     | 9783   | 112.4        | 1545        |
| tinystore-embedded-joined | request@8   | 2.4 k   | 58     | 38361  | 132.3        | 721         |
| tinystore-embedded-joined | request@64  | 2.8 k   | 17033  | 136239 | 159.1        | 626         |
| tinystore-embedded-joined | request@256 | 2.8 k   | 83042  | 241940 | 166.1        | 614         |
| tinystore-sidecar         | request@1   | 756     | 306    | 13699  | 108.6        | 541         |
| tinystore-sidecar         | request@8   | 3.7 k   | 325    | 19957  | 145.4        | 302         |
| tinystore-sidecar         | request@64  | 26 k    | 420    | 24327  | 204.7        | 127         |
| tinystore-sidecar         | request@256 | 47 k    | 1967   | 35440  | 215.8        | 65          |
| services                  | request@1   | 2.0 k   | 172    | 5619   | 659.7        | 352         |
| services                  | request@8   | 8.8 k   | 272    | 10801  | 789.0        | 229         |
| services                  | request@64  | 15 k    | 2755   | 22924  | 878.0        | 191         |
| services                  | request@256 | 21 k    | 9403   | 38361  | 927.7        | 155         |

| contender                 | at rest, all | program | store or services | page cache | disk MiB |
|---------------------------|--------------|---------|-------------------|------------|----------|
| tinystore-embedded        | 74.8         | 74.8    | 0.0               | 116.4      | 190.9    |
| tinystore-embedded-joined | 73.4         | 73.4    | 0.0               | 105.7      | 60.0     |
| tinystore-sidecar         | 75.7         | 58.7    | 16.4              | 109.7      | 194.1    |
| services                  | 469.2        | 51.2    | 417.4             | 555.3      | 748.6    |

| contender                 | requests | writes | jobs left | logs stored | logs dropped | versions |
|---------------------------|----------|--------|-----------|-------------|--------------|----------|
| tinystore-embedded        | 1598575  | 159592 | 0         | 1598575     | 0            | 159592   |
| tinystore-embedded        | 1584082  | 158078 | 0         | 1584082     | 0            | 158078   |
| tinystore-embedded        | 1471326  | 146969 | 0         | 1471326     | 0            | 146969   |
| tinystore-embedded-joined | 202320   | 20019  | 0         | 202320      | 0            | 20019    |
| tinystore-embedded-joined | 200798   | 19878  | 0         | 200798      | 0            | 19878    |
| tinystore-embedded-joined | 178217   | 17581  | 0         | 178217      | 0            | 17581    |
| tinystore-sidecar         | 1702856  | 170008 | 0         | 1702856     | 0            | 170008   |
| tinystore-sidecar         | 1557866  | 155376 | 0         | 1557866     | 0            | 155376   |
| tinystore-sidecar         | 1694340  | 169180 | 0         | 1694340     | 0            | 169180   |
| services                  | 1047282  | 104593 | 0         | 1047282     | 0            | 104593   |
| services                  | 1043253  | 104183 | 0         | 1043253     | 0            | 104183   |
| services                  | 1037779  | 103644 | 0         | 1037779     | 0            | 103644   |

## KV

| contender                  | stage   | calls/s | p50 µs | p99 µs | peak PSS MiB | CPU µs/call |
|----------------------------|---------|---------|--------|--------|--------------|-------------|
| tinystore-embedded-synced  | get@1   | 141 k   | 6.2    | 19     | 94.3         | 7.4         |
| tinystore-embedded-synced  | get@8   | 159 k   | 33     | 246    | 100.8        | 21          |
| tinystore-embedded-synced  | get@64  | 352 k   | 111    | 2260   | 121.9        | 12          |
| tinystore-embedded-synced  | get@256 | 394 k   | 404    | 3934   | 123.2        | 11          |
| tinystore-embedded-synced  | set@1   | 358     | 3228   | 4519   | 99.9         | 4620        |
| tinystore-embedded-synced  | set@8   | 2.3 k   | 3425   | 9038   | 99.7         | 758         |
| tinystore-embedded-synced  | set@64  | 12 k    | 4093   | 17033  | 101.6        | 157         |
| tinystore-embedded-synced  | set@256 | 23 k    | 9783   | 24813  | 103.4        | 86          |
| tinystore-sidecar-synced   | get@1   | 11 k    | 86     | 176    | 90.8         | 110         |
| tinystore-sidecar-synced   | get@8   | 73 k    | 88     | 396    | 95.0         | 32          |
| tinystore-sidecar-synced   | get@64  | 217 k   | 214    | 2545   | 110.3        | 22          |
| tinystore-sidecar-synced   | get@256 | 322 k   | 522    | 4175   | 112.7        | 16          |
| tinystore-sidecar-synced   | set@1   | 344     | 3228   | 4013   | 89.5         | 421         |
| tinystore-sidecar-synced   | set@8   | 2.2 k   | 3563   | 9038   | 99.5         | 96          |
| tinystore-sidecar-synced   | set@64  | 12 k    | 4259   | 16699  | 101.8        | 32          |
| tinystore-sidecar-synced   | set@256 | 21 k    | 10382  | 26332  | 103.1        | 29          |
| redis-synced               | get@1   | 20 k    | 43     | 128    | 87.9         | 67          |
| redis-synced               | get@8   | 143 k   | 50     | 128    | 90.1         | 11          |
| redis-synced               | get@64  | 357 k   | 166    | 373    | 94.6         | 4.9         |
| redis-synced               | get@256 | 406 k   | 600    | 1646   | 98.6         | 4.4         |
| redis-synced               | set@1   | 305     | 3292   | 3782   | 82.6         | 171         |
| redis-synced               | set@8   | 1.2 k   | 6455   | 7127   | 81.2         | 47          |
| redis-synced               | set@64  | 9.7 k   | 6715   | 8350   | 85.8         | 13          |
| redis-synced               | set@256 | 37 k    | 6850   | 9979   | 97.1         | 8.2         |
| valkey-synced              | get@1   | 20 k    | 42     | 93     | 86.2         | 66          |
| valkey-synced              | get@8   | 143 k   | 50     | 123    | 89.7         | 11          |
| valkey-synced              | get@64  | 355 k   | 166    | 388    | 92.0         | 4.9         |
| valkey-synced              | get@256 | 412 k   | 588    | 1614   | 96.7         | 4.4         |
| valkey-synced              | set@1   | 305     | 3292   | 4013   | 84.6         | 167         |
| valkey-synced              | set@8   | 1.3 k   | 6328   | 7269   | 80.2         | 48          |
| valkey-synced              | set@64  | 9.9 k   | 6584   | 8186   | 83.9         | 13          |
| valkey-synced              | set@256 | 37 k    | 6850   | 9979   | 92.6         | 8.5         |
| tinystore-embedded-written | set@1   | 16 k    | 26     | 103    | 90.9         | 121         |
| tinystore-embedded-written | set@8   | 20 k    | 198    | 4344   | 90.0         | 118         |
| tinystore-embedded-written | set@64  | 29 k    | 873    | 19957  | 90.6         | 75          |
| tinystore-embedded-written | set@256 | 33 k    | 4013   | 25310  | 94.7         | 63          |
| tinystore-sidecar-written  | set@1   | 5.5 k   | 150    | 318    | 82.3         | 287         |
| tinystore-sidecar-written  | set@8   | 17 k    | 266    | 13167  | 82.4         | 109         |
| tinystore-sidecar-written  | set@64  | 29 k    | 823    | 19957  | 85.0         | 35          |
| tinystore-sidecar-written  | set@256 | 32 k    | 4796   | 24813  | 86.1         | 29          |
| redis-written              | set@1   | 15 k    | 64     | 133    | 92.1         | 84          |
| redis-written              | set@8   | 101 k   | 68     | 214    | 113.8        | 16          |
| redis-written              | set@64  | 189 k   | 283    | 1377   | 117.2        | 9.6         |
| redis-written              | set@256 | 205 k   | 1044   | 2495   | 121.3        | 9.1         |
| valkey-written             | set@1   | 14 k    | 64     | 138    | 91.7         | 85          |
| valkey-written             | set@8   | 100 k   | 68     | 251    | 110.5        | 16          |
| valkey-written             | set@64  | 192 k   | 283    | 1350   | 114.8        | 9.4         |
| valkey-written             | set@256 | 204 k   | 1065   | 2596   | 118.8        | 9.1         |

## SQL

| contender          | stage      | calls/s | p50 µs | p99 µs | peak PSS MiB | CPU µs/call |
|--------------------|------------|---------|--------|--------|--------------|-------------|
| tinystore-embedded | insert@1   | 363     | 3164   | 4344   | 114.9        | 4553        |
| tinystore-embedded | insert@8   | 2.5 k   | 3425   | 7563   | 110.5        | 692         |
| tinystore-embedded | insert@64  | 14 k    | 3934   | 10590  | 109.6        | 129         |
| tinystore-embedded | insert@256 | 33 k    | 7714   | 17373  | 111.7        | 54          |
| tinystore-embedded | read@1     | 32 k    | 28     | 80     | 107.7        | 66          |
| tinystore-embedded | read@8     | 123 k   | 46     | 1086   | 117.0        | 23          |
| tinystore-embedded | read@64    | 180 k   | 218    | 2305   | 127.8        | 18          |
| tinystore-embedded | read@256   | 167 k   | 927    | 4344   | 128.4        | 19          |
| tinystore-sidecar  | insert@1   | 342     | 3292   | 3857   | 98.7         | 453         |
| tinystore-sidecar  | insert@8   | 2.3 k   | 3563   | 8517   | 101.6        | 82          |
| tinystore-sidecar  | insert@64  | 12 k    | 4175   | 11238  | 105.9        | 30          |
| tinystore-sidecar  | insert@256 | 26 k    | 9979   | 19957  | 110.5        | 28          |
| tinystore-sidecar  | read@1     | 6.9 k   | 136    | 246    | 98.6         | 281         |
| tinystore-sidecar  | read@8     | 46 k    | 159    | 1324   | 100.6        | 70          |
| tinystore-sidecar  | read@64    | 136 k   | 331    | 2596   | 111.3        | 30          |
| tinystore-sidecar  | read@256   | 152 k   | 1086   | 4519   | 114.2        | 26          |
| postgres           | insert@1   | 599     | 1782   | 2215   | 227.5        | 242         |
| postgres           | insert@8   | 2.5 k   | 3164   | 3934   | 235.5        | 187         |
| postgres           | insert@64  | 18 k    | 3358   | 5295   | 302.5        | 136         |
| postgres           | insert@256 | 20 k    | 12656  | 18075  | 324.9        | 106         |
| postgres           | read@1     | 11 k    | 81     | 172    | 242.3        | 112         |
| postgres           | read@8     | 26 k    | 256    | 891    | 236.3        | 74          |
| postgres           | read@64    | 27 k    | 2215   | 4519   | 245.0        | 74          |
| postgres           | read@256   | 62 k    | 3707   | 8026   | 245.2        | 33          |

## Files

| contender          | stage      | calls/s | p50 µs | p99 µs | peak PSS MiB | CPU µs/call |
|--------------------|------------|---------|--------|--------|--------------|-------------|
| tinystore-embedded | get-16k@1  | 17 k    | 43     | 499    | 94.1         | 125         |
| tinystore-embedded | get-16k@16 | 34 k    | 206    | 3102   | 112.1        | 91          |
| tinystore-embedded | get-16k@64 | 30 k    | 1987   | 4749   | 135.1        | 100         |
| tinystore-embedded | get-1m@1   | 534     | 1447   | 5140   | 210.4        | 3729        |
| tinystore-embedded | get-1m@16  | 990     | 14113  | 34418  | 525.6        | 2435        |
| tinystore-embedded | put-16k@1  | 252     | 4505   | 12358  | 123.6        | 6471        |
| tinystore-embedded | put-16k@16 | 2.2 k   | 5756   | 20372  | 129.0        | 852         |
| tinystore-embedded | put-16k@64 | 2.8 k   | 19957  | 35440  | 136.6        | 673         |
| tinystore-embedded | put-1m@1   | 84      | 11692  | 15893  | 370.7        | 20557       |
| tinystore-embedded | put-1m@16  | 495     | 31173  | 60080  | 432.6        | 4584        |
| tinystore-sidecar  | get-16k@1  | 5.1 k   | 174    | 799    | 84.5         | 359         |
| tinystore-sidecar  | get-16k@16 | 28 k    | 297    | 3325   | 112.9        | 125         |
| tinystore-sidecar  | get-16k@64 | 21 k    | 2982   | 5401   | 138.3        | 140         |
| tinystore-sidecar  | get-1m@1   | 369     | 2446   | 6143   | 213.6        | 4111        |
| tinystore-sidecar  | get-1m@16  | 472     | 33396  | 44074  | 347.1        | 4794        |
| tinystore-sidecar  | put-16k@1  | 157     | 6146   | 14982  | 124.1        | 631         |
| tinystore-sidecar  | put-16k@16 | 1.6 k   | 7947   | 26864  | 130.6        | 254         |
| tinystore-sidecar  | put-16k@64 | 2.8 k   | 20979  | 41698  | 142.2        | 215         |
| tinystore-sidecar  | put-1m@1   | 82      | 12045  | 16053  | 270.4        | 4134        |
| tinystore-sidecar  | put-1m@16  | 425     | 36177  | 76778  | 343.2        | 3432        |
| minio              | get-16k@1  | 2.5 k   | 388    | 656    | 354.7        | 634         |
| minio              | get-16k@16 | 25 k    | 548    | 1987   | 453.8        | 403         |
| minio              | get-16k@64 | 36 k    | 1567   | 5348   | 551.2        | 347         |
| minio              | get-1m@1   | 872     | 1013   | 2701   | 579.1        | 2044        |
| minio              | get-1m@16  | 2.7 k   | 5789   | 12286  | 615.9        | 2496        |
| minio              | put-16k@1  | 78      | 12782  | 16063  | 480.0        | 2076        |
| minio              | put-16k@16 | 698     | 24570  | 55793  | 467.6        | 2571        |
| minio              | put-16k@64 | 606     | 92711  | 297873 | 495.2        | 3892        |
| minio              | put-1m@1   | 76      | 13169  | 16574  | 553.0        | 4767        |
| minio              | put-1m@16  | 276     | 51629  | 134223 | 675.7        | 5664        |

## A million keys

| contender          | stage  | calls/s | p50 µs | p99 µs | peak PSS MiB | CPU µs/call |
|--------------------|--------|---------|--------|--------|--------------|-------------|
| tinystore-embedded | get@64 | 349 k   | 117    | 2239   | 119.6        | 13          |
| tinystore-sidecar  | get@64 | 214 k   | 223    | 2495   | 113.0        | 23          |
| redis              | get@64 | 367 k   | 161    | 344    | 280.7        | 4.7         |
| valkey             | get@64 | 364 k   | 161    | 366    | 285.6        | 4.8         |

| contender          | at rest, all | program | store or services | page cache | disk MiB |
|--------------------|--------------|---------|-------------------|------------|----------|
| tinystore-embedded | 65.1         | 65.1    | 0.0               | 263.9      | 162.3    |
| tinystore-sidecar  | 72.5         | 54.6    | 17.9              | 268.3      | 162.3    |
| redis              | 256.3        | 45.1    | 211.3             | 98.2       | 132.5    |
| valkey             | 259.2        | 45.0    | 214.2             | 97.9       | 132.5    |

## Metrics

| case            | samples  | ingest/s | settle s | disk MiB | B/sample | payload B/sample | memory ingest | memory settled |
|-----------------|----------|----------|----------|----------|----------|------------------|---------------|----------------|
| tinystore-wide  | 5090400  | 1.48 M   | 0.18     | 7.3      | 1.50     | 0.68             | 17.6          | 16.5           |
| victoria-wide   | 5090400  | 2.15 M   | 3.11     | 5.9      | 1.22     |                  | 325.4         | 271.6          |
| prometheus-wide | 5090400  | 301 k    | 151      | 8.6      | 1.78     |                  | 1278.0        | 96.3           |
| tinystore-long  | 26179200 | 1.43 M   | 0.11     | 24.9     | 1.00     | 0.70             | 14.7          |                |
| victoria-long   | 26179200 | 2.09 M   | 2.61     | 17.8     | 0.71     |                  | 404.2         | 257.9          |
| prometheus-long | 26179200 | 50 k     | 212      | 39.5     | 1.58     |                  | 6069.0        | 104.0          |

Query medians, ms:

| case            | cpu-max-all-8 | double-groupby-1 | raw-1-host-all | single-groupby-1-1-1 | single-groupby-1-8-1 | single-groupby-5-1-12 |
|-----------------|---------------|------------------|----------------|----------------------|----------------------|-----------------------|
| tinystore-wide  | 1.67          | 0.36             | 0.08           | 0.05                 | 0.24                 | 0.82                  |
| victoria-wide   | 3.77          | 0.46             | 0.54           | 0.28                 | 0.38                 | 1.64                  |
| prometheus-wide | 12            | 2.68             | 0.57           | 0.33                 | 0.45                 | 2.48                  |
| tinystore-long  | 2.20          | 0.34             | 0.34           | 0.05                 | 0.20                 | 1.28                  |
| victoria-long   | 4.30          | 0.47             | 2.63           | 0.29                 | 0.41                 | 1.87                  |
| prometheus-long | 14            | 2.27             | 4.03           | 0.33                 | 0.47                 | 3.15                  |

## Logs

| dataset    | lines    | raw MiB | gzip -6 | zstd -19 | TinyStore file | TinyStore pages | Loki chunks+index | TinyStore lines/s | Loki lines/s | TinyStore MiB | Loki MiB |
|------------|----------|---------|---------|----------|----------------|-----------------|-------------------|-------------------|--------------|---------------|----------|
| Linux      | 25567    | 2.2     | 9.0 %   | 5.1 %    | 23.4 %         | 11.9 %          | 26.1 %            | 1.76 M            | 213 k        | 13.5          | 253.9    |
| Proxifier  | 21329    | 2.4     | 6.4 %   | 4.8 %    | 20.7 %         | 10.4 %          | 15.1 %            | 1.80 M            | 200 k        | 12.1          | 226.4    |
| Apache     | 56482    | 4.9     | 4.7 %   | 3.5 %    | 13.9 %         | 8.6 %           | 15.2 %            | 2.19 M            | 245 k        | 13.3          | 256.3    |
| Zookeeper  | 74380    | 9.9     | 4.0 %   | 2.6 %    | 9.3 %          | 4.9 %           | 9.2 %             | 1.58 M            | 209 k        | 17.0          | 225.3    |
| Mac        | 117283   | 16.1    | 8.6 %   | 3.7 %    | 14.2 %         | 9.2 %           | 14.3 %            | 1.17 M            | 189 k        | 17.5          | 219.0    |
| HealthApp  | 253395   | 22.4    | 9.2 %   | 6.1 %    | 18.6 %         | 13.3 %          | 19.2 %            | 1.35 M            | 240 k        | 13.9          | 237.3    |
| HPC        | 433490   | 32.0    | 9.0 %   | 5.9 %    | 18.4 %         | 14.6 %          | 23.7 %            | 1.49 M            | 276 k        | 16.1          | 374.6    |
| Hadoop     | 393421   | 46.3    | 5.0 %   | 2.3 %    | 10.5 %         | 6.5 %           | 12.0 %            | 1.49 M            | 219 k        | 20.3          | 226.9    |
| OpenStack  | 207820   | 58.6    | 8.5 %   | 6.2 %    | 12.8 %         | 8.3 %           | 12.3 %            | 809 k             | 136 k        | 19.8          | 243.2    |
| SSH        | 655147   | 70.0    | 6.0 %   | 4.1 %    | 11.2 %         | 7.6 %           | 14.1 %            | 1.52 M            | 230 k        | 13.5          | 252.5    |
| Android_v1 | 1555005  | 183.4   | 13.0 %  | 4.4 %    | 18.5 %         | 15.0 %          | 27.6 %            | 1.09 M            | 186 k        | 18.3          | 466.2    |
| BGL        | 4747963  | 708.8   | 7.8 %   | 4.6 %    | 14.0 %         | 9.1 %           | 17.4 %            | 1.15 M            | 207 k        | 20.6          | 695.7    |
| HDFS_v1    | 11175629 | 1504.9  | 9.6 %   | 6.2 %    | 14.0 %         | 10.0 %          | 19.9 %            | 1.17 M            | 205 k        | 18.1          | 1284.9   |

Query medians, ms:

| dataset    | needle-count TS / Loki | needle-lines TS / Loki | error-count TS / Loki | newest-100 TS / Loki | minute TS / Loki |
|------------|------------------------|------------------------|-----------------------|----------------------|------------------|
| Linux      | 8.88 / 2795            | 9.25 / 2294            | 7.58 / 2611           | 0.13 / 48            | 0.10 / 1.18      |
| Proxifier  | 5.39 / 1165            | 1.40 / 436             | 5.21 / 1120           | 0.06 / 1.78          | 0.13 / 1.49      |
| Apache     | 16 / 2521              | 0.67 / 730             | 18 / 2362             | 0.11 / 32            | 0.09 / 1.60      |
| Zookeeper  | 26 / 270               | 0.34 / 357             | 24 / 317              | 0.08 / 1.89          | 1.15 / 10        |
| Mac        | 41 / 94                | 17 / 626               | 42 / 139              | 0.07 / 2.12          | 0.16 / 2.01      |
| HealthApp  | 82 / 205               | 86 / 180               | 57 / 184              | 0.28 / 3.68          | 0.29 / 3.47      |
| HPC        | 94 / 18769             | 104 / 17170            | 106 / 14517           | 0.15 / 64            | 0.18 / 1.11      |
| Hadoop     | 110 / 87               | 46 / 91                | 110 / 85              | 0.19 / 7.30          | 2.00 / 17        |
| OpenStack  | 130 / 57               | 0.42 / 361             | 96 / 195              | 0.06 / 1.97          | 0.18 / 2.49      |
| SSH        | 209 / 539              | 101 / 1599             | 208 / 537             | 0.27 / 7.73          | 0.13 / 2.12      |
| Android_v1 | 463 / 274              | 526 / 273              | 470 / 298             | 0.26 / 14            | 0.22 / 13        |
| BGL        | 1255 / 14786           | 1672 / 14540           | 1294 / 9644           | 0.27 / 122           | 0.73 / 23        |
| HDFS_v1    | 3078 / 1396            | 4726 / 1428            | 3032 / 1747           | 0.13 / 13            | 2.64 / 33        |

