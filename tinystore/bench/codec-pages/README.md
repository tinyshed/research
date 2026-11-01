# codec-pages

The harness of the round
[codec-pages-2026-11-01](../../reports/codec-pages-2026-11-01.md): published
floating-point codecs and TinyStore's own, each block stored as one row of
`(id integer primary key, body blob)`, measured as payload and as file.

Every encoder reads the same series file, cuts it into blocks by the same
rule, encodes each block alone, decodes it again and compares every value's
64 bits with the input, and writes the bodies to a bodies file. Everything
after that — the SQLite and PostgreSQL files, the page model, the rank
statistics — reads only bodies files.

## Formats

All integers little-endian.

**Series file** (`.series`): the corpus, one series after another.

```text
"CPS1"                      4 bytes
series count                uint32
per series:
  name length               uint32
  name                      UTF-8 bytes
  value count n             uint64
  values                    n x float64, IEEE-754 bits as stored
```

**Block rule.** For a block length L, each series in file order is cut into
blocks of L consecutive values from its first value; its last block holds the
remainder, n mod L values, when that is not zero. A block never spans two
series.

**Bodies file** (`.bodies`): one record a block, in block order.

```text
"CPB1"                      4 bytes
block count                 uint32
per block:
  series index              uint32
  values in the block       uint32
  body length               uint32
  status                    uint8: 0 every value's bits came back, 1 not
  body                      body length bytes
```

A body holds everything its codec needs to decode the block's values, the
value count included, and nothing else: no timestamps, no checksum unless the
codec's own format carries one.

**Round trip.** A block's status is 0 only if decoding its body gave exactly
n values whose 64 bits equal the input's, `-0.0`, NaN payloads and infinities
included; an exception while decoding is status 1. A codec's result on a
dataset counts only if every block's status is 0.

## Encoders

Each takes `<codec> <series file> <out dir> <L1,L2,...>`, writes
`<out dir>/<codec>.L<L>.bodies` for every block length given, and prints one
JSON line a block length:
`{"codec":…,"block":L,"blocks":…,"values":…,"payload_bytes":…,"failed_blocks":…}`.

| Codec | Name | Where |
|---|---|---|
| TinyStore's adaptive codec | `tinystore` | `tinystore/`, Go, against `../../source` |
| Gorilla | `gorilla` | `java/` |
| Chimp, Chimp128 | `chimp`, `chimp128` | `java/` |
| Elf | `elf` | `java/` |
| ALP | `alp` | `alp/` |
| zstd at levels 3 and 19 | `zstd3`, `zstd19` | `alp/` |

## The TinyStore encoder (`tinystore/`)

TinyStore's codec at the submodule's pinned commit, `codec.EncodeValues`,
which packs 1 to 240 values and leaves the first value and the count to the
row around it. A body here carries them, so that it decodes alone as the
others do: a block is cut into sub-blocks of at most 240 values, each
`uvarint count | first value, 8 bytes | uvarint stream length | stream`, the
last sub-block without its stream length. A block of 240 values or fewer is
one sub-block, 9 or 10 bytes beyond the stream; a longer block is several of
the codec's blocks in one row, which TinyStore itself never writes. Every
value is decoded again with `codec.DecodeValues` and compared bit for bit.

```sh
docker run --rm -v <repo>:/src -v codec-pages-go:/go -e GOCACHE=/go/build-cache \
  -e GOMODCACHE=/go/mod-cache -e GOTOOLCHAIN=local -e GOWORK=off -e CGO_ENABLED=0 \
  -e HTTPS_PROXY=<proxy> -w /src/tinystore/bench/codec-pages golang:1.27 \
  go build -o /go/bin/cp-tinystore ./tinystore
```

## The Java encoders (`java/`)

Gorilla, Chimp, Chimp128 and Elf are their authors' Java classes, compiled
unmodified from source in Linux containers; none of their code is in this
repository and no jar is downloaded. `fetch-sources.sh` (in `golang:1.27`,
for git, gcc and make) clones each repository at a pinned commit into the
Docker volume `codec-pages-java-src`; `compile.sh` (in
`eclipse-temurin:21-jdk`) compiles the classes below with `javac` into
`codec-pages-java-build`.

```sh
cd <repo>/tinystore/bench/codec-pages
java/codec-pages.sh build      # CODEC_PAGES_PROXY=<http proxy> to clone through one
java/codec-pages.sh <codec> <corpus>/<file>.series <out dir> 240,1000,1024
```

