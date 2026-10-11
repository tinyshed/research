# Records: the Rust port against the Go engine — 11 October 2026

TinyStore's records engine is being written again in Rust, on its `rust`
branch, with the Go engine at `e81a050` as the reference: its format worked,
so it is ported as measured and changed only by a measurement. This round
puts the two side by side on the same inputs, four ways: what a block takes of
a production corpus by column; what each of the Go engine's codings is worth on
it, turned off one at a time; what the two zstd encoders make of the same
bytes; and the three reads [the round of 9 October](records-optimization-2026-10-09.md)
found the Go engine still won.

| Question | Answer | What follows |
|---|---|---|
| Does the port keep events as densely as the Go engine? | Yes, without FSE: 17.50 B a record against 17.64 on 1,713,543 records, 178,796 of them with fields; 17.30 against 17.51 on their 1,534,747 lines of text. | The block format stands; FSE is left for the entropy coder metrics needs. |
| What is each of the Go engine's codings worth on this corpus? | Turned off one at a time: FSE 0.154 B a record, quotes taken off and values typed 0.264 (uuids 0.163 of it), integers that count time 0.101, dictionaries of integers 0.021, radix words 0.001, hex 0.001, integers with values apart nothing. | All but radix words and FSE are ported. |
| Why is the port's text smaller under the same format? | The encoder: on the same 1,535 blobs of lines, libzstd 1.5.7 at level 7 makes 10.44 B a line where klauspost/compress 1.20.1's `SpeedBetterCompression` makes 13.06. | The share of it inside a block, 0.2 B a record, is a hypothesis: the times cut out first leave less for either to find. |
| Does the port still lose the three reads? | No, on this fixture and this machine: an exact filter 14.11 ms against 18.54, a single trace 0.17 against 0.45, an absent context value 1.94 against 3.00. | Measured on Windows: owed in the Linux container before the numbers sit beside the others. |

## Environment and reproduction

AMD Ryzen 7 7700, 31 GB, **Windows 11 Pro on the host, not the Linux
container** the heavier rounds use; Go 1.27.1 windows/amd64 with
klauspost/compress v1.20.1; Rust 1.99.0, rusqlite 0.40.2 with its bundled
SQLite, zstd-sys 2.1.1 (libzstd 1.5.7). Sizes are counted bytes and do not
depend on the machine; the timings do, and are a comparison of the two engines
run one after the other on it, not a figure for elsewhere.

The Go engine is TinyStore at `e81a050`, with the scratch tests of
[records-port-bench/go](../records-port-bench/README.md) copied into its
`records/`: they reach the package's own encoder, so they are files to drop
in, not a module of their own. The Rust engine is TinyStore's `rust` branch:
the sizes at `062e01d`, which `e3ae052` leaves unchanged, and the reads at
`e3ae052`. The submodule in `tinystore/source` stays at `e307c48`: the
measured commits are on the `rust` branch, which holds no Go for the modules
here to build against, and the report waits for no merge to `main`, which
`rust` replaces when its phase 4 ends — the commits it names are on the
branch, pushed, and never rewritten.

```sh
# the Go side, at e81a050 with go/*.go in <go-repo>/records
TINYSTORE_SPIKE=1 TINYSTORE_CORPUS=<corpus> TINYSTORE_DUMP=<lines> go test ./records -run 'TestCompareDump$' -count=1 -v
TINYSTORE_SPIKE=1 TINYSTORE_CORPUS=<corpus> TINYSTORE_DUMP=<records> go test ./records -run 'TestCompareFields$' -count=1 -v
TINYSTORE_SPIKE=1 TINYSTORE_CORPUS=<corpus> go test ./records -run 'TestCompareZstd$' -count=1 -v
TINYSTORE_SPIKE=1 TINYSTORE_READS=<requests> go test ./records -run 'TestCompareReads$' -count=1 -v
python records-port-bench/ablate.py <go-repo> <corpus>

# the Rust side, in <repo> on its rust branch
TINYSTORE_DUMP=<lines> cargo test --release -p tinystore --lib a_corpus_by_its_columns -- --ignored --nocapture
TINYSTORE_DUMP=<records> cargo test --release -p tinystore --lib a_corpus_by_its_columns -- --ignored --nocapture
TINYSTORE_READS=<requests> cargo test --release -p tinystore --lib three_reads_of_a_packed_corpus -- --ignored --nocapture
```

