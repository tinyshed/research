// do series that stopped long ago (and aged past keep) give their slot back?
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tinystore::metrics::Series;
use tinystore::{Options, Store, TestClock};

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-churn");
    let _ = std::fs::remove_dir_all(&dir);
    let t0 = UNIX_EPOCH + Duration::from_secs(1_800_000_000);
    let clock = Arc::new(TestClock::new(t0));
    let store = Store::open(&dir, Options { clock: Some(clock.clone()), background: false, ..Options::default() })?;
    let stats = store.metrics()?;
    let keep = Duration::from_hours(1);
    // yesterday's containers: 100 000 series, each sent once
    for batch in 0..50 {
        let series: Vec<Series> = (0..2000)
            .map(|i| Series::gauge("container_cpu").label("container", format!("old-{batch}-{i}")).keep(keep).sample(t0, 1.0))
            .collect();
        stats.ingest(&series)?;
    }
    println!("100000 series written at t0, keep {keep:?}");
    clock.advance(Duration::from_hours(48));
    store.clock_moved();
    let done = tinystore::metrics::maintain(&store)?;
    println!("48 h later, maintenance: {done:?}");
    let now: SystemTime = store.now();
    let fresh = Series::gauge("container_cpu").label("container", "new-0").keep(keep).sample(now, 1.0);
    match stats.ingest(&[fresh]) {
        Ok(()) => println!("a new container's series: accepted"),
        Err(e) => println!("a new container's series: {e}"),
    }
    store.close()
}
