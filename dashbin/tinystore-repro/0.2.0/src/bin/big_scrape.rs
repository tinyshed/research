// one scrape of 100 000 series written whole, and how long a small ingest beside it waits
use std::fmt::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

use tinystore::metrics::Series;
use tinystore::{Options, Store};

fn scrape(samples: usize) -> String {
    let mut text = String::from("# TYPE kube_pod_status_ready gauge\n");
    for pod in 0..samples {
        writeln!(text, "kube_pod_status_ready{{pod=\"pod-{pod}\",condition=\"true\"}} 1").unwrap();
    }
    text
}

fn main() -> tinystore::Result<()> {
    let dir = std::env::temp_dir().join("big-scrape-repro");
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir, Options { max_series: 1_000_000, ..Options::default() })?;
    let stats = store.metrics()?;
    let text = scrape(100_000);
    println!("a text of 100000 samples, {} bytes", text.len());

    for pass in ["first scrape, every series new", "second scrape, the same series"] {
        let done = AtomicBool::new(false);
        let started = Instant::now();
        std::thread::scope(|scope| {
            // a collector of another source writes one sample every 10 ms for as long as the big write runs
            let beside = scope.spawn(|| {
                let (mut longest, mut at, mut writes) = (Duration::ZERO, Duration::ZERO, 0);
                while !done.load(Ordering::Relaxed) {
                    let asked = Instant::now();
                    stats.ingest(&[Series::gauge("orders").label("shop", "msk").sample(SystemTime::now(), 1.0)]).unwrap();
                    if asked.elapsed() > longest {
                        (longest, at) = (asked.elapsed(), asked - started);
                    }
                    writes += 1;
                    std::thread::sleep(Duration::from_millis(10));
                }
                (longest, at, writes)
            });
            let answer = stats.prometheus().label("instance", "ksm").ingest(&text);
            let took = started.elapsed();
            done.store(true, Ordering::Relaxed);
            let (longest, at, writes) = beside.join().unwrap();
            println!(
                "{pass}: {:?} in {:.2} s; beside it {writes} small writes, the longest {:.2} s, asked {:.2} s in",
                answer.map_err(|e| e.to_string()),
                took.as_secs_f64(),
                longest.as_secs_f64(),
                at.as_secs_f64()
            );
        });
        std::thread::sleep(Duration::from_millis(1100));
    }

    let over = scrape(100_001);
    let answer = stats.prometheus().label("instance", "ksm-2").ingest(&over);
    println!("100001 samples: {:?}", answer.map_err(|e| e.to_string()));
    store.close()
}