| Pinned | |
|---|---|
| panagiotisl/chimp | `320d397157c7e0696b3c64dc1711fc17a3add3da` (main) |
| Spatio-Temporal-Lab/elf | `386d69348e6c30475761d3519ee0bfe10ed473d3` (branch `vldb2023-release`, the VLDB 2023 paper's; `dev` is a later algorithm) |
| vigna/dsiutils | `838c5070b74382a652649f24042c19e4f492dab5` (sets version 2.7.0, which both poms name) |
| vigna/fastutil | `dc909bb15d7cb0b9d40d7382bf7bb0a5c6c517e0` (tag 8.5.8) |
| JDK | `eclipse-temurin:21-jdk@sha256:ebfbb18d11ff163ee83b21258acf12248af48d22a0fdfdc68fa1a1516c05976a` (Temurin 21.0.13+9) |
| git, gcc, make | `golang:1.27@sha256:2920bb124f1a2e12a1d39f0cab756ae0068441ee1da7655dec7f5e374944095b` |

Both repositories' bit streams (`gr.aueb.delorean.chimp.InputBitStream`,
`OutputBitStream`) are copies of dsiutils' that import dsiutils and fastutil
and load dsiutils' code tables when initialised, so those two are built from
source as well (fastutil's type-specific classes generated by its own
`make sources`), and javac compiles only the classes the bit streams reach.
Elf carries its own copy of the bit streams, so each repository compiles
into its own class tree, chosen by codec.

| Codec | Classes | Why these |
|---|---|---|
| `gorilla` | `fi.iki.yak.ts.compression.gorilla.Compressor`, `Decompressor` over `ByteBufferBitOutput`, `ByteBufferBitInput` (chimp) | the value-only Gorilla the Chimp paper measured (`TestDoublePrecision.testCorilla`), and the Gorilla paper's encoding: `0` for an equal value, `10` and the meaningful bits inside the previous window, `11`, 5 bits of leading zeros, 6 bits of length and the meaningful bits; not `ValueCompressor` (6-bit leading zeros, length − 1) nor `GorillaCompressor` (timestamps) |
| `chimp` | `gr.aueb.delorean.chimp.Chimp`, `ChimpDecompressor` | the test's Chimp |
| `chimp128` | `ChimpN(128)`, `ChimpNDecompressor(body, 128)` | the test's Chimp128 (`ChimpNNoIndex` drops its index) |
| `elf` | `org.urbcomp.startdb.compress.elf.compressor.ElfCompressor`, `decompressor.ElfDecompressor` | the complete Elf, as its `TestCompressor` runs it |

**A body is the codec's own bytes for the block.** All four end the stream
with a NaN, `0x7ff8000000000000`, encoded like a value, so none needs the
value count added. Gorilla: the first value's 64 bits, each value's control
bits and fields, the marker, a zero bit and `flush()`'s last byte (a whole
zero byte if the stream was aligned). Chimp, Chimp128: the same with Chimp's
flags (Chimp128's with 7-bit references to one of the 128 previous values),
a zero bit, padding to a byte. Elf: each value's erasure flag (`0`, or `1`
and 4 bits of β*) and the ElfXOR encoding of the erased value, the first as 7
bits of trailing zeros and the rest of its bits; then a `0` flag, the marker,
a zero bit, padding.

**One intervention.** Chimp, ChimpN and ElfXORCompressor write into fixed
arrays of 8,000 and 12,000 bytes, sized for blocks of 1,000 values, and past
them the bit stream drops bits silently. Before the first value the harness
replaces that private field, by reflection, with a bit stream over 16 bytes
a value; the bytes written are unchanged.

**Blocks run in a child JVM**, so one that never returns can be killed:
after 3 s + 1 ms × L it is status 1 with an empty body, and the next block
starts in a new child. An encoder that throws also leaves an empty body; a
decoder that throws or returns other bits leaves the encoder's. Each failure
is one line on stderr.

**Known failures of the authors' code**, found on the harness's test file:

- `0x7ff8000000000000` ends every codec's stream, so a block holding it
  decodes to the values before it. Gorilla, Chimp and Chimp128 keep every
  other NaN exactly; Elf turns every NaN into that one, so any NaN fails.
- Elf never returns on a value whose digit-count search cannot end
  (`AbstractElfCompressor.getSignificantCount` multiplies by 10^i until the
  product equals its `long`): |v| ≥ about 9.22e17, and small values whose
  search passes 10^308 (subnormals, the smallest normal, many-digit values
  below about 1e-292). It stops only when i overflows and throws.

## The ALP and zstd encoder (`alp/`)

