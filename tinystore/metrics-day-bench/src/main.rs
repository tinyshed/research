//! What a dashboard's reads of a day cost: 600 series of a sample a minute
//! for two days, read as one number, by the hour, by five minutes and by the
//! calendar's day, a counter's increase and a gauge's average.
//!
//! `metrics-day-bench <ramp|noisy> <passes>` prints, a read a line, the
//! least and the median time of its passes and the samples it decoded.
use std::time::{Duration, Instant, SystemTime};

use tinystore::metrics::{Op, Query, Series};
use tinystore::{Calendar, Options, Store};

const SERIES: usize = 600;
const MINUTES: u64 = 2 * 24 * 60;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let noisy = args.next().as_deref() == Some("noisy");
    let passes: usize = args.next().map_or(Ok(31), |passes| passes.parse())?;
    let dir = std::env::temp_dir().join(format!("ts-metrics-day-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let holder = Store::open(&dir, Options::default())?;
    let metrics = holder.metrics()?;
    // whole hours, so that every run cuts its blocks alike
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs()
        / 3600
        * 3600;
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(now - MINUTES * 60);
    let mut totals = vec![0u64; SERIES];
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    for hour in 0..MINUTES / 60 {
        let mut batch = Vec::with_capacity(2 * SERIES);
        for series in 0..SERIES {
            let labelled = |one: Series| {
                one.label("node", format!("node{}", series % 3))
                    .label("domain", format!("domain{}.example", series / 3))
            };
            let mut counter = labelled(Series::counter("bytes_total"));
            let mut gauge = labelled(Series::gauge("queue_depth"));
            for minute in hour * 60..(hour + 1) * 60 {
                let at = start + Duration::from_secs(minute * 60 + series as u64 % 60);
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                totals[series] += seed % 5000;
                let counted = if noisy {
                    totals[series]
                } else {
                    minute * (series as u64 % 7 + 1)
                };
                let depth = if noisy {
                    (seed % 977) as f64 / 8.0
                } else {
                    (minute % 60) as f64
                };
                counter = counter.sample(at, counted as f64);
                gauge = gauge.sample(at, depth);
            }
            batch.push(counter);
            batch.push(gauge);
        }
        metrics.ingest(&batch)?;
    }
    while tinystore::metrics::maintain(&holder)?.packed > 0 {}
    let day = Duration::from_secs(24 * 3600);
    let hour = Duration::from_secs(3600);
    let bytes = || metrics.query().name("bytes_total").since(day);
    let depth = || metrics.query().name("queue_depth").since(day);
    let reads: [(&str, Query, Op); 7] = [
        ("increase, one number", bytes(), Op::Increase),
        ("increase by the hour", bytes().every(hour), Op::Increase),
        (
            "increase by the hour, by node",
            bytes().every(hour).by(&["node"]),
            Op::Increase,
        ),
        (
            "increase by 5 minutes",
            bytes().every(hour / 12),
            Op::Increase,
        ),
        (
            "increase by the day, Moscow",
            bytes().calendar(Calendar::Day, "Europe/Moscow"),
            Op::Increase,
        ),
        ("avg by the hour", depth().every(hour), Op::Avg),
        ("max by the hour", depth().every(hour), Op::Max),
    ];
    for (what, query, op) in reads {
        let plan = query.explain(op)?;
        let mut took = Vec::with_capacity(passes);
        for _ in 0..passes {
            let begun = Instant::now();
            let answer = query.aggregate(op)?;
            took.push(begun.elapsed());
            std::hint::black_box(answer);
        }
        took.sort();
        let ms = |took: Duration| took.as_secs_f64() * 1000.0;
        println!(
            "{what:30} {:>7.2} {:>7.2} ms  decoded {:>7}",
            ms(took[0]),
            ms(took[took.len() / 2]),
            plan.decoded
        );
    }
    drop(metrics);
    holder.close()?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}
