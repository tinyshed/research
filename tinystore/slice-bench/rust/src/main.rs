//! The Rust core under the slice round's load: one case a process, a store of
//! its own, the same keys, rows and jobs as the Go program beside it.
//!
//! `slice-bench --dir <dir> --case <case> --callers <n> --seconds <s>` prints
//! one line of JSON: the calls made, the time they took and their latencies.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tinystore::kv::Bytes;
use tinystore::{Options, Result, Store, sql};

const KEYS: u64 = 100_000;
const VALUE_BYTES: usize = 128;
const NOTES: i64 = 100_000;
const ROWS: i64 = 25_000;
const DRAINED: u64 = 20_000;
const DRAIN_WORKERS: u32 = 8;

const SCHEMA: &str =
    "create table note (id integer primary key, title text not null, body text not null) strict";
const INSERT: &str = "insert into note (id, title, body) values (?, ?, ?)";
const POINT: &str = "select id, title, body from note where id = ?";
const PAGE: &str = "select id, title, body from note order by id limit 25000";

#[derive(Serialize, Deserialize)]
struct Note {
    id: i64,
    title: String,
    body: String,
}

#[derive(Serialize, Deserialize)]
struct Job {
    n: i64,
    text: String,
}

/// What one case measured.
struct Measured {
    ops: u64,
    elapsed: Duration,
    /// Each call's time in nanoseconds, sorted.
    latencies: Vec<u32>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| {
        let at = args.iter().position(|arg| arg == name).unwrap_or_else(|| panic!("missing {name}"));
        args[at + 1].clone()
    };
    let (dir, case) = (arg("--dir"), arg("--case"));
    let callers: usize = arg("--callers").parse().unwrap();
    let seconds: f64 = arg("--seconds").parse().unwrap();
    let store = Store::open(&dir, Options::default()).unwrap();
    let measured = run(&store, &case, callers, Duration::from_secs_f64(seconds)).unwrap();
    store.close().unwrap();
    let at = |quantile: f64| {
        let index = ((measured.latencies.len() as f64 - 1.0) * quantile).round() as usize;
        measured.latencies.get(index).copied().unwrap_or(0)
    };
    println!(
        "{}",
        serde_json::json!({
            "engine": "rust", "case": case, "callers": callers, "ops": measured.ops,
            "elapsed_ns": measured.elapsed.as_nanos() as u64,
            "per_second": measured.ops as f64 / measured.elapsed.as_secs_f64(),
            "p50_ns": at(0.50), "p99_ns": at(0.99),
        })
    );
}

fn run(store: &Store, case: &str, callers: usize, time: Duration) -> Result<Measured> {
    match case {
        "kv-get" => {
            let bucket = store.bucket::<Bytes>("sessions").open()?;
            fill(64, KEYS, |n| bucket.set(key(n), &value(n)))?;
            timed(callers, time, |caller, call| {
                let n = pick(caller, call) % KEYS;
                let found = bucket.get(key(n))?.expect("a key the fill wrote");
                assert_eq!(found.0.len(), VALUE_BYTES);
                Ok(())
            })
        }
        "kv-set" => {
            let bucket = store.bucket::<Bytes>("sessions").open()?;
            timed(callers, time, |caller, call| bucket.set(format!("s{caller:02}-{call:010}"), &value(call)))
        }
        "sql-point" => {
            let db = notes(store, NOTES)?;
            timed(callers, time, |caller, call| {
                let id = (pick(caller, call) % NOTES as u64) as i64 + 1;
                let note: Note = db.one(sql!(POINT, id))?.expect("a note the fill wrote");
                assert_eq!(note.id, id);
                Ok(())
            })
        }
        "sql-insert" => {
            let db = notes(store, 0)?;
            timed(callers, time, |caller, call| {
                let id = (caller as i64 + 1) * 1_000_000_000 + call as i64;
                db.exec(sql!(INSERT, id, title(id), body(id))).map(drop)
            })
        }
        "sql-rows" => {
            let db = notes(store, ROWS)?;
            timed(1, time, |_, _| {
                let rows: Vec<Note> = db.all(sql!(PAGE))?;
                assert_eq!(rows.len() as i64, ROWS);
                Ok(())
            })
        }
        "jobs-add" => {
            let queue = store.queue::<Job>("mail").open()?;
            timed(callers, time, |_, call| queue.add(&job(call)).map(drop))
        }
        "jobs-drain" => {
            let queue = store.queue::<Job>("mail").open()?;
            fill(64, DRAINED, |n| queue.add(&job(n)).map(drop))?;
            let started = Instant::now();
            let ran = queue.concurrency(DRAIN_WORKERS).run_due(|_: Job, _run| -> Result<()> { Ok(()) })?;
            Ok(Measured { ops: ran as u64, elapsed: started.elapsed(), latencies: Vec::new() })
        }
        other => panic!("no case {other}"),
    }
}

