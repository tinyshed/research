// a store's streams, as a program that draws a store it has never seen lists them: which fields do their events carry?

use serde::Serialize;
use tinystore::{Options, Store};

#[derive(Serialize)]
struct Order {
    order_id: i64,
    amount: f64,
    currency: &'static str,
    paid: bool,
}

#[derive(Serialize)]
struct Deploy {
    service: &'static str,
    version: &'static str,
    took_ms: i64,
}

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-051-fields");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let orders = store.events("orders").open()?;
    let deploys = store.events("deploys").open()?;
    for order_id in 0..100 {
        orders.append("order.paid", &Order { order_id, amount: 12.5, currency: "EUR", paid: order_id % 3 != 0 })?;
    }
    deploys.append("deploy.done", &Deploy { service: "api", version: "1.4.2", took_ms: 5200 })?;
    for stream in store.all_events()?.streams()? {
        println!("{stream:?}");
    }
    Ok(())
}
