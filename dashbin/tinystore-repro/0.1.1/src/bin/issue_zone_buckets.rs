use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tinystore::metrics::{Op, Series};
use tinystore::records::Aggregate;
use tinystore::{Options, Store};

#[derive(serde::Serialize)]
struct Order {
    shop: &'static str,
}

// a time as utc text, so the output needs no crate
fn utc(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y}-{m:02}-{d:02}T{:02}:{:02}Z", rest / 3600, rest % 3600 / 60)
}

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("zone-buckets-repro");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;

    // a shop in moscow (utc+3) takes one order an hour for two moscow days, from moscow midnight
    let today = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() / 86_400 * 86_400;
    let moscow_midnight = UNIX_EPOCH + Duration::from_secs(today - 3 * 86_400 - 3 * 3600);
    let hours = (0..48).map(|h| moscow_midnight + Duration::from_secs(3600 * h));

    let stats = store.metrics()?;
    let mut orders = Series::gauge("orders").label("shop", "msk");
    for at in hours.clone() {
        orders = orders.sample(at, 1.0);
    }
    stats.ingest(&[orders])?;
    let days = stats
        .query()
        .name("orders")
        .from(moscow_midnight)
        .to(moscow_midnight + Duration::from_secs(2 * 86_400))
        .every(Duration::from_secs(86_400))
        .aggregate(Op::Sum)?;
    println!("metrics, orders a day over two moscow days, 24 each:");
    for bucket in &days[0].buckets {
        println!("  bucket at {}: {:?}", utc(bucket.at), bucket.value);
    }

    let events = store.events("orders").open()?;
    for at in hours {
        events.event("order").at(at).append(&Order { shop: "msk" })?;
    }
    let rows = events
        .list()
        .from(moscow_midnight)
        .to(moscow_midnight + Duration::from_secs(2 * 86_400))
        .aggregate(&Aggregate::new().every(Duration::from_secs(86_400)))?;
    println!("records, the same orders as events, counted a day:");
    for row in &rows {
        println!("  row at {}: {}", utc(row.at), row.count);
    }
    store.close()
}
