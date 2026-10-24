// the 100 000th + 1 series is refused, and nothing in Options moves the bound
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
    // 50 hosts x 2000 series, the way 50 servers with node_exporter + postgres_exporter would look
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
