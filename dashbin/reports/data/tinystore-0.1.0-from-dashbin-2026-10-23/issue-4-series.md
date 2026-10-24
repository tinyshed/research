`MOST_SERIES = 100_000` in `metrics/registry.rs`, and nothing in `Options` moves it. hitting it:

```
metrics node_metric{host="site-50",id="0"}: a series past the 100000 a store holds: limit
```

for monitoring that's not a lot. an exporter gives hundreds to a few thousand series per host, and a histogram multiplies that by its buckets. 50 hosts x 2000 series is already the cap, and the first server past it just doesn't get in.

## repro

tinystore 0.1.0 from crates.io, rust 1.97.1, windows 11.

```rust
use std::time::{Duration, Instant, SystemTime};
use tinystore::metrics::Series;
use tinystore::{Options, Store};

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-series");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let stats = store.metrics()?;
    let at = SystemTime::now() - Duration::from_secs(60);
    let started = Instant::now();
    // 50 hosts x 2000 series
    for host in 0..50 {
        let batch: Vec<Series> = (0..2000)
            .map(|i| Series::gauge("node_metric").label("host", format!("site-{host}")).label("id", i).sample(at, 1.0))
            .collect();
        stats.ingest(&batch)?;
    }
    println!("100000 series in {:?}", started.elapsed());
    let one_more = Series::gauge("node_metric").label("host", "site-50").label("id", 0).sample(at, 1.0);
    match stats.ingest(&[one_more]) {
        Ok(()) => println!("one more series: accepted"),
        Err(e) => println!("one more series: {e} (kind {:?})", e.kind()),
    }
    store.close()
}
```

```
100000 series in 4.8101765s
one more series: <tmp>\metrics.db: metrics node_metric{host="site-50",id="0"}: a series past the 100000 a store holds: limit (kind Limit)
```

## good news: churn is fine

checked whether series that stopped give their slot back (containers that come and go). they do: 100k series with keep 1h, a `TestClock` moved 48h, `metrics::maintain` -> `Maintenance { expired: 100000, reclaimed: 100000, .. }`, and a new series is accepted after that. so this is only about the ceiling, nothing leaks.

## what i'd like

- an `Options` field for it. 100k as the default is fine.
- a line in the docs on what one series costs (memory in the registry / postings, disk), so whoever raises it knows what they're paying.

we planned the format around ~1M active series back when this was still inside dashbin, so the code might already be fine with a bigger number and it's only the const.
