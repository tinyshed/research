# TinyStore 0.3.0: what a restore holds across files, 27 October 2026

Chapter 4 keeps everything that must commit together in one database and the
rest apart. A review asked what a restore promises across the files that are
apart: a collector's window in the database against its samples in Metrics, a
package in files against its row, secrets restored without their key. Each
was run as a program against the published crate.

| Question | Answer | What follows |
|---|---|---|
| A window in the database and its samples in Metrics, backed up twenty times while a collector writes | each engine is a moment of its own, copied in the order the program opened them: the database opened first is behind by 9–10 samples in 20 of 20; Metrics opened first puts the window **ahead** by 3–10 in 20 of 20 | [tinyshed/tinystore#35](https://github.com/tinyshed/tinystore/issues/35): one moment for the whole store; until then Dashbin opens its database first and tests the direction |
| The same against events in Records | the database opened first is behind in 20 of 20 | the same rule covers alert states and their transitions |
| A process killed between writing a package and its row | file first: a package no row names; row first: a row naming no package | [tinyshed/tinystore#36](https://github.com/tinyshed/tinystore/issues/36): files inside a database; until then the package first, a sweep, a check at start |
| A backup without its key, restored | the bucket does not open and no key is made; with a new key nothing reads or writes until the bucket is cleared; the old key found afterwards opens neither the old values, gone, nor the new | [tinyshed/tinystore#37](https://github.com/tinyshed/tinystore/issues/37): old keys kept for reading; until then the hub starts without secrets and asks again |

## Environment and reproduction

- `tinystore = "=0.3.0"` from crates.io, features `metrics`, `records`,
  `sql`, `kv`, `blobs`, `backup`.
- Windows 11, Rust 1.97.1, Ryzen 7 7700, release builds; twenty backups a run
  for the moments, one run each for the other two.
- The programs were committed before they ran: e5bdcbb, 535e967 for the
  records variant, 5463456 for the order the engines are opened in. The first
  run of `issue_backup_moments` was made before 5463456 and is kept in
  `outputs.txt`; the issue quotes the run of the program as it now is.
- Read beside the runs: `backup/snapshot.rs` says "two engines are two
  moments, which is safe as no write spans two files", and the store keeps its
  engines in a `Vec`, in the order they were opened.

```sh
cd <research>/dashbin/tinystore-repro/0.3.0
cargo build --release --bins
./target/release/issue_backup_moments; ./target/release/issue_backup_moments metrics-first
./target/release/issue_backup_moments_records
./target/release/issue_files_with_rows; ./target/release/issue_one_key
```

Raw output: [data/tinystore-0.3.0-backup-2026-10-27/](data/tinystore-0.3.0-backup-2026-10-27/),
the temp directory written as `<tmp>`.

## What follows

A restore promises that nothing in it names what it does not hold. Dashbin
keeps that with its own order until #35 gives one moment, with the order of
an install until #36, and by asking for secrets again until #37; chapter 4
says each and names its issue.