`alp/encode.cpp` encodes `alp`, `zstd3` and `zstd19`. `alp/build.sh` builds it
in `golang:1.27` (Debian trixie, g++ 14.2): it fetches cwida/ALP at
`31ca0ed11c93c99d3f5b5c30e01a3e1c3832d3ce`, used unmodified, and zstd 1.5.7's
source release, checked against the release's SHA-256
`eb33e51f49a15e023950cd7825ca74a4a2b43db8354825ac24fc1b7ee09e6fa3`, and
compiles both with `-O2 -fno-strict-aliasing -fwrapv` and no `-march`, so ALP
runs its scalar paths. `alp/run.sh` wraps the container: sources in the volume
`codec-pages-alp-src`, the build in `codec-pages-alp-build`, the repository
read-only.

```sh
sh <repo>/tinystore/bench/codec-pages/alp/run.sh build
sh <repo>/tinystore/bench/codec-pages/alp/run.sh alp <corpus>/x.series <out> 120,1000,1024
sh <repo>/tinystore/bench/codec-pages/alp/run.sh --verify alp <corpus>/x.series <out>/alp.L1024.bodies
```

`--verify` decodes a bodies file again in a fresh process. With
`CODEC_PAGES_TRACE=1`, `alp` writes one JSON line a vector to stderr: its
scheme, exponent, factor, bit width and exceptions.

**zstd.** A body is one frame from `ZSTD_compress` at level 3 or 19 over the
block's little-endian float64 bytes: default parameters, content size in the
header, no checksum. It decodes with `ZSTD_decompress`, which takes the value
count from that content size.

**alp.** A block is one ALP row group, or several of 102,400 values each when
the block is longer, cut into vectors of 1,024 values; the last vector holds
m < 1,024 when the block is shorter. `encoder::init` samples the block as ALP
samples a row group (every 12th vector, so vector 0 alone below 13 vectors; a
stride of ceil(m/32) values within it) and picks ALP or ALP_RD and up to five
(exponent, factor) pairs; `rd_encoder::init` builds ALP_RD's dictionary from
the same sample. Each vector chooses its pair, is encoded, framed by
`analyze_ffor` and packed by FastLanes `ffor`. Decoding is `unffor`,
`decoder::decode` and `patch_exceptions`, or two `unffor` and
`rd_encoder::decode`; the fused `falp` runs beside it and is compared. A body,
all integers little-endian:

```text
value count n                  uint32
per row group:
  scheme                       uint8: 2 ALP, 1 ALP_RD (ALP's Scheme)
  ALP_RD only:
    right bit width rbw        uint8
    left bit width lbw         uint8
    dictionary size d          uint8, at most 8
    dictionary                 d x uint16
  per vector of m values:
    ALP:
      exponent index           uint8
      factor index             uint8
      exception count c        uint16
      frame of reference       int64
      bit width bw             uint8
      packed integers          ceil(m x bw / 8) bytes
      exceptions               c x float64, bits as given
      exception positions      c x uint16
    ALP_RD:
      exception count c        uint16
      packed left parts        ceil(m x lbw / 8) bytes
      packed right parts       ceil(m x rbw / 8) bytes
      exceptions               c x uint16, the left parts
      exception positions      c x uint16
```

