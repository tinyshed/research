use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tinystore::metrics::{Op, Series, Value};
use tinystore::{Options, Store};

fn main() -> tinystore::Result<()> {
    // two minutes ago, readings a dashboard takes from a live source and never stores:
    // ten of a gauge, and five of a counter its process reset once
    let start = UNIX_EPOCH + Duration::from_secs(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() / 60 * 60 - 120);
    let at = |s: u64| start + Duration::from_secs(s);
    let gauge: Vec<(SystemTime, f64)> = (0..10).map(|i| (at(i * 5), 0.1)).collect();
    let counter: Vec<(SystemTime, f64)> = [100.0, 150.0, 200.0, 20.0, 70.0].iter().enumerate().map(|(i, v)| (at(i as u64 * 10), *v)).collect();

    // by hand, the way a caller writes it
    let sum: f64 = gauge.iter().map(|(_, v)| v).sum();
    let increase = counter.last().unwrap().1 - counter[0].1;
    println!("by hand: sum {sum:?}, increase {increase:?}");

    // through metrics, which takes a store and writes the samples first
    let started = Instant::now();
    let dir = std::env::temp_dir().join("aggregate-in-memory-repro");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let stats = store.metrics()?;
    let reading = gauge.iter().fold(Series::gauge("reading").label("source", "live"), |s, (t, v)| s.sample(*t, *v));
    let requests = counter.iter().fold(Series::counter("requests").label("source", "live"), |s, (t, v)| s.sample(*t, *v));
    stats.ingest(&[reading, requests])?;
    let one = |name: &str, op: Op| -> tinystore::Result<Value> {
        let page = stats.query().name(name).from(start).to(at(60)).aggregate(op)?;
        Ok(page.groups[0].buckets[0].value)
    };
    println!("metrics: sum {:?}, increase {:?}", one("reading", Op::Sum)?, one("requests", Op::Increase)?);
    store.close()?;
    let mut left: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    left.sort();
    println!("what it took: a store, {:.1?} to open, ingest and read, and on disk {left:?}", started.elapsed());
    Ok(())
}
