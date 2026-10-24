// 8 threads reading one table at once: how long it takes depends on build flags the app has to know about
use std::thread;
use std::time::Instant;
use tinystore::{Options, Store};

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-readers");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;
    let db = store.database("app").open()?;
    db.exec("create table t(x integer primary key, v real)")?;
    db.exec("insert into t select value, value * 0.5 from generate_series(1, 200000)")
        .or_else(|_| db.exec("with recursive c(x) as (select 1 union all select x + 1 from c where x < 200000) insert into t select x, x * 0.5 from c"))?;
    for threads in [1, 8] {
        let started = Instant::now();
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                let db = db.clone();
                thread::spawn(move || {
                    for _ in 0..400 {
                        let _: f64 = db.scalar("select sum(v) from t where x % 7 = 3").unwrap();
                    }
                })
            })
            .collect();
        for w in workers {
            w.join().unwrap();
        }
        let took = started.elapsed();
        println!("{threads} thread(s) x 400 reads: {took:?}, {:.0} reads/s", (threads * 400) as f64 / took.as_secs_f64());
    }
    store.close()
}
