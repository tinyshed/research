use std::time::{Duration, SystemTime};

use serde::Serialize;
use tinystore::{Options, Store};

#[derive(Serialize)]
struct Push {
    delivery: &'static str,
    sha: &'static str,
}

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("event-id-repro");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let deploys = store.events("deploys").open()?;

    // a job step appends the delivery, the worker dies before the step's answer is kept,
    // and the next attempt appends it again
    let at = SystemTime::now();
    let push = Push { delivery: "72d3162e-cc78-11e3-81ab-4c9367dc0958", sha: "26a0865" };
    deploys.event("push").at(at).append(&push)?;
    deploys.event("push").at(at).append(&push)?;

    let kept = deploys.list().since(Duration::from_secs(60)).all().count();
    println!("one delivery appended by two attempts: the stream holds {kept} events");
    store.close()
}
