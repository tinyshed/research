# TinyStore 0.2.0 against Dashbin's issues, 26 October 2026

TinyStore 0.2.0 closed eight of the nine issues Dashbin filed on 25 October
([tinyshed/tinystore](https://github.com/tinyshed/tinystore) #23–#28, #30,
#31) and left #29 open with its reasons. Each program was rerun against the
published 0.2.0 crate rather than taken from the closing comment; the ones
whose fix is a new call were moved onto it.

| Issue | 0.1.1 | 0.2.0 | What follows for Dashbin |
|---|---|---|---|
| #23 an event appended twice | two events; no id to give | `.id(delivery)`: the first append answers `true`, the second `false`, the stream holds one; a list with that id and a new one writes 1; without an id, still two | an event a flow keeps carries an id, and a crash between the append and the job's answer writes nothing twice |
| #24 the same batch after packing | refused whole, `too old` | `Ok`, nothing changes; a different value behind what is packed is still `too old`, now naming `at` and `packed` | the collector's workaround, dropping what is behind the window it knew it kept, is not built |
| #25 day buckets | UTC days: 3, 24, 21 | `.calendar(Calendar::Day, "Europe/Moscow")`: 24 and 24, in metrics and in records; Berlin across 25 October's clock change: 24 and 25 hours | a dashboard's day is its zone's; grouping by day in the source's SQL is not needed for that |
| #26 help and unit from a scrape | both empty | the family's help on every name it writes, its unit on `_sum` and not on `_bucket` or `_count`; a name the code described keeps the code's; node_exporter's text: help on all three names, no unit (it writes none) | the metric picker shows a scrape's help |
| #27 a long scrape | past 10 000 samples refused | 12 000 written whole; 100 001 refused naming the line; 100 000 written whole and holding the metrics writer, below | no cutting up to 100 000 samples; the writer it holds is a fact collectors are planned around |
| #28 Prometheus 3's quoted names | the whole text refused | read and kept with their dots: `http.server.request.duration` with its `method` label | an OpenTelemetry collector can be scraped |
| #29 a gauge corrected behind what is packed | `too old` | still `too old`, open by design; the error names `at` and `packed` | a number its source corrects later is not kept as a series yet; chapter 7 leaves it open |
| #30 a rate per group | each bot one message a second | `.rate(30, 1s).group_rate(2, 1s)`: each bot two at 0 s and two at 1 s | an instance is a group with its provider's rate, not a queue of its own |
| #31 config without a store | `LOCK` and `kv.db` with its WAL left behind | `tinystore::config::load`: nothing beside the file; a `Live` field refused naming it | the agent reads its file through TinyStore's config, with `Secret` and its errors |

What 0.1.1 already did still holds: an abandoned read stops (`select 1` after
it in 0.2 ms), `once` runs a 2 MiB answer once (0.50 s for four callers), the
build warning is the logger's first line, `max_series` takes series 100 001,
a Prometheus label both sides give is refused naming the line, and a step's
missed runs collapse into one.

## Environment and reproduction

- `tinystore = "=0.2.0"` from crates.io, features `kv`, `metrics`, `records`,
  `sql`, `async`, `jobs`, `config`.
- Windows 11, Rust 1.97.1, Ryzen 7 7700, release builds; functional checks,
  one pass each, the large scrape three.
- The harness was committed before it ran: 394c3ab, and abc193a for the large
  scrape's second form (below). Ten programs are 0.1.1's unchanged.
  `issue_event_id`, `issue_zone_buckets`, `issue_group_rate` and
  `issue_config_no_store` moved onto the new calls; `issue_prometheus_rest`
  also reads the quoted name back and ingests a histogram;
  `issue_gauge_correction` also prints the error's `at` and `packed`;
  `big_scrape` is new.
- `node_exporter_text` read the same saved node_exporter 1.12.1 text 0.1.1
  read (81 734 bytes, SHA-256 `67a38ce7…`); the text names a machine, so it is
  not committed, and any node_exporter's `/metrics` serves.

```sh
cd <research>/dashbin/tinystore-repro/0.2.0
cargo build --release --bins
./target/release/issue_event_id; ./target/release/issue_packed_retry; ./target/release/issue_zone_buckets
./target/release/issue_prometheus_rest; ./target/release/issue_gauge_correction; ./target/release/issue_group_rate
./target/release/issue_config_no_store; ./target/release/event_twice; ./target/release/sample_twice
./target/release/missed_runs; ./target/release/warning; ./target/release/once; ./target/release/series
./target/release/prom; ./target/release/pileup; ./target/release/big_scrape
./target/release/node_exporter_text <a saved node_exporter /metrics>
```

Raw output: [outputs.txt](data/tinystore-0.2.0-recheck-2026-10-26/outputs.txt)
for every program, and [big-scrape.txt](data/tinystore-0.2.0-recheck-2026-10-26/big-scrape.txt)
for the large scrape's three runs; the temp directory written as `<tmp>`.

## A scrape of 100 000 samples holds the writer

One text of 100 000 series, 5.8 MB, is written whole, and while it is written
no other ingest into `metrics.db` is: a collector of another source writing
one sample every 10 ms beside it waited up to

| Scrape | The whole ingest | The longest small write beside it | Small writes in that time |
|---|---|---|---|
| first, every series new | 2.75–2.87 s | 2.57–2.70 s, asked 0.18 s in | 18–19 |
| second, the same series | 0.73–0.76 s | 0.50–0.53 s, asked 0.22 s in | 22–23 |

So reading the text takes about 0.2 s and the write holds the writer for the
rest. The maintainers measured 4.5 s and 0.8 s on the same CPU with their own
test; this text's labels are shorter. 100 001 samples are refused with
`a text past 100000 samples or 32 MiB: send it in parts`.

The first form of this program wrote its one sample 100 ms in, before the
writer was taken, and it answered at once; that run is in `outputs.txt`, says
nothing about the wait, and is not used.

For Dashbin: collectors share one writer. A scrape of 100 000 series every
15 s holds it about 3% of the time after its first run, and a collector of
another source lands up to half a second late meanwhile; the first scrape of
a new exporter stalls every other collector for nearly three seconds. Reads
do not wait.

## A step's missed runs: the same collapse, a different next run

Closed for five seconds and reopened, a job repeating every second ran once
for everything it missed, as in 0.1.1. What changed is the run after it:
0.1.1 ran next a second after the late run, 0.2.0 at the next second of the
original schedule, 0.38 s after reopening, so 3.5 s held five runs rather
than four. A collector's windows do not care; it is noted because chapter 7
says "then on time".

## What follows

Chapter 7 drops its two workarounds, gives a kept event an id and an
instance's actions a group rate, and leaves a corrected number open with
#29's suggestion beside it. The agent's configuration, chapter 8, starts from
`config::load`.
