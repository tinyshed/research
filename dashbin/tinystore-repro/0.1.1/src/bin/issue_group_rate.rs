use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tinystore::jobs::Concurrency;
use tinystore::{Options, Store};

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("group-rate-repro");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options::default())?;

    // telegram lets a bot send about 30 messages a second; here 2 a second, so it shows in a few seconds.
    // two bots, two groups, four messages each
    let messages = store.queue::<String>("messages").rate(2, Duration::from_secs(1)).concurrency(Concurrency::total(8)).open()?;
    for n in 0..4 {
        messages.id(format!("a-{n}")).group("bot-a").add(&format!("bot-a message {n}"))?;
        messages.id(format!("b-{n}")).group("bot-b").add(&format!("bot-b message {n}"))?;
    }

    let started = Instant::now();
    let sent: Arc<Mutex<Vec<(Duration, String)>>> = Arc::default();
    let log = sent.clone();
    let worker = messages.work(move |message: String, _run| {
        log.lock().unwrap().push((started.elapsed(), message));
        Ok::<(), std::io::Error>(())
    })?;
    std::thread::sleep(Duration::from_millis(4500));
    worker.stop();

    for (at, message) in sent.lock().unwrap().iter() {
        println!("  {:>5.1} s  {message}", at.as_secs_f64());
    }
    println!("each bot may send 2 a second at telegram; the queue's one rate gave them 1 a second each, 4 s for what took 2");
    store.close()
}
