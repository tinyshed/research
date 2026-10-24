writing dashbin on top of tinystore (rust, embedded, axum + tokio) and hit this pretty quick.

the rust api is all blocking. that's ok by itself, under tokio i just wrap every call in `spawn_blocking`. the problem is there's no way to **stop** a call once it started. if the http request goes away (tab closed, user hit refresh) the read keeps running to the end and holds a reader the whole time. `abort()` on the tokio handle does nothing to the thread, and tinystore has nothing like a cancel handle / deadline / interrupt for reads (grepped the crate, no `sqlite3_interrupt` anywhere).

why it hurts for a dashboard: 12 panels on a page, someone hits refresh 3 times -> 36 reads, 24 of them nobody wants anymore. they take all 16 readers for a while, and the next visitor waits 10s (reader patience) and gets `unavailable`.

## repro

tinystore 0.1.0 from crates.io, rust 1.97.1, windows 11, 16 threads.

```toml
tinystore = { version = "=0.1.0", default-features = false, features = ["kv", "metrics", "records", "sql"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }
```

the slow query is a dumb recursive cte, it stands in for a heavy aggregate or some sql a user typed.

```rust
use std::time::{Duration, Instant};
use tinystore::{Options, Store};

const SLOW: &str = "with recursive c(x) as (select 1 union all select x + 1 from c where x < 30000000) select count(*) from c";

#[tokio::main]
async fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-pileup");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let db = store.database("app").open()?;
    let started = Instant::now();
    let mut abandoned = Vec::new();
    for _ in 0..36 {
        let db = db.clone();
        abandoned.push(tokio::task::spawn_blocking(move || {
            let result = db.scalar::<i64>(SLOW);
            println!("  an abandoned read ended at {:?}: {}", started.elapsed(), if result.is_ok() { "ok".to_string() } else { result.unwrap_err().to_string() });
        }));
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    for read in &abandoned {
        read.abort();
    }
    println!("36 reads abandoned at {:?}", started.elapsed());
    let t = Instant::now();
    let answer = tokio::task::spawn_blocking({
        let db = db.clone();
        move || db.scalar::<i64>("select 1")
    })
    .await
    .unwrap();
    match answer {
        Ok(one) => println!("the next visitor's `select 1` answered {one} after {:?}", t.elapsed()),
        Err(e) => println!("the next visitor's `select 1` failed after {:?}: {e}", t.elapsed()),
    }
    tokio::time::sleep(Duration::from_secs(30)).await;
    store.close()
}
```

output (trimmed, paths shortened):

```
36 reads abandoned at 115.9591ms
  an abandoned read ended at 6.3337389s: ok
  ... 16 of these around 6.3-6.7s
  an abandoned read ended at 10.0156143s: sql app: statement "with recursive c(x) as (select 1 union all select x + 1 from…": <tmp>\sql/app.db: all 16 readers are busy: unavailable
  ... 4 of these
the next visitor's `select 1` failed after 10.0431655s: sql app: statement "select 1": <tmp>\sql/app.db: all 16 readers are busy: unavailable
  an abandoned read ended at 12.7613945s: ok
  ... 16 more around 12.8-13.1s
```

so 32 reads ran to the end for nobody, 4 timed out, and an unrelated `select 1` from the next request waited 10s and failed.

same thing with a single read: caller gave up at 107ms, the read still finished at 3.07s.

## what i'd like

- a way to stop a call from outside. could be a cancel handle or a deadline you pass into the read, which calls `sqlite3_interrupt` on that reader and gives the reader back. also the natural place for a per-call timeout.
- even nicer: futures you can await, and dropping one = cancel. the core already does "send, get woken when ready" for the other sdks (`tinystore_send` / `recv` / `wake`), so the rust side might not be far from it. not asking for tokio as a dependency, a runtime-agnostic future is enough.

reads of the store's own small stuff are fast and don't care. it's anything that can be slow that needs this: an aggregate over a long range, a gauge percentile (decodes samples), sql a user typed.
