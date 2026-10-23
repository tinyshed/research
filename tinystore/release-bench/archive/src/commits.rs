//! Lone durable commits: one kv set at a time, each synced before the next,
//! as a quiet application writes. Built twice, with and without SQLite's
//! HAVE_FDATASYNC, it says what a commit costs each way on this disk.

use std::time::Instant;

use serde_json::json;
use tinystore::{Options, Store};

use crate::out::{Args, Fallible, result};

pub fn run(args: &Args) -> Fallible<()> {
    let store = Store::open(args.get("dir")?, Options::default())?;
    let sessions = store.bucket::<String>("sessions").open()?;
    let count: usize = args.or("count", "2000").parse()?;
    let value = "a session of a user".to_string();
    for n in 0..100 {
        sessions.set(format!("s{n}"), &value)?;
    }
    let mut times = Vec::with_capacity(count);
    let began = Instant::now();
    for n in 0..count {
        let before = Instant::now();
        sessions.set(format!("s{}", n % 1000), &value)?;
        times.push(before.elapsed());
    }
    let elapsed = began.elapsed();
    times.sort();
    let at = |rank: f64| times[((times.len() - 1) as f64 * rank) as usize].as_secs_f64() * 1e6;
    result(json!({
        "commits": count, "perSecond": count as f64 / elapsed.as_secs_f64(),
        "p50us": at(0.5), "p99us": at(0.99), "meanUs": (elapsed / count as u32).as_secs_f64() * 1e6,
    }));
    drop(sessions);
    store.close()?;
    Ok(())
}
