use std::time::{Duration, SystemTime};

use serde::Serialize;
use tinystore::records::NewEvent;
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
    // and the next attempt appends it again, now with the delivery as the event's id
    let at = SystemTime::now();
    let push = Push { delivery: "72d3162e-cc78-11e3-81ab-4c9367dc0958", sha: "26a0865" };
    let first = deploys.event("push").id(push.delivery).at(at).append(&push)?;
    let second = deploys.event("push").id(push.delivery).at(at).append(&push)?;
    let kept = deploys.list().since(Duration::from_secs(60)).all().count();
    println!("one delivery appended by two attempts with its id: wrote {first}, then {second}; the stream holds {kept} events");

    // a list holding that delivery again and a new one
    let again = NewEvent::new("push").id(push.delivery).at(at).fields(&push);
    let next = Push { delivery: "9a1f0c3e-cc78-11e3-81ab-4c9367dc0958", sha: "76b7915" };
    let new = NewEvent::new("push").id(next.delivery).at(at).fields(&next);
    let wrote = deploys.append_all([again, new])?;
    let kept = deploys.list().since(Duration::from_secs(60)).all().count();
    println!("a list of that delivery and a new one: wrote {wrote}; the stream holds {kept} events");

    // without an id an append is what it was
    deploys.event("push").at(at).append(&push)?;
    deploys.event("push").at(at).append(&push)?;
    let kept = deploys.list().since(Duration::from_secs(60)).all().count();
    println!("the same push twice without an id: the stream holds {kept} events");
    store.close()
}