The libzstd side of the encoder comparison is
[rust/zz_zstd_levels.rs](../records-port-bench/rust/zz_zstd_levels.rs),
appended to the block tests for its run. Every output is in
[data/records-port-2026-10-11](data/records-port-2026-10-11/), one run each.

## The corpora

**Production logs**, private and never published: the docker json-file logs
of two hosts' 75 containers, 507 MB, fetched on 11 October as
`bench/fetch-docker-logs.sh` leaves them. The Go engine's line writer makes
1,713,543 records of them: 1,534,747 lines of text, 215.3 B each, 112,448 JSON
lines and 66,348 logfmt lines kept as fields. `TestCompareDump` writes the text
lines (`<lines>`, sha-256 `f9fad36ad02ebecc…` of its files in order) and
`TestCompareFields` every record with its fields (`<records>`, `397201457f7577e1…`),
both a file a container in arrival order. Each engine cuts a container's
records into segments of 16,384 and those into blocks by its own bounds; the
port weighs an event more than the Go engine's input size and so makes 2,525
blocks of the records where the Go engine makes 2,323, which, measured at
`f157fbf` with the Go engine's weight instead, cost it 0.018 B a record.

**Requests**, generated: 65,536 events of one stream twenty milliseconds apart,
each a request id, one of sixteen routes, a status and a duration as fields, a
host in its context, and a trace one in sixty-four (`readsFixture` in
`zz_reads_test.go`; `<requests>`, sha-256 `dbf85d13fb339530…`). Both engines
append it and pack it before they read.

## Density, by column

Bytes a record of the payload: the blocks and the segment rows, not the file,
which this round did not measure.

| Column | Go `e81a050` | Rust `f157fbf` | Rust `062e01d` |
|---|---|---|---|
| times | 4.261 | 4.261 | 4.261 |
| names | 0.000 | 0.008 | 0.001 |
| shapes | 0.016 | 0.034 | 0.021 |
| levels | 0.013 | 0.025 | 0.020 |
| messages | 11.851 | 11.810 | 11.655 |
| fields kept whole | 0.082 | 0.071 | 0.071 |
| fields by column | 1.385 | 1.883 | 1.431 |
| segment rows | 0.009 | 0.005 | 0.005 |
| **all** | **17.639** | **18.134** | **17.500** |

The lines of text alone took the port 21.71 B a record at `e01e53a`, 18.02
once the times a text spells were cut out (`66c1e7c`), 17.48 once integers had
a rice code and a steady step (`f157fbf`) and 17.30 at `062e01d` (4.257 times,
13.013 messages), against the Go engine's 17.506 (4.258 times, 13.231 text).
The 0.155 B the messages lost at `062e01d` are the integers inside a text
column — its dictionary's ids, its lengths, the parts of the times it spells —
taking dictionaries and the value most of them share: turned off in the port,
those are worth 0.202 B a record, 0.155 of it in the messages, 0.032 in the
fields, 0.009 in the shapes and 0.005 in the levels (17.702 without them). The
Go engine's FSE takes the same there, which is why they are worth it 0.021.

The fields, by the kind of their columns, on the same blocks:

| Kind | Go columns | Go bytes | Go as text | Rust columns | Rust bytes |
|---|---|---|---|---|---|
| quoted uuid | 134 | 1.0286 | 1.2870 | 242 | 1.0294 |
| quoted text | 2,642 | 0.1987 | 0.2028 | 3,935 | 0.2343 |
| quoted stamped | 181 | 0.1052 | 0.2036 | 263 | 0.1088 |
| integers | 1,087 | 0.0152 | 0.0248 | 1,665 | 0.0183 |
| integers counting time | 134 | 0.0081 | 0.2549 | 241 | 0.0113 |
| text | 263 | 0.0262 | 0.0262 | 273 | 0.0250 |
| quoted hex | 142 | 0.0027 | 0.0034 | 249 | 0.0037 |
| quoted integers | 1 | 0.0000 | 0.0000 | 1 | 0.0000 |

