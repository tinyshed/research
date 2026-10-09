# Public inputs for the metrics storage-layout rounds

The two studies use the same production-Go-created stores and immutable block
exports. `metrics-layout-bench` owns that builder and verifies every source
sample through the actual Go metrics API before the layout studies begin.
Production source is pinned to `e307c48a40126aad0e2873b6bf3aaedef8115483`.

TSBS uses the retained normalized corpus with SHA-256
`e4f502af7b0b2ff2c4dba92057a8f2b95e636882f3cb9900986e013d189572cf`:
5,090,400 samples across 2,020 series. Its prepared corpus is copied to the
Linux volume as `/work/metrics-storage-input/corpus/tsbs-series.jsonl`.

Alibaba is fetched from the original public archive and selected using the
retained `bench/alibaba` and `bench/tsbs` Go programs. The archive hash is
`3e6ee87fd204bb85b9e234c5c75a5096580fdabc8f085b224033080090753a7a`.
The selection keeps machine IDs divisible by 16 and trace seconds below
172800, mapping trace zero to Unix millisecond 1767225600000. It reads
246,934,820 rows and retains 2,486,377 rows, 248 machines, 1,240 series and
12,431,885 samples, without duplicate timestamps. The normalized JSONL hash is
`7ace58aaba7b3c34efc34c41585014521121c76aa991646c30735127a619afdd`.

From the Linux container with this checkout at `/src` and a Linux volume at
`/work`, prepare Alibaba with Go 1.27.1, Python 3, tar and curl:

```sh
python3 /src/tinystore/metrics-storage-inputs/prepare_alibaba.py
```

The script checks the archive, streams the 9 GB CSV into the selector without
retaining that CSV, converts once, and records source/executable/corpus hashes.
It refuses to overwrite an already prepared normalized file. Download and
normalization elapsed time are input preparation, not storage performance.
The compact provenance is retained in
`../reports/data/metrics-storage-inputs-2026-10-09/alibaba-provenance.json`.

Corpus bytes, line protocol, SQLite stores, block exports and generated
executables stay on the Linux volume and are not committed. The reports also
use bounded deterministic regular, irregular, nonsparse and IEEE edge fixtures;
those recipes and their exact hashes are part of the builder and manifests.
