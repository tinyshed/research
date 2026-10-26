// one scrape of 100 000 series written whole, and how long a small ingest beside it waits
use std::fmt::Write;
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
        std::thread::scope(|scope| {
            let started = Instant::now();
            // a collector of another source writes one sample 100 ms into the big write
            let beside = scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(100));
                let asked = Instant::now();
                let answer = stats.ingest(&[Series::gauge("orders").label("shop", "msk").sample(SystemTime::now(), 1.0)]);
                (asked.elapsed(), answer.map_err(|e| e.to_string()))
            });
            let answer = stats.prometheus().label("instance", "ksm").ingest(&text);
            let took = started.elapsed();
            let (waited, small) = beside.join().unwrap();
            println!("{pass}: {:?} in {:.2} s; one sample beside it {small:?} in {:.2} s", answer.map_err(|e| e.to_string()), took.as_secs_f64(), waited.as_secs_f64());
        });
        std::thread::sleep(Duration::from_millis(1100));
    }

    let over = scrape(100_001);
    let answer = stats.prometheus().label("instance", "ksm-2").ingest(&over);
    println!("100001 samples: {:?}", answer.map_err(|e| e.to_string()));
    store.close()
}
