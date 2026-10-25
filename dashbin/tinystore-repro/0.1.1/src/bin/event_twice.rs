// an event appended twice, as a job step that crashed after the append and before its answer was kept appends it:
// does Records keep one event or two, and is there an id that would make the second append nothing?
use std::time::{Duration, SystemTime};

use serde::Serialize;
use tinystore::{Options, Store};

#[derive(Serialize)]
struct Push {
    delivery: &'static str,
    repo: &'static str,
    sha: &'static str,
}

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-event-twice-011");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let deploys = store.events("deploys").open()?;

    // the same delivery, at the same time, with the same fields, twice
    let at = SystemTime::now() - Duration::from_secs(5);
    let push = Push { delivery: "72d3162e-cc78-11e3-81ab-4c9367dc0958", repo: "tinyshed/dashbin", sha: "26a0865" };
    deploys.event("push").at(at).append(&push)?;
    deploys.event("push").at(at).append(&push)?;

    let found: Vec<_> = deploys.list().since(Duration::from_secs(60)).all().collect::<tinystore::Result<_>>()?;
    println!("appended the same event twice, the stream holds {}", found.len());
    for event in &found {
        println!("  {} at {:?}: {:?}", event.name, event.at, event.fields.iter().collect::<Vec<_>>());
    }

    // the same, through append_all with both in one call
    deploys.append_all([tinystore::records::NewEvent::new("push").at(at), tinystore::records::NewEvent::new("push").at(at)])?;
    let after = deploys.list().since(Duration::from_secs(60)).all().count();
    println!("two more in one append_all, the stream holds {after}");
    store.close()
}
