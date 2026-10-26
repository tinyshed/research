# TinyStore 0.1.1 against what flows need, 25 October 2026

Chapter 7 puts collectors, alerts, webhooks and buttons on TinyStore's jobs,
which run a job at least once. That holds only if every step bears running
twice, so three of TinyStore's behaviours a step relies on were run as
programs against the published crate rather than read from its docs.

| Question | Answer | What follows |
|---|---|---|
| An event appended twice, as a step retried after a crash appends it? | Two events, with `event().append` and with `append_all`; an event has no id, and Records opens in no database's file, so it cannot commit with the job (NOT-01) | [tinyshed/tinystore#23](https://github.com/tinyshed/tinystore/issues/23): an id on append |
| The same metrics batch ingested again? | `Ok` while its stretch is in the head; **after maintenance has packed it, the identical batch is refused whole: `behind what its series has packed: too old`** | [tinyshed/tinystore#24](https://github.com/tinyshed/tinystore/issues/24): an identical sample behind the frontier is nothing; until then a collector drops what is behind the window it knows it kept |
| A different value at a packed time? | Refused `too old`, as the docs say (NOT-08), and the stored value and the hour's exact sum are unchanged | by design, not asked |
| A repeating job's runs missed while the store was closed? | Collapse into one: every second, closed for 5 s, then 4 runs in 3.5 s — the first 2 ms after reopening with its `at` the first missed time, 5 s back, the rest on time | chapter 7 relies on it; an alert reads up to when it runs, not when it was due |

## Environment and reproduction

- `tinystore = "=0.1.1"` from crates.io, features `kv`, `metrics`,
  `records`, `sql`, `async`, `jobs`, `config`. Windows 11, Rust 1.97.1, functional
  checks, one pass each.
- The programs were committed before they ran: 3aa66f8, then 7e7cde3 and
  1da376f for the retries of the metrics batch, c9f19a1 for the two
  programs the issues carry, and 70b46f3 and 3780c1e for the seven asked
  besides. Before writing anything up, TinyStore's open and
  closed issues (#1–#22) and its documentation were read: NOT-08 is the stated
  reason for refusing a sample behind the sealed frontier, and the complaint
  argues with it rather than past it.

```sh
cd <research>/dashbin/tinystore-repro/0.1.1
cargo build --release --bins
./target/release/event_twice; ./target/release/sample_twice; ./target/release/missed_runs
./target/release/issue_event_id; ./target/release/issue_packed_retry
./target/release/issue_zone_buckets; ./target/release/issue_prometheus_rest; ./target/release/issue_gauge_correction
./target/release/issue_group_rate; ./target/release/issue_config_no_store
./target/release/node_exporter_text <a saved node_exporter /metrics>
```

Raw output: [data/tinystore-0.1.1-flows-2026-10-25/](data/tinystore-0.1.1-flows-2026-10-25/),
the temp directory written as `<tmp>`.

## What it means for flows

A collector whose step dies after `ingest` and before its answer is kept is
retried; if maintenance packed the stretch in between, every retry fails the
same way, and since the window moves only when the step finishes, that series
is stuck for good. NOT-08's reason is that a packed counter's increase would
change, and an identical sample changes nothing, so the ask keeps NOT-08
whole: accept what is already there as nothing, or skip and report what is
behind the frontier, or let the caller read the frontier.

A webhook's event kept twice is a deploy shown twice on a dashboard. `once`
does not close the gap: its answer and the append live in different files, so
a crash between them repeats the append the same way.

Business data adds a case NOT-08 was not written for: yesterday's revenue
changes when a refund lands, and a collector that re-reads the last days to
catch that is refused even for a gauge, which has no increase to protect.
That is asked apart, in #29 below.

## Asked besides

Nothing in chapter 7 waited on these. They are what Dashbin will meet next on
TinyStore, filed now so its maintainers can weigh them, including the ones we
consider optional. Each carries a whole program and its output. TinyStore
0.2.0 closed all of them but #29, and #23 and #24 above, as
[the recheck](tinystore-0.2.0-recheck-2026-10-26.md) measured.

| Ask | What the program shows | Issue |
|---|---|---|
| Day, week and month buckets in a zone | A day is a UTC day in metrics and in records: one order an hour over two Moscow days reads 3, 24, 21 | [#25](https://github.com/tinyshed/tinystore/issues/25) |
| Keep `# HELP` and `# UNIT` from a scrape | Help and unit come back empty; a real node_exporter 1.12.1 carries 292 help lines, all dropped | [#26](https://github.com/tinyshed/tinystore/issues/26) |
| Cut a long scrape at family boundaries | 12 000 samples, written to stand for kube-state-metrics on a large cluster, refused whole: `send it in parts` | [#27](https://github.com/tinyshed/tinystore/issues/27) |
| Read Prometheus 3's quoted UTF-8 names | The whole text refused at the first quoted name | [#28](https://github.com/tinyshed/tinystore/issues/28) |
| Correct a gauge behind the frontier | Yesterday's revenue corrected from 1000 to 900 is refused `too old` and stays 1000 | [#29](https://github.com/tinyshed/tinystore/issues/29) |
| A rate per job group | Two bots allowed 2 a second each share the queue's 2: four messages each take 4 s rather than 2 | [#30](https://github.com/tinyshed/tinystore/issues/30) |
| Config without a store | Reading a toml file and the environment opens a store and leaves `LOCK` and `kv.db` with its WAL behind | [#31](https://github.com/tinyshed/tinystore/issues/31) |

Until they land, Dashbin will group by day in the source's own SQL when a
zone matters, cut a long scrape itself, read a corrected number from the
source rather than from history, give a provider with limits per account a
queue per account, and read the agent's file with `toml` and `serde`
directly. Each workaround will carry the issue's link and go when it closes.