/// A database of `count` notes, written a thousand a transaction.
fn notes(store: &Store, count: i64) -> Result<tinystore::sql::Database> {
    let db = store.database("app").migrations([("0001_notes.sql", SCHEMA)]).open()?;
    for from in (1..=count).step_by(1000) {
        db.tx(|tx| -> Result<()> {
            for id in from..(from + 1000).min(count + 1) {
                tx.exec(sql!(INSERT, id, title(id), body(id)))?;
            }
            Ok(())
        })?;
    }
    Ok(db)
}

fn key(n: u64) -> String {
    format!("k{n:08}")
}

fn value(n: u64) -> Bytes {
    Bytes((0..VALUE_BYTES).map(|at| (n as usize + at) as u8).collect())
}

fn title(id: i64) -> String {
    format!("note {id:012}")
}

fn body(id: i64) -> String {
    let word = format!("{id:010} ");
    word.repeat(10)[..100].to_owned()
}

fn job(n: u64) -> Job {
    Job { n: n as i64, text: "send the weekly digest".to_owned() }
}

/// The `call`th choice of `caller`: a xorshift of its own, the same in Go and
/// in Bun, so that every program reads the same keys in the same order.
fn pick(caller: usize, call: u64) -> u64 {
    let mut x = (caller as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ call.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Makes `count` calls from `callers` threads, untimed.
fn fill(callers: u64, count: u64, call: impl Fn(u64) -> Result<()> + Sync) -> Result<()> {
    let next = AtomicU64::new(0);
    std::thread::scope(|scope| {
        let workers: Vec<_> = (0..callers)
            .map(|_| {
                scope.spawn(|| -> Result<()> {
                    loop {
                        let n = next.fetch_add(1, Ordering::Relaxed);
                        if n >= count {
                            return Ok(());
                        }
                        call(n)?;
                    }
                })
            })
            .collect();
        workers.into_iter().try_for_each(|worker| worker.join().unwrap())
    })
}

/// Calls `call` from `callers` threads for `time`, each call timed.
fn timed(callers: usize, time: Duration, call: impl Fn(usize, u64) -> Result<()> + Sync) -> Result<Measured> {
    let (start, stop) = (Arc::new(Barrier::new(callers + 1)), AtomicBool::new(false));
    let (elapsed, mut latencies) = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..callers)
            .map(|caller| {
                let (start, stop, call) = (Arc::clone(&start), &stop, &call);
                scope.spawn(move || -> Result<Vec<u32>> {
                    let mut latencies = Vec::with_capacity(1 << 16);
                    start.wait();
                    let mut made = 0u64;
                    while !stop.load(Ordering::Relaxed) {
                        let began = Instant::now();
                        call(caller, made)?;
                        latencies.push(u32::try_from(began.elapsed().as_nanos()).unwrap_or(u32::MAX));
                        made += 1;
                    }
                    Ok(latencies)
                })
            })
            .collect();
        start.wait();
        let began = Instant::now();
        std::thread::sleep(time);
        stop.store(true, Ordering::Relaxed);
        let mut all = Vec::new();
        for worker in workers {
            all.extend(worker.join().unwrap().unwrap());
        }
        (began.elapsed(), all)
    });
    latencies.sort_unstable();
    Ok(Measured { ops: latencies.len() as u64, elapsed, latencies })
}
