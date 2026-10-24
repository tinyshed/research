// 0.1.1: a scrape body goes straight into the store
use tinystore::{Options, Store};

const BODY: &str = r#"# HELP node_cpu_seconds_total Seconds the CPUs spent in each mode.
# TYPE node_cpu_seconds_total counter
node_cpu_seconds_total{cpu="0",mode="idle"} 1.23456789e+06
node_cpu_seconds_total{cpu="0",mode="user"} 4567.89
# TYPE node_load1 gauge
node_load1 0.42
# TYPE http_request_duration_seconds histogram
http_request_duration_seconds_bucket{le="0.1"} 240
http_request_duration_seconds_bucket{le="0.5"} 310
http_request_duration_seconds_bucket{le="+Inf"} 320
http_request_duration_seconds_sum 41.7
http_request_duration_seconds_count 320
"#;

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-prom-011");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let stats = store.metrics()?;
    stats.prometheus().label("instance", "site-3").ingest(BODY)?;
    match stats.prometheus().label("instance", "site-3").ingest("node_load1{instance=\"x\"} 1\nbroken line here\n") {
        Ok(()) => println!("broken text: accepted?!"),
        Err(e) => println!("broken text: {e}"),
    }
    stats.flush()?;
    println!("{:#?}", stats.names().page()?);
    store.close()
}
