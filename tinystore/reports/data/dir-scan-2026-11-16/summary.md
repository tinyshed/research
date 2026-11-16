## windows (SQLite 3.53.2, rusqlite 0.40.2)

### warm: seconds, median [least, most]

| step | lib10k | lib100k | flat100k | lib1m |
|---|---|---|---|---|
| 1. bare walk | 0.039 [0.028, 0.137] n=7 | 0.382 [0.319, 0.424] n=7 | 0.079 [0.063, 0.109] n=7 | 2.360 [2.099, 2.967] n=7 |
| 2. first scan | 0.454 [0.198, 0.872] n=5 | 1.398 [1.369, 1.650] n=5 | 1.088 [1.004, 1.157] n=5 | 13.071 [10.963, 14.101] n=5 |
| 3. scan, nothing changed | 0.077 [0.062, 0.101] n=7 | 0.730 [0.439, 0.927] n=7 | 0.331 [0.236, 0.666] n=7 | 4.396 [3.841, 8.262] n=7 |
| 3m. same, sorted in memory | 0.044 [0.039, 0.055] n=7 | 0.382 [0.299, 0.552] n=7 | 0.148 [0.098, 0.284] n=7 | 2.843 [2.693, 9.451] n=7 |
| 4. scan, 1 % changed | 0.092 [0.083, 0.228] n=5 | 1.167 [0.827, 1.601] n=5 | 0.652 [0.584, 0.937] n=5 | 4.798 [4.709, 14.659] n=5 |
| CPU s, 1. bare walk | 0.031 [0.016, 0.047] n=7 | 0.281 [0.234, 0.328] n=7 | 0.062 [0.047, 0.078] n=7 | 2.250 [2.062, 2.484] n=7 |
| CPU s, 2. first scan | 0.047 [0.031, 0.125] n=5 | 0.938 [0.891, 1.016] n=5 | 0.578 [0.500, 0.594] n=5 | 6.828 [6.141, 7.469] n=5 |
| CPU s, 3. scan, nothing changed | 0.062 [0.047, 0.078] n=7 | 0.531 [0.406, 0.672] n=7 | 0.266 [0.219, 0.359] n=7 | 4.359 [3.797, 6.422] n=7 |
| CPU s, 3m. same, sorted in memory | 0.047 [0.031, 0.047] n=7 | 0.359 [0.297, 0.391] n=7 | 0.109 [0.094, 0.141] n=7 | 2.734 [2.594, 4.172] n=7 |
| CPU s, 4. scan, 1 % changed | 0.062 [0.062, 0.094] n=5 | 0.734 [0.703, 0.781] n=5 | 0.328 [0.266, 0.344] n=5 | 4.219 [4.203, 6.750] n=5 |
| peak RSS MiB, 1. bare walk | 4.4 | 4.4 | 4.3 | 4.5 |
| peak RSS MiB, 2. first scan | 7.2 | 7.4 | 9.2 | 8.0 |
| peak RSS MiB, 3. scan, nothing changed | 7.1 | 7.3 | 9.4 | 7.5 |
| peak RSS MiB, 3m. same, sorted in memory | 6.8 | 6.9 | 15.1 | 7.1 |
| peak RSS MiB, 4. scan, 1 % changed | 7.1 | 7.5 | 9.5 | 7.8 |

- lib10k: changed [95, 112, 104, 105, 105], commits [1, 1, 1, 1, 1]
- lib10k: files 10000, folders 570, first commits 10
- lib100k: changed [977, 1033, 967, 1019, 1048], commits [1, 2, 1, 2, 2]
- lib100k: files 100000, folders 5571, first commits 100
- flat100k: changed [1003, 1028, 967, 1025, 1027], commits [2, 2, 1, 2, 2]
- flat100k: files 100000, folders 2, first commits 100
- lib1m: changed [10005, 9914, 10022, 10166, 9888], commits [11, 10, 11, 11, 10]
- lib1m: files 1000000, folders 55015, first commits 1000

### the list's file

- lib10k: 10000 rows, file 2744320 B = 274.4 B a file, paths 101.0 B a row
  - changes: 1 pages, 1 cells, payload 5, unused 4079, 0.4 B a file, payload 5.0 B a cell
  - files: 344 pages, 10000 cells, payload 1209793, unused 161392, 140.9 B a file, payload 121.0 B a cell
  - files_by_change: 324 pages, 10000 cells, payload 1080062, unused 211068, 132.7 B a file, payload 108.0 B a cell
  - sqlite_schema: 1 pages, 3 cells, payload 446, unused 3529, 0.4 B a file, payload 148.7 B a cell
