the docs (guides/kv/once) say once with a short keep is "a memo that many callers share: the dashboard's panels are computed once for everyone who asks at once". tried exactly that for dashbin panels.

works great while the answer is small. but a panel answer is easily over 1 MiB (a table panel with ~50k rows). once keeps its answers in kv.db, so a value over 1 MiB can't be kept: you get a warning, and then the waiters wake up, find nothing, and each of them runs the work itself, **one after another**. 4 callers = 4 runs in a row, 4x slower than not using once at all.

## repro

tinystore 0.1.0 from crates.io, rust 1.97.1, windows 11.

```rust
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
```

output:

```
  caller 0 got 102400 bytes at 502.7665ms
  caller 3 got 102400 bytes at 503.1335ms
  caller 1 got 102400 bytes at 503.1921ms
  caller 2 got 102400 bytes at 503.2189ms
small: answer of 102400 bytes, work ran 1 times, took 503.2934ms
WARN tinystore: a once answer was not kept; the next run of its key runs again error=kv once big: key "panel-7": a value of 2097153 bytes, over 1048576: limit
  caller 0 got 2097152 bytes at 501.2473ms
WARN tinystore: a once answer was not kept; ...
  caller 2 got 2097152 bytes at 1.002301s
WARN tinystore: a once answer was not kept; ...
  caller 1 got 2097152 bytes at 1.5032294s
WARN tinystore: a once answer was not kept; ...
  caller 3 got 2097152 bytes at 2.0040007s
big: answer of 2097152 bytes, work ran 4 times, took 2.0041428s
```

## more stuff that makes it a bad fit for panels, even with small answers

- every kept answer is a real write to kv.db (full durability, so a sync) just to share it for a few seconds
- a waiter can't give up: it sits on `recv()` with no timeout, so a closed tab keeps a thread parked until somebody else's query ends
- `work` is a sync closure and a panel query is async (pg / mysql driver), so it ends up async inside `spawn_blocking` inside async

## what i'd like

1. **bug:** when an answer can't be kept, hand it to the callers who already waited for that run instead of letting them re-run one by one. they waited for *this* run, they should get its answer.
2. **docs:** drop the dashboard line, or say the limits next to it (1 MiB, every answer is a disk write, waiters can't leave).

honestly i think collapsing dashboard requests belongs in dashbin, in memory, not in the store. once is great at its real job (idempotency keys). the docs just pitch it for something it isn't.
