docs (guides/metrics/ingesting, "From Prometheus") say the text format isn't read by the store yet: parse it with a library of your language and ingest each series.

for dashbin the agent will scrape node_exporter / postgres_exporter / mysqld_exporter on each server and push into the store. so i have to: parse the text format (and openmetrics), map types right (counter -> counter, gauge -> gauge, histogram -> `_bucket` as `histogram` with `le` plus the `_sum` / `_count` counters), deal with summaries and untyped, unescape labels, handle optional timestamps, and handle restarts (the docs want a zero at the time a counter started, which openmetrics `_created` gives and plain text doesn't).

that's exactly the kind of thing that's easy to get subtly wrong (a histogram ingested as plain counters reads fine until you ask for a percentile), and every tinystore user who scrapes anything will write it again.

input looks like:

```
# HELP node_cpu_seconds_total Seconds the CPUs spent in each mode.
# TYPE node_cpu_seconds_total counter
node_cpu_seconds_total{cpu="0",mode="idle"} 1.23456789e+06
node_cpu_seconds_total{cpu="0",mode="user"} 4567.89
# TYPE http_request_duration_seconds histogram
http_request_duration_seconds_bucket{le="0.1"} 240
http_request_duration_seconds_bucket{le="0.5"} 310
http_request_duration_seconds_bucket{le="+Inf"} 320
http_request_duration_seconds_sum 41.7
http_request_duration_seconds_count 320
```

## what i'd like

an optional feature (`expo`?) with something like:

```rust
let batch: Vec<Series> = tinystore::metrics::expo::parse(text, scraped_at)?
    .with_label("instance", "site-3");
stats.ingest(&batch)?;
```

- gives back ready `Series` with kinds set, histograms already split the way `ingest` wants them
- an error names the line it choked on
- no http client inside, scraping stays the caller's job

## open questions

- summary (the `quantile` series): gauge per quantile, or refuse?
- untyped: gauge?
- `_created`: use it as the counter's start (the zero), when it's there?

this was on the old list of asks from the dashbin side too, now there's an actual caller for it.