- lib100k: 100000 rows, file 27344896 B = 273.4 B a file, paths 100.7 B a row
  - changes: 1 pages, 1 cells, payload 6, unused 4078, 0.0 B a file, payload 6.0 B a cell
  - files: 3437 pages, 100000 cells, payload 12137104, unused 1562275, 140.8 B a file, payload 121.4 B a cell
  - files_by_change: 3237 pages, 100000 cells, payload 10840273, unused 2058599, 132.6 B a file, payload 108.4 B a cell
  - sqlite_schema: 1 pages, 3 cells, payload 446, unused 3529, 0.0 B a file, payload 148.7 B a cell
- flat100k: 100000 rows, file 11583488 B = 115.8 B a file, paths 33.5 B a row
  - changes: 1 pages, 1 cells, payload 6, unused 4078, 0.0 B a file, payload 6.0 B a cell
  - files: 1562 pages, 100000 cells, payload 5314003, unused 765209, 64.0 B a file, payload 53.1 B a cell
  - files_by_change: 1264 pages, 100000 cells, payload 4017243, unused 844937, 51.8 B a file, payload 40.2 B a cell
  - sqlite_schema: 1 pages, 3 cells, payload 446, unused 3529, 0.0 B a file, payload 148.7 B a cell
- lib1m: 1000000 rows, file 274821120 B = 274.8 B a file, paths 101.1 B a row
  - changes: 1 pages, 1 cells, payload 6, unused 4078, 0.0 B a file, payload 6.0 B a cell
  - files: 34559 pages, 1000000 cells, payload 122071341, unused 15678040, 141.6 B a file, payload 122.1 B a cell
  - files_by_change: 32534 pages, 1000000 cells, payload 109103205, unused 20550385, 133.3 B a file, payload 109.1 B a cell
  - sqlite_schema: 1 pages, 3 cells, payload 446, unused 3529, 0.0 B a file, payload 148.7 B a cell

- gen lib:10000: 10000 files, 569 folders, 20594637 B, 15.002 s
- gen lib:100000: 100000 files, 5570 folders, 204449247 B, 21.723 s
- gen flat:100000: 100000 files, 1 folders, 204030960 B, 19.639 s
- gen lib:1000000: 1000000 files, 55014 folders, 2048949761 B, 246.155 s

## linux (SQLite 3.53.2, rusqlite 0.40.2)

### warm: seconds, median [least, most]

| step | lib10k | lib100k | flat100k | lib1m |
|---|---|---|---|---|
| 1. bare walk | 0.020 [0.019, 0.022] n=7 | 0.216 [0.197, 0.409] n=7 | 0.191 [0.166, 0.202] n=7 | 2.039 [2.030, 2.169] n=7 |
| 2. first scan | 0.096 [0.093, 0.104] n=5 | 3.773 [2.440, 4.809] n=5 | 0.769 [0.743, 0.839] n=5 | 7.975 [7.095, 8.453] n=5 |
| 3. scan, nothing changed | 0.030 [0.030, 0.035] n=7 | 0.457 [0.352, 1.099] n=7 | 0.415 [0.395, 0.435] n=7 | 3.440 [3.273, 4.545] n=7 |
| 3m. same, sorted in memory | 0.023 [0.023, 0.025] n=7 | 0.275 [0.253, 0.546] n=7 | 0.212 [0.202, 0.223] n=7 | 2.536 [2.388, 2.701] n=7 |
| 4. scan, 1 % changed | 0.042 [0.040, 0.043] n=5 | 0.399 [0.385, 0.415] n=5 | 0.454 [0.424, 0.483] n=5 | 4.594 [3.793, 9.079] n=5 |
| CPU s, 1. bare walk | 0.020 [0.010, 0.020] n=7 | 0.200 [0.190, 0.400] n=7 | 0.180 [0.160, 0.190] n=7 | 2.030 [2.020, 2.160] n=7 |
| CPU s, 2. first scan | 0.040 [0.030, 0.040] n=5 | 0.910 [0.880, 1.480] n=5 | 0.510 [0.500, 0.590] n=5 | 5.640 [4.840, 5.650] n=5 |
| CPU s, 3. scan, nothing changed | 0.020 [0.020, 0.030] n=7 | 0.450 [0.350, 1.090] n=7 | 0.410 [0.390, 0.430] n=7 | 3.430 [3.260, 4.540] n=7 |
| CPU s, 3m. same, sorted in memory | 0.020 [0.010, 0.020] n=7 | 0.270 [0.240, 0.530] n=7 | 0.200 [0.200, 0.220] n=7 | 2.520 [2.380, 2.690] n=7 |
| CPU s, 4. scan, 1 % changed | 0.030 [0.020, 0.030] n=5 | 0.360 [0.340, 0.370] n=5 | 0.430 [0.390, 0.450] n=5 | 4.340 [3.580, 7.570] n=5 |
| peak RSS MiB, 1. bare walk | 2.7 | 2.7 | 2.6 | 2.7 |
| peak RSS MiB, 2. first scan | 5.9 | 6.0 | 7.9 | 6.1 |
| peak RSS MiB, 3. scan, nothing changed | 5.8 | 6.0 | 7.8 | 6.2 |
| peak RSS MiB, 3m. same, sorted in memory | 5.6 | 5.8 | 13.8 | 6.0 |
| peak RSS MiB, 4. scan, 1 % changed | 5.8 | 6.1 | 7.9 | 6.4 |

