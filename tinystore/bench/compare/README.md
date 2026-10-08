# Engine comparison harness

This module builds against the repository's TinyStore submodule at
`../../source`. The current pin is
`e307c48a40126aad0e2873b6bf3aaedef8115483`. Compile without running measurement
loops:

```sh
GOWORK=off CGO_ENABLED=0 GOMAXPROCS=1 go test -run '^$' ./...
```

The TinyStore metric adapter keeps the corpus's Prometheus `__name__` as
`metrics.Series.Name` and `metrics.Range.Name`, with ordinary equality labels
in `metrics.Labels` and `Range.Match`. Exact-series and single-label reads still
count decoded samples in `[from, to)`. The records adapter uses `records.Scan`
and follows `Page.More` and `Page.Next`, retaining its 10,000-record page limit
and the same stream and time filters.

The compatibility edits keep the harness buildable; they do not replace any
published measurement. The former publication pin was `034b11d`; each older
report still identifies the TinyStore revision its round measured.
Historical rounds must use both the TinyStore commit
named in their dated report and the matching historical revision of this
harness. Checking an old TinyStore commit out under this current adapter alone
can fail because its public API changed. Use a separate checkout or worktree
for historical reproduction, and retain that round's toolchain, contender
versions, corpus hashes, options, and commands. Original dated reports and raw
measurement files remain the evidence for those runs.
