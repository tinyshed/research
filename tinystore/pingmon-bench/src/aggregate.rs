//! Times aggregates of replayed Pingmon stores in turns: aggregate <store>...
//! An hour and twenty hours, both on whole hours, every 1m, 5m and 1h, by
//! target, `avg` and `max` of `ping_rtt_ms`; the median of 21 passes, and
//! what `explain` says each decodes.

use std::time::{Duration, Instant, UNIX_EPOCH};

use tinystore::metrics::Op;
use tinystore::{Options, Store};

const PASSES: usize = 21;

fn main() {
    let dirs: Vec<String> = std::env::args().skip(1).collect();
    let stores: Vec<Store> =
        dirs.iter().map(|dir| Store::open(dir, Options { background: false, ..Options::default() }).unwrap()).collect();
    // 2026-10-27 08:00 UTC, before the newest samples, which wait unpacked
    let end = UNIX_EPOCH + Duration::from_secs(1_793_088_000);
    let ranges = [("1 hour", Duration::from_hours(1)), ("20 hours", Duration::from_hours(20))];
    let everys = [("1m", Duration::from_mins(1)), ("5m", Duration::from_mins(5)), ("1h", Duration::from_hours(1))];
    println!("{:<9} {:<5} {:<4} {}", "range", "every", "op", dirs.join(" | "));
    for (range_name, range) in ranges {
        for (every_name, every) in everys {
            for (op_name, op) in [("avg", Op::Avg), ("max", Op::Max)] {
                let query = |store: &Store| {
                    let metrics = store.metrics().unwrap();
                    metrics.query().name("ping_rtt_ms").from(end - range).to(end).every(every).by(&["target"])
                };
                let mut runs: Vec<Vec<Duration>> = vec![Vec::new(); stores.len()];
                let mut buckets = vec![0; stores.len()];
                for _ in 0..PASSES {
                    for (index, store) in stores.iter().enumerate() {
                        let query = query(store);
                        let started = Instant::now();
                        let page = query.aggregate(op).unwrap();
                        runs[index].push(started.elapsed());
                        buckets[index] = page.groups.iter().map(|group| group.buckets.len()).sum::<usize>();
                    }
                }
                let cells: Vec<String> = stores
                    .iter()
                    .enumerate()
                    .map(|(index, store)| {
                        runs[index].sort();
                        let plan = query(store).explain(op).unwrap();
                        let median = runs[index][PASSES / 2].as_secs_f64() * 1000.0;
                        format!("{median:7.2} ms {:>6} decoded {:>4} buckets", plan.decoded, buckets[index])
                    })
                    .collect();
                println!("{range_name:<9} {every_name:<5} {op_name:<4} {}", cells.join(" | "));
            }
        }
    }
}