- lib10k: changed [125, 108, 96, 107, 105], commits [1, 1, 1, 1, 1]
- lib10k: files 10000, folders 570, first commits 10
- lib100k: changed [1047, 1014, 1002, 981, 1032], commits [2, 2, 2, 1, 2]
- lib100k: files 100000, folders 5571, first commits 100
- flat100k: changed [992, 1026, 1018, 1032, 1001], commits [1, 2, 2, 2, 2]
- flat100k: files 100000, folders 2, first commits 100
- lib1m: changed [10042, 10000, 9981, 9962, 9924], commits [11, 10, 10, 10, 10]
- lib1m: files 1000000, folders 55015, first commits 1000

### cold: seconds, median [least, most]

| step | lib10k | lib100k | flat100k | lib1m |
|---|---|---|---|---|
| 1. bare walk | 0.151 [0.053, 0.166] n=7 | 1.565 [1.489, 1.745] n=7 | 0.804 [0.496, 0.857] n=7 | 22.067 [16.541, 34.608] n=7 |
| 2. first scan | 0.239 [0.223, 0.271] n=5 | 2.175 [2.146, 2.411] n=5 | 1.436 [1.378, 1.616] n=5 | 26.841 [23.390, 50.348] n=5 |
| 3. scan, nothing changed | 0.173 [0.154, 0.177] n=7 | 1.883 [1.701, 2.052] n=7 | 1.012 [1.002, 1.037] n=7 | 23.664 [16.924, 29.623] n=7 |
| 3m. same, sorted in memory | 0.151 [0.139, 0.186] n=7 | 1.745 [1.577, 1.875] n=7 | 0.819 [0.783, 0.866] n=7 | 19.842 [15.297, 25.851] n=7 |
| 4. scan, 1 % changed | 0.172 [0.163, 0.195] n=5 | 2.166 [2.126, 4.175] n=5 | 1.263 [1.170, 1.559] n=5 | 19.972 [19.073, 89.618] n=5 |
| CPU s, 1. bare walk | 0.060 [0.050, 0.080] n=7 | 0.710 [0.660, 0.800] n=7 | 0.550 [0.490, 0.590] n=7 | 12.700 [8.650, 21.070] n=7 |
| CPU s, 2. first scan | 0.090 [0.090, 0.100] n=5 | 1.110 [1.090, 1.250] n=5 | 0.950 [0.880, 1.060] n=5 | 13.770 [12.200, 21.770] n=5 |
| CPU s, 3. scan, nothing changed | 0.080 [0.070, 0.090] n=7 | 0.970 [0.840, 1.190] n=7 | 0.770 [0.750, 0.780] n=7 | 12.790 [9.550, 20.970] n=7 |
| CPU s, 3m. same, sorted in memory | 0.070 [0.060, 0.080] n=7 | 0.830 [0.720, 1.020] n=7 | 0.560 [0.550, 0.610] n=7 | 11.210 [8.060, 17.080] n=7 |
| CPU s, 4. scan, 1 % changed | 0.080 [0.070, 0.080] n=5 | 1.140 [1.130, 1.850] n=5 | 0.970 [0.890, 1.180] n=5 | 11.170 [10.850, 32.060] n=5 |
| peak RSS MiB, 1. bare walk | 2.6 | 2.7 | 2.7 | 2.7 |
| peak RSS MiB, 2. first scan | 5.8 | 6.1 | 7.9 | 6.2 |
| peak RSS MiB, 3. scan, nothing changed | 5.8 | 6.0 | 7.8 | 6.2 |
| peak RSS MiB, 3m. same, sorted in memory | 5.5 | 5.7 | 13.8 | 6.0 |
| peak RSS MiB, 4. scan, 1 % changed | 5.8 | 6.1 | 7.8 | 6.3 |

