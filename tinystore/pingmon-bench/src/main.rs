//! Replays a CSV of `name,target,ms,value` into a store as Pingmon wrote it:
//! every 10 s the samples since the last flush, and maintenance every minute,
//! on a clock that follows the data.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};

use tinystore::metrics::Series;
use tinystore::{Options, Store, TestClock};

const FLUSH: i64 = 10_000;
const MAINTAIN: i64 = 60_000;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (csv, dir) = (&args[1], &args[2]);
    let mut rows: Vec<(String, String, i64, f64)> = std::fs::read_to_string(csv)
        .unwrap()
        .lines()
        .map(|line| {
            let mut parts = line.split(',');
            let name = parts.next().unwrap().to_owned();
            let target = parts.next().unwrap().to_owned();
            let ms = parts.next().unwrap().parse().unwrap();
            let value = parts.next().unwrap().parse().unwrap();
            (name, target, ms, value)
        })
        .collect();
    rows.sort_by_key(|row| row.2);
    let at = |ms: i64| UNIX_EPOCH + Duration::from_millis(ms as u64);

    let first = rows[0].2;
    let clock = Arc::new(TestClock::new(at(first)));
    let options = Options { clock: Some(clock.clone()), background: false, ..Options::default() };
    let store = Store::open(dir, options).unwrap();
    let metrics = store.metrics().unwrap();
    let started = Instant::now();

    let mut flush_at = first - first % FLUSH + FLUSH;
    let mut maintain_at = first - first % MAINTAIN + MAINTAIN;
    let mut pending: BTreeMap<(String, String), Vec<(i64, f64)>> = BTreeMap::new();
    let mut flush = |pending: &mut BTreeMap<(String, String), Vec<(i64, f64)>>| {
        let batch: Vec<Series> = std::mem::take(pending)
            .into_iter()
            .map(|((name, target), samples)| {
                let series = Series::gauge(name).label("target", target).keep(Duration::from_hours(720));
                samples.into_iter().fold(series, |series, (ms, value)| series.sample(at(ms), value))
            })
            .collect();
        if !batch.is_empty() {
            metrics.ingest(&batch).unwrap();
        }
    };
    let (mut packed, mut blocks) = (0, 0);
    for (name, target, ms, value) in rows {
        while ms >= flush_at {
            clock.set(at(flush_at)).unwrap();
            flush(&mut pending);
            if flush_at >= maintain_at {
                let done = tinystore::metrics::maintain(&store).unwrap();
                (packed, blocks) = (packed + done.packed, blocks + done.blocks);
                maintain_at += MAINTAIN;
            }
            flush_at += FLUSH;
        }
        pending.entry((name, target)).or_default().push((ms, value));
    }
    clock.set(at(flush_at)).unwrap();
    flush(&mut pending);
    let done = tinystore::metrics::maintain(&store).unwrap();
    (packed, blocks) = (packed + done.packed, blocks + done.blocks);
    store.close().unwrap();
    println!("packed {packed} samples into {blocks} blocks in {:.1?}", started.elapsed());
}
