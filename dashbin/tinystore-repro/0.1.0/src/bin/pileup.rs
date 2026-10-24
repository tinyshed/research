// someone hits refresh on a 12-panel dashboard three times: the abandoned reads hold every reader
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
