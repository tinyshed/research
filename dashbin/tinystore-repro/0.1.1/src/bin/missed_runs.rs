// a job repeating every second, while nothing runs it for five seconds, as a hub that was down:
// when a worker starts again, does the job run once, or once for each time it missed?
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use tinystore::{Options, Store};

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("ts-repro-missed-runs-011");
    let _ = std::fs::remove_dir_all(&dir);

    {
        let store = Store::open(&dir, Options::default())?;
        let flows = store.queue::<String>("flows").open()?;
        flows.id("flow-1").every(Duration::from_secs(1)).add(&"collect".to_string())?;
        println!("added flow-1, every second; closing the store for five seconds");
        store.close()?;
    }
    std::thread::sleep(Duration::from_secs(5));

    let store = Store::open(&dir, Options::default())?;
    let flows = store.queue::<String>("flows").open()?;
    let runs: Arc<Mutex<Vec<(Duration, Duration, u32)>>> = Arc::default();
    let started = Instant::now();
    let seen = runs.clone();
    let worker = flows.work(move |_value: String, run| {
        let late = SystemTime::now().duration_since(run.at()).unwrap_or_default();
        seen.lock().unwrap().push((started.elapsed(), late, run.attempt()));
        Ok::<(), std::io::Error>(())
    })?;
    std::thread::sleep(Duration::from_millis(3500));
    worker.stop();

    let runs = runs.lock().unwrap();
    println!("reopened and worked for 3.5 s: {} runs", runs.len());
    for (when, late, attempt) in runs.iter() {
        println!("  at {:>6.0?} after reopening, due {:>6.0?} before it ran, attempt {attempt}", when, late);
    }
    drop(runs);
    store.close()
}
