## At rest

| contender          | PSS, all processes | program | store or services                                                             | page cache | open, ms | ready, s                                                                 |
|--------------------|--------------------|---------|-------------------------------------------------------------------------------|------------|----------|--------------------------------------------------------------------------|
| bun                | 38.5               | 38.5    |                                                                               | 79.7       | 0.38     |                                                                          |
| tinystore-embedded | 57.9               | 57.9    |                                                                               | 97.4       | 129      |                                                                          |
| tinystore-sidecar  | 64.0               | 47.6    | tinystore 16.5                                                                | 102.1      | 136      |                                                                          |
| tinystore-serve    | 62.0               | 45.6    | tinystore 16.7                                                                | 102.1      | 115      | tinystore 0.02                                                           |
| services           | 445.1              | 43.0    | loki 132.8 + postgres 125.8 + minio 109.1 + victoria-metrics 26.0 + redis 8.8 | 544.6      | 28       | postgres 0.07, redis 0.01, minio 0.38, victoria-metrics 0.06, loki 15.37 |
| valkey             | 48.6               | 39.3    | valkey 9.4                                                                    | 92.6       | 1.09     | valkey 0.06                                                              |
| prometheus         | 137.1              | 39.3    | prometheus 97.8                                                               | 182.4      | 1.48     | prometheus 0.19                                                          |

