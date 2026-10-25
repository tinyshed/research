// a sample written again at a time already written, as a collector's step that crashed after ingest writes it:
// in the head the later value wins, as the docs say; what happens once that stretch of the series is packed?
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tinystore::metrics::{Series, maintain};
use tinystore::{Options, Store};

fn points(stats: &tinystore::metrics::Metrics, at: SystemTime) -> tinystore::Result<Vec<f64>> {
    let read = stats.query().name("revenue").from(at - Duration::from_secs(1)).to(at + Duration::from_secs(1)).read()?;
    Ok(read.iter().flat_map(|series| series.samples().iter().filter(|s| s.at == at).map(|s| s.value)).collect())
}

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-sample-twice-011");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let stats = store.metrics()?;

    // three hours of 15-second samples, from a whole hour three hours ago, then one sample now:
    // a 15-second series packs in one-hour blocks, and the first two hours are complete
    let now = SystemTime::now();
    let hour = 3600;
    let start_secs = (now.duration_since(UNIX_EPOCH).unwrap().as_secs() / hour) * hour - 3 * hour;
    let start = UNIX_EPOCH + Duration::from_secs(start_secs);
    let batch = || {
        let mut series = Series::gauge("revenue").label("site", "kazan");
        for i in 0..(3 * hour / 15) {
            series = series.sample(start + Duration::from_secs(15 * i), 1.0);
        }
        series.sample(now, 1.0)
    };
    stats.ingest(&[batch()])?;
    // what a collector's retry does after a crash between its ingest and keeping that it ingested: the same batch
    println!("the same batch ingested again, every sample identical: {:?}", stats.ingest(&[batch()]).map_err(|e| e.to_string()));

    let packed_at = start + Duration::from_secs(15 * 60); // inside the first, complete hour
    let head_at = start + Duration::from_secs(3 * hour - 60); // the last minutes, still in the head

    println!("before maintenance: at the packed time {:?}, at the head time {:?}", points(&stats, packed_at)?, points(&stats, head_at)?);
    let done = maintain(&store)?;
    println!("maintenance packed {} series into {} blocks", done.packed, done.blocks);

    let again = |at| Series::gauge("revenue").label("site", "kazan").sample(at, 2.0);
    let packed_answer = stats.ingest(&[again(packed_at)]);
    let head_answer = stats.ingest(&[again(head_at)]);
    println!("2.0 written again at the packed time: {packed_answer:?}; at the head time: {head_answer:?}");
    println!("read back: at the packed time {:?}, at the head time {:?}", points(&stats, packed_at)?, points(&stats, head_at)?);

    let done = maintain(&store)?;
    println!("maintenance again: packed {} series into {} blocks", done.packed, done.blocks);
    println!("read back: at the packed time {:?}, at the head time {:?}", points(&stats, packed_at)?, points(&stats, head_at)?);

    let sums = stats.query().name("revenue").from(start).to(start + Duration::from_secs(hour)).every(Duration::from_secs(hour)).aggregate(tinystore::metrics::Op::Sum)?;
    println!("sum over the packed hour, 240 samples of 1.0 with one of them written again as 2.0: {:?}", sums.iter().map(|a| format!("{:?}", a)).collect::<Vec<_>>());
    store.close()
}
