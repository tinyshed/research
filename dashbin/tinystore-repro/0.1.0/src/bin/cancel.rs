// a read that the caller gave up on keeps running: there is no way to stop it
use std::time::{Duration, Instant};
use tinystore::{Options, Store};

#[tokio::main]
async fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-cancel");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let db = store.database("app").open()?;

    let started = Instant::now();
    let read = {
        let db = db.clone();
        tokio::task::spawn_blocking(move || {
            // ~a few seconds of pure cpu inside sqlite
            let n: i64 = db
                .scalar("with recursive c(x) as (select 1 union all select x + 1 from c where x < 30000000) select count(*) from c")
                .unwrap();
            println!("read finished after {:?} (n = {n}), nobody was waiting for it", started.elapsed());
        })
    };

    // the browser tab closed after 100 ms
    tokio::time::sleep(Duration::from_millis(100)).await;
    read.abort();
    println!("caller gave up after {:?}", started.elapsed());

    // a second read on the same db while the first one still burns a reader
    let t = Instant::now();
    let one: i64 = tokio::task::spawn_blocking({
        let db = db.clone();
        move || db.scalar("select 1").unwrap()
    })
    .await
    .unwrap();
    println!("unrelated read answered {one} in {:?}", t.elapsed());

    tokio::time::sleep(Duration::from_secs(15)).await;
    store.close()
}