Fields and order are those of ALP's example compressor (`include/alp/
compressor.hpp`, removed in a19695f). The widths are those ALP's
compression-ratio benchmark charges: 8 bits each for bit width, exponent and
factor, 64 for the frame of reference, 64 + 16 for an exception and its
position, 16 + 16 for an ALP_RD exception, 16 for a dictionary entry. The
benchmark does not count the value count or the exception count.

A full vector's packed bytes are FastLanes' buffer, 128 x bw bytes. A shorter
vector keeps only its m values' bits. FastLanes puts value i in lane i mod 16
(mod 64 for 16-bit left parts), row i / 16 of it; the vector stores the
buffer's words in order, each cut to the low bits holding rows below m, and
decoding puts them back into a zeroed buffer before `unffor`.

ALP's primitives take 1,024 values; its benchmarks drop a column's incomplete
last vector. Here an ALP vector of m values runs second-level sampling over
the m values only, is encoded once with 0.0 padding, padded again with its
first real non-exception value, and encoded again, so the padding adds no
exception and does not widen the frame. An ALP_RD vector is padded with its
first value, and exceptions in the padding are dropped.

Besides the common fields, the `alp` JSON line carries `alp_estimate_bits`
(ALP's own accounting summed per vector, m x bw + 80c + 88 for ALP and
m x (lbw + rbw) + 32c + m x 128 / 102,400 for ALP_RD; exactly the
benchmark's for full vectors), `full_vector_payload_bytes` (the payload if
every short vector stored FastLanes' whole buffer), `alp_rowgroups`,
`alp_rd_rowgroups`, `exceptions`, `falp_mismatch_blocks` and `library`.

## The round

| File | What it does |
|---|---|
| `cpfiles.py` | reads and writes series and bodies files, and the block rule |
| `make_test_series.py` | the encoders' test file: five real columns, integers, a decimal walk, random bits, edge values |
| `fetch_alp.py`, `prepare.py` | ALP's dataset prefixes by HTTP ranges; every corpus into series files |
| `run.sh`, `store.py` | one dataset through every encoder and into SQLite files, divided and predicted |
| `pg_prepare.py`, `pg_sql.py`, `pg_load.sh` | the kept bodies into PostgreSQL 18, every size and every page's header out |
| `pg_model.py`, `pg_analyze.py` | the page model extended to PostgreSQL's heap and TOAST; its orders against SQLite's and a flat file's |
| `analyze.py` | Kendall's τ-b, reversals, model checks and block-length optima |
| `h15.py`, `h15_hist.py` | TinyStore's groups layout predicted from a file's rows, and from metrics-layout's histograms |
| `corpus-sha256.txt` | the prepared corpus's hashes, relative to `<corpus>` |

From Git Bash (`MSYS_NO_PATHCONV=1`), the corpora under `<corpus>`, which is
`tinystore/bench/corpus/codec-pages` here and ignored:

```sh
cd <repo>/tinystore/bench/codec-pages
HTTPS_PROXY=<proxy> python -I fetch_alp.py <corpus>/alp 1048576   # ALP's 30 datasets, 2^20 values at most
# NAB's 24 files as ../nab-sha256.txt pins them, into <corpus>/nab/<category>/
# TSBS long, ten hosts for three days, converted by ../tsbs into <corpus>/tsbs-long.jsonl
python -I prepare.py <corpus>                                      # <corpus>/series/<dataset>.series
sha256sum -c corpus-sha256.txt                                     # from <corpus>
sh run.sh <corpus> <dataset>              # every codec, 24 block lengths, 1/4/8/16 KiB pages
sh pg_load.sh <dataset> ...               # PostgreSQL 18, EXTERNAL and EXTENDED
docker run --rm -v codec-pages-work:/work -v <repo>:/src:ro golang:1.27 sh -c '
  python3 -I /src/tinystore/bench/codec-pages/pg_model.py /work/pg/out /work/results/pg.jsonl <dataset> ... &&
  python3 -I /src/tinystore/bench/codec-pages/analyze.py /work/results /work/analysis &&
  python3 -I /src/tinystore/bench/codec-pages/pg_analyze.py /work/results/pg.jsonl /work/results /work/analysis'
```

`run.sh` keeps the bodies of 240, 1,000 and 1,024 values for the PostgreSQL
pass and deletes the rest once stored. `store.py` builds each file as
`tinystore/bench/page-model/page_model.py` builds its own and predicts it
with that file's model, unchanged, from the stored rows' `length(body)`.

H1.5 reads TinyStore's TSBS metrics file as the metrics-layout round's Go
builder writes it at `e307c48`, and that round's raw JSON:

```sh
docker run --rm -v <repo>:/src:ro -v codec-pages-go:/go -v codec-pages-work:/work \
  -e GOCACHE=/go/build-cache -e GOMODCACHE=/go/mod-cache -e GOTOOLCHAIN=local -e GOWORK=off \
  -e CGO_ENABLED=0 -e GOFLAGS=-mod=readonly -e HTTPS_PROXY=<proxy> \
  -w /src/tinystore/metrics-layout-bench/go golang:1.27 sh -c '
  go build -trimpath -o /go/bin/layout-go-input . &&
  GOMAXPROCS=1 /go/bin/layout-go-input --out /work/h15/tsbs --dataset tsbs --corpus <tsbs wide series.jsonl>'
docker run --rm -v codec-pages-work:/work -v <repo>:/src:ro golang:1.27 sh -c '
  python3 -I /src/tinystore/bench/codec-pages/h15.py --model /src/tinystore/bench/page-model/page_model.py \
    --db /work/h15/tsbs/metrics.db --work /tmp/h15 --out /work/h15/tsbs-h15.json &&
  python3 -I /src/tinystore/bench/codec-pages/h15_hist.py --model /src/tinystore/bench/page-model/page_model.py \
    --data /src/tinystore/reports/data/metrics-layout-2026-10-09 --out /work/h15/h15-hist.json'
```
