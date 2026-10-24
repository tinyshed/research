// 0.1.1: the cap is an option now
use std::time::{Duration, SystemTime};
use tinystore::metrics::Series;
use tinystore::{Options, Store};

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-series-011");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options { max_series: 1_000_000, ..Options::default() })?;
    let stats = store.metrics()?;
    let at = SystemTime::now() - Duration::from_secs(60);
    for host in 0..50 {
        let batch: Vec<Series> = (0..2000)
            .map(|i| Series::gauge("node_metric").label("host", format!("site-{host}")).label("id", i).sample(at, 1.0))
            .collect();
        stats.ingest(&batch)?;
    }
    let one_more = Series::gauge("node_metric").label("host", "site-50").label("id", 0).sample(at, 1.0);
    match stats.ingest(&[one_more]) {
        Ok(()) => println!("series 100001 with max_series 1M: accepted"),
        Err(e) => println!("series 100001 with max_series 1M: {e}"),
    }
    store.close()
}
