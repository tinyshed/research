use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tinystore::metrics::{Series, maintain};
use tinystore::{Options, Store};

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("packed-retry-repro");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let stats = store.metrics()?;

    // what a collector ingests in one job step: three hours of 15 s samples, read from a database
    let hour = 3600;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let start = UNIX_EPOCH + Duration::from_secs(now / hour * hour - 3 * hour);
    let batch = || {
        let mut series = Series::gauge("revenue").label("site", "kazan");
        for i in 0..3 * hour / 15 {
            series = series.sample(start + Duration::from_secs(15 * i), 1.0);
        }
        series
    };

    stats.ingest(&[batch()])?;
    // the worker dies before the step's answer is kept, so the next attempt ingests the same batch
    println!("the same batch again, right away: {:?}", stats.ingest(&[batch()]).map(|_| "ok"));
    maintain(&store)?;
    println!("the same batch again, after maintenance: {:?}", stats.ingest(&[batch()]).map(|_| "ok"));
    store.close()
}
