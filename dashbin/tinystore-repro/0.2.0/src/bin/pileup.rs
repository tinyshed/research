// the same refresh storm on the async api: dropping a future should stop its read
use std::time::{Duration, Instant};
use tinystore::Options;
use tinystore::aio::Store;

const SLOW: &str = "with recursive c(x) as (select 1 union all select x + 1 from c where x < 30000000) select count(*) from c";

#[tokio::main]
async fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-pileup-011");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let db = store.database("app").open()?;
    let started = Instant::now();
    let abandoned: Vec<_> = (0..36)
        .map(|_| {
            let read = db.scalar::<i64>(SLOW);
            tokio::spawn(async move {
                let result = read.await;
                println!("  a read ended at {:?}: {:?}", started.elapsed(), result.map_err(|e| e.kind()));
            })
        })
        .collect();
    tokio::time::sleep(Duration::from_millis(100)).await;
    for read in &abandoned {
        read.abort();
    }
    println!("36 reads abandoned at {:?}", started.elapsed());
    let t = Instant::now();
    match db.scalar::<i64>("select 1").await {
        Ok(one) => println!("the next visitor's `select 1` answered {one} after {:?}", t.elapsed()),
        Err(e) => println!("the next visitor's `select 1` failed after {:?}: {e}", t.elapsed()),
    }
    let t = Instant::now();
    let r = tokio::time::timeout(Duration::from_millis(200), db.scalar::<i64>(SLOW)).await;
    println!("a read under a 200 ms tokio timeout: {:?} after {:?}", r.is_err(), t.elapsed());
    tokio::time::sleep(Duration::from_secs(1)).await;
    let t = Instant::now();
    let n: i64 = db.scalar("select 1").await?;
    println!("one second later `select 1` = {n} in {:?}", t.elapsed());
    store.close().await
}
