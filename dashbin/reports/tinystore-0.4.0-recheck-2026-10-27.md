# TinyStore 0.4.0 against issues 34 to 37, 27 October 2026

TinyStore 0.4.0 closed the four issues Dashbin filed the same day
([tinyshed/tinystore](https://github.com/tinyshed/tinystore) #34–#37). Each
program was rerun against the published 0.4.0 crate rather than taken from
the closing comment; the ones whose fix is a new call were moved onto it.

| Issue | 0.3.0 | 0.4.0 | What follows for Dashbin |
|---|---|---|---|
| #34 aggregating what is never stored | Metrics' answer only through a store: 24.5–30.6 ms, a file on disk | `series.query().aggregate(op)` in memory: `Exact(1.0)` and `Exact(170.0)`, the same as Metrics, in 85 µs; `ExactSum` gives 1.0 where a plain sum gives 0.9999999999999999 | chapter 5's kernel sums and aggregates with TinyStore's own code; no compensated sum of ours |
| #35 a backup at one moment | each engine its own moment, in the order opened: the window **ahead** of its samples in 20 of 20 with Metrics opened first | ahead in 0 of 60 backups, whichever was opened first, Records included; equal in 55, behind by exactly one in 5 — the moment fell between the sample and its window | a restore holds nothing naming what it lacks; the order the hub opens its engines in no longer matters, and the test on the direction goes |
| #36 files with their rows | a crash between the two writes left a file alone or a row alone | `db.files(..)` inside `db.tx`: committed, both kept; rolled back, neither; the process killed inside the transaction, neither, and no bytes left after reopening | a plugin's package and its row are one write; the order of an install, the sweep and the check at start go |
| #37 old keys | a restore without the key meant clearing the bucket, and the old key found later opened nothing | a new key seals beside values it cannot open; the old key among `older_encryption_keys` opens them; `reseal()` moved one value, after which the old key could go | a hub restored without its key keeps the old secrets until the key is found, and asks only for those it needs now |

## Environment and reproduction

- `tinystore = "=0.4.0"` from crates.io, features `metrics`, `records`,
  `sql`, `kv`, `blobs`, `backup`.
- Windows 11, Rust 1.97.1, Ryzen 7 7700, release builds; twenty backups a run
  for each of the three moment programs, one run for the others.
- The programs were committed before they ran: e77f983. The three backup
  programs are 0.3.0's unchanged; `issue_aggregate_in_memory` also aggregates
  in memory and with `ExactSum`; `issue_files_with_rows` writes the row and
  the file in one transaction, commits one, rolls one back and kills the
  process inside a third; `issue_one_key` writes beside the old value, opens
  it with the old key among the older ones, and reseals.

```sh
cd <research>/dashbin/tinystore-repro/0.4.0
cargo build --release --bins
./target/release/issue_aggregate_in_memory
./target/release/issue_backup_moments; ./target/release/issue_backup_moments metrics-first
./target/release/issue_backup_moments_records
./target/release/issue_files_with_rows; ./target/release/issue_one_key
```

Raw output: [data/tinystore-0.4.0-recheck-2026-10-27/outputs.txt](data/tinystore-0.4.0-recheck-2026-10-27/outputs.txt),
the temp directory written as `<tmp>`.

## What follows

Chapters 4, 5 and 12 drop the four workarounds they carried for these issues.
Of everything Dashbin has asked of TinyStore, #29 — a gauge corrected behind
what is packed — is the one still open.