- lib10k: changed [92, 94, 105, 77, 119], commits [1, 1, 1, 1, 1]
- lib10k: files 10000, folders 570, first commits 10
- lib100k: changed [949, 961, 1000, 972, 1005], commits [1, 1, 1, 1, 2]
- lib100k: files 100000, folders 5571, first commits 100
- flat100k: changed [966, 944, 1037, 1002, 1000], commits [1, 1, 2, 2, 1]
- flat100k: files 100000, folders 2, first commits 100
- lib1m: changed [9994, 9869, 10018, 10078, 10073], commits [10, 10, 11, 11, 11]
- lib1m: files 1000000, folders 55015, first commits 1000

### the list's file

- lib10k: 10000 rows, file 2744320 B = 274.4 B a file, paths 101.0 B a row
  - changes: 1 pages, 1 cells, payload 5, unused 4079, 0.4 B a file, payload 5.0 B a cell
  - files: 344 pages, 10000 cells, payload 1209792, unused 161393, 140.9 B a file, payload 121.0 B a cell
  - files_by_change: 324 pages, 10000 cells, payload 1080061, unused 211069, 132.7 B a file, payload 108.0 B a cell
  - sqlite_schema: 1 pages, 3 cells, payload 446, unused 3529, 0.4 B a file, payload 148.7 B a cell
- lib100k: 100000 rows, file 27357184 B = 273.6 B a file, paths 100.7 B a row
  - changes: 1 pages, 1 cells, payload 6, unused 4078, 0.0 B a file, payload 6.0 B a cell
  - files: 3437 pages, 100000 cells, payload 12137084, unused 1562286, 140.8 B a file, payload 121.4 B a cell
  - files_by_change: 3240 pages, 100000 cells, payload 10840253, unused 2070868, 132.7 B a file, payload 108.4 B a cell
  - sqlite_schema: 1 pages, 3 cells, payload 446, unused 3529, 0.0 B a file, payload 148.7 B a cell
- flat100k: 100000 rows, file 11583488 B = 115.8 B a file, paths 33.5 B a row
  - changes: 1 pages, 1 cells, payload 6, unused 4078, 0.0 B a file, payload 6.0 B a cell
  - files: 1562 pages, 100000 cells, payload 5314034, unused 765178, 64.0 B a file, payload 53.1 B a cell
  - files_by_change: 1264 pages, 100000 cells, payload 4017274, unused 844906, 51.8 B a file, payload 40.2 B a cell
  - sqlite_schema: 1 pages, 3 cells, payload 446, unused 3529, 0.0 B a file, payload 148.7 B a cell
- lib1m: 1000000 rows, file 274817024 B = 274.8 B a file, paths 101.1 B a row
  - changes: 1 pages, 1 cells, payload 6, unused 4078, 0.0 B a file, payload 6.0 B a cell
  - files: 34559 pages, 1000000 cells, payload 122071430, unused 15677946, 141.6 B a file, payload 122.1 B a cell
  - files_by_change: 32533 pages, 1000000 cells, payload 109103294, unused 20546206, 133.3 B a file, payload 109.1 B a cell
  - sqlite_schema: 1 pages, 3 cells, payload 446, unused 3529, 0.0 B a file, payload 148.7 B a cell

- gen lib:10000: 10000 files, 569 folders, 20594637 B, 0.292 s
- gen lib:100000: 100000 files, 5570 folders, 204449247 B, 3.214 s
- gen flat:100000: 100000 files, 1 folders, 204030960 B, 2.229 s
- gen lib:1000000: 1000000 files, 55014 folders, 2048949761 B, 36.534 s

