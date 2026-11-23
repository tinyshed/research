# metrics-day-bench

What a dashboard's reads of a day cost in TinyStore's metrics: 600 counters
and 600 gauges of a sample a minute for two days, read as one number, by the
hour, by five minutes and by a calendar's day.

One program, `src/main.rs`, built twice: `candidate/` against
`../source`, the submodule, and `base/` against `../base`, a worktree of the
submodule at the commit a candidate is compared with:

```sh
git -C ../source worktree add ../base <base commit>
(cd base && cargo build --release) && (cd candidate && cargo build --release)
base/target/release/metrics-day-bench-base noisy 31
candidate/target/release/metrics-day-bench-candidate noisy 31
```

`ramp` writes counters that grow by the same step, `noisy` ones that grow by
a random step; a line is a read's least and median time of its passes and
the samples it decoded. `.cargo/config.toml` carries SQLite's flags, which
TinyStore's workspace sets for itself only.
