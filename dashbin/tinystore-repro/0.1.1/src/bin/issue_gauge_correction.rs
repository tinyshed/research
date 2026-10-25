use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tinystore::metrics::{Series, maintain};
use tinystore::{Options, Store};

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("gauge-correction-repro");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let stats = store.metrics()?;

    // a shop's revenue, kept once a day by a collector that reads it from the shop's database
    let today = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() / 86_400 * 86_400;
    let day = |n: u64| UNIX_EPOCH + Duration::from_secs(today - n * 86_400);
    let mut revenue = Series::gauge("revenue").label("shop", "msk");
    for n in (0..=10).rev() {
        revenue = revenue.sample(day(n), 1000.0);
    }
    stats.ingest(&[revenue])?;
    maintain(&store)?;

    // a refund lands for yesterday: the collector reads yesterday again and writes what it is now
    let corrected = Series::gauge("revenue").label("shop", "msk").sample(day(1), 900.0);
    println!("yesterday's revenue corrected from 1000 to 900: {:?}", stats.ingest(&[corrected]).map_err(|e| e.to_string()));
    let read = stats.query().name("revenue").from(day(1)).to(day(1) + Duration::from_secs(1)).read()?;
    println!("yesterday now reads {:?}", read.iter().flat_map(|s| s.samples().iter().map(|x| x.value)).collect::<Vec<_>>());
    store.close()
}
