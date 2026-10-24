// four callers of one key: with a small answer the work runs once, with a big one it runs four times in a row
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tinystore::{Options, Store};

fn race(store: &Store, name: &str, answer_bytes: usize) -> tinystore::Result<()> {
    let once = store.once::<String>(name).keep(Duration::from_secs(5)).open()?;
    let runs = Arc::new(AtomicUsize::new(0));
    let started = Instant::now();
    let callers: Vec<_> = (0..4)
        .map(|i| {
            let once = once.clone();
            let runs = Arc::clone(&runs);
            thread::spawn(move || {
                let answer = once
                    .run("panel-7", || {
                        runs.fetch_add(1, Ordering::SeqCst);
                        thread::sleep(Duration::from_millis(500)); // the query to someone's postgres
                        Ok::<_, tinystore::Error>("x".repeat(answer_bytes))
                    })
                    .unwrap();
                println!("  caller {i} got {} bytes at {:?}", answer.len(), started.elapsed());
            })
        })
        .collect();
    for c in callers {
        c.join().unwrap();
    }
    println!("{name}: answer of {answer_bytes} bytes, work ran {} times, took {:?}", runs.load(Ordering::SeqCst), started.elapsed());
    Ok(())
}

fn main() -> tinystore::Result<()> {
    tracing_subscriber::fmt().with_target(true).init();
    let dir = std::env::temp_dir().join("ts-repro-once");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    race(&store, "small", 100 * 1024)?;
    race(&store, "big", 2 * 1024 * 1024)?;
    store.close()
}
