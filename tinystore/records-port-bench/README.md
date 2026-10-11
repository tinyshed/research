# Records: the Rust port against the Go engine

What [the round of 11 October](../reports/records-port-2026-10-11.md) ran on
the Go side, and the one Rust test it ran beside TinyStore's own.

| Path | What it is |
|---|---|
| `go/zz_compare_test.go` | `TestCompareDump`: the text lines a writer of `Lines` makes of the corpus, by column, and their dump |
| `go/zz_fields_test.go` | `TestCompareFields`: every record it makes, text, JSON and logfmt, by slot and by the kind of each field's column, and their dump |
| `go/zz_zstd_test.go` | `TestCompareZstd`: klauspost/compress at three levels on the corpus's lines, 1,024 a blob |
| `go/zz_reads_test.go` | `TestCompareReads`: 65,536 generated requests packed, three reads timed, and their dump |
| `ablate.py` | `TestCompareFields` again with each of the Go engine's codings turned off in turn |
| `rust/zz_zstd_levels.rs` | libzstd at four levels on the same blobs, appended to the port's block tests for a run |

The Go files are `package records` tests: they reach the engine's encoder,
which no other package can, so they are copied into `<go-repo>/records` with
TinyStore at `e81a050` checked out there, and are no module of their own. They
read `TINYSTORE_SPIKE=1` and `TINYSTORE_CORPUS`, a corpus as
`bench/fetch-docker-logs.sh` leaves it, and write their dumps where
`TINYSTORE_DUMP` or `TINYSTORE_READS` names.

The Rust side of every other measurement is in TinyStore itself, on its `rust`
branch: `a_corpus_by_its_columns` reads either dump and `three_reads_of_a_packed_corpus`
the requests, both ignored tests run by hand; the report has the commands.