The port's columns are more because its blocks are; with the Go engine's
weight, at `f157fbf`, it made the same 4,584.

## The Go engine's codings, one at a time

Each run turns one coding off in `records/ints.go` or `records/values.go`
and measures the records again; `git checkout` gives the file back after it.

| Turned off | All | Fields | Messages | Levels | Shapes |
|---|---|---|---|---|---|
| nothing | 17.639 | 1.385 | 11.851 | 0.013 | 0.016 |
| quotes taken off, and so every quoted type | 17.903 | 1.648 | 11.851 | 0.013 | 0.016 |
| uuids | 17.802 | 1.548 | 11.851 | 0.013 | 0.016 |
| FSE | 17.793 | 1.405 | 11.971 | 0.021 | 0.021 |
| integers that count time | 17.740 | 1.486 | 11.851 | 0.013 | 0.016 |
| dictionaries of integers and a dominant value | 17.660 | 1.405 | 11.851 | 0.013 | 0.016 |
| radix words | 17.640 | 1.385 | 11.851 | 0.013 | 0.016 |
| hex | 17.640 | 1.385 | 11.851 | 0.013 | 0.016 |
| integers with values apart | 17.639 | 1.385 | 11.851 | 0.013 | 0.016 |

On the text lines alone, FSE turned off the same way earlier that day cost
0.134 B a record (17.640 against 17.506).

## The zstd encoders

The same 1,535 blobs, each a container's next 1,024 lines joined, every line
ended by a break; 216.30 B of text a line.

| Encoder | Level | B a line |
|---|---|---|
| klauspost/compress 1.20.1 | `SpeedDefault` | 14.132 |
| klauspost/compress 1.20.1 | `SpeedBetterCompression`, the Go engine's for blocks | 13.058 |
| klauspost/compress 1.20.1 | `SpeedBestCompression` | 11.775 |
| libzstd 1.5.7 | 3 | 11.792 |
| libzstd 1.5.7 | 7, the port's for blocks | 10.444 |
| libzstd 1.5.7 | 11 | 9.480 |
| libzstd 1.5.7 | 19 | 7.924 |

A block compresses its lines after the times they spell are cut out, so these
whole-line figures bound the encoders' difference rather than measure it
inside a block; level 11 for the port's blocks, which packing writes once in
the background, is a question for a round that measures it there with its
packing time.

## The three reads

Medians of fifteen runs of ten, after three, each read paging through every
match with its engine's own iterator; the Go engine first, then the port.

| Read | Found | Go `e81a050` | Rust `e3ae052` |
|---|---|---|---|
| exact filter: a name, a route and a status | 896 | 18.535 ms | 14.106 ms |
| a single trace | 1 | 0.450 ms | 0.171 ms |
| an absent context value | 0 | 2.999 ms | 1.942 ms |

What the port does for them, as the Go engine does: a segment's names and keys
are listed beside it, and a block keeps bloom filters of its trace ids and of
the values of its fields that look like ids, so a read passes a block that
cannot hold what it asks without fetching it; a block fetched has the columns a
read asks about read first, and only the events they leave are built. Before it
built only what it keeps, compared a column as the column keeps its values —
a dictionary's distinct values once, integers as integers — and summed its
CRC-32 sixteen bytes at a time rather than one, the port took 76.24 ms for the
exact filter, 0.86 for the trace and 3.18 for the absent context.

## What landed

On TinyStore's `rust` branch: `f157fbf` integers in rice codes and by a steady
step; `062e01d` typed values and dictionaries of integers; `23a5eb2` the late
head, what each head holds and merges of small segments with their places;
`e3ae052` the reads above.

## What is owed

- The three reads in the Linux container, beside the other rounds' timings.
- A division of the file: `dbstat` of `records.db` holding each corpus, the
  bloom filters and the segment keys among its b-trees.
- FSE, 0.15 B a record here, with the entropy coder metrics brings.
- The zstd level of packed blocks, measured inside them with packing's time.
