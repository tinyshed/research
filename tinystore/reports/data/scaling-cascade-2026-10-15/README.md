# Scaling cascade raw data

`bench.jsonl` is the release harness's stdout, captured 2026-10-15
08:40:21–08:40:30 UTC. It contains the configuration row, six interleaved
capture-cost rows, 50 per-operation engine packet-size rows, and six
interleaved star/tree bootstrap rows. `resources.json` measures the whole
process, including untimed setup, using Linux `getrusage` from its Python
parent. `bench-stderr.txt` is empty for the successful run.

`environment.json` records non-secret machine facts, the exact TinyStore and
research base commits, build versions and every measured Rust/Cargo/config
input's SHA-256. `harness.tar.gz` preserves the working-tree inputs used before
the harness was committed. The
source worktree is recreated at `source-revision`; it is not copied into the
archive. The source hash is provisional provenance, not a Git commit.

`batch/` records a separate follow-up, 08:45:15–08:45:17 UTC, with its own
environment, archived harness revision, resources and six alternating cap-1 /
cap-32 rows. The runner change adds batching measurements; the receiver and
TCP transport are identical to the first archive.

`summary.json` derives the reported min/median/max values from those raw
rows. `demo.txt` preserves the separate loopback integration run's assertions.

`tests.txt` records 21 passing top-level tests and one ignored subprocess
worker that its parent launches twice. `clippy.txt` records warnings-denied
lint success; an empty `fmt.txt` accompanies the successful formatting check.
`setup-error.txt` preserves the initial missing-`/usr/bin/time` attempt, which
did not execute a measurement. Machine paths in text logs are replaced by
`<repo>` or `<tmp>`; numeric output is unchanged.

Reports: [cascade](../../scaling-cascade-2026-10-15.md),
[handoff](../../scaling-handoff-2026-10-15.md).
