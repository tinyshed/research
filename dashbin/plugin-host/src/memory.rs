// the memory round: what an install limit holds the process to, what real SDKs need, and a spike inside a call;
// each run is a process of its own, so the peak it reports is that run's
use std::time::{Duration, Instant};

use crate::calls::{Call, call, describe, discover_limited, first_line, host_of, instance_limited, settle, RANGE};
use crate::{purge, rss};

fn mib(bytes: i64) -> f64 {
    bytes as f64 / 1048576.0
}

fn line(what: &str, start: f64) {
    let (now, peak) = rss();
    println!("  {what:<22} VmRSS {now:>5.1} MiB  peak {peak:>5.1}  peak over the start {:>5.1}", peak - start);
}

fn typed(settings: &str) -> Vec<String> {
    settings.split("\"api\":\"").nth(1).and_then(|rest| rest.split('"').next()).map(host_of).into_iter().collect()
}

pub async fn install(bundle: &str, limit_mib: usize) {
    let (start, _) = rss();
    let started = Instant::now();
    let (found, held) = discover_limited(std::fs::read(bundle).unwrap(), limit_mib << 20).await;
    let took = started.elapsed();
    match &found {
        Ok(_) => println!("  declared in {took:.0?}, its JavaScript held {:.1} MiB", mib(held)),
        Err(e) => println!("  refused in {took:.0?}: {}", first_line(e)),
    }
    line("after", start);
    purge();
    line("after a purge", start);
}

pub async fn needs(bundle: &str, function: &str, settings: &str, limit_mib: usize) {
    tokio::task::LocalSet::new().run_until(needs_here(bundle, function, settings, limit_mib)).await
}

// the install and one call at the same limit; the call is tried even when the install was refused, to tell them apart
async fn needs_here(bundle: &str, function: &str, settings: &str, limit_mib: usize) {
    let (start, _) = rss();
    let bytes = std::fs::read(bundle).unwrap();
    let (found, held) = discover_limited(bytes.clone(), limit_mib << 20).await;
    match &found {
        Ok(_) => println!("  install: declared, its JavaScript held {:.1} MiB", mib(held)),
        Err(e) => println!("  install: refused: {}", first_line(e)),
    }
    match instance_limited(bytes, vec![], typed(settings), limit_mib << 20).await {
        Err(e) => println!("  instance: refused while loading: {}", first_line(&e)),
        Ok((instance, deadline)) => {
            let loaded = instance.runtime.memory_usage().await.malloc_size;
            let params = format!("{{{RANGE}}}");
            let (outcome, _) =
                call(&instance, &deadline, Call { function, params, settings: settings.into(), id: "needs", rows: 10, stop_after: None }).await;
            let after = instance.runtime.memory_usage().await.malloc_size;
            println!("  instance: loaded holding {:.1} MiB, {} , holding {:.1} MiB after", mib(loaded), describe(&outcome), mib(after));
            instance.unload().await;
        }
    }
    line("after", start);
}

pub async fn spike(bundle: &str, limit_mib: usize) {
    tokio::task::LocalSet::new().run_until(spike_here(bundle, limit_mib)).await
}

async fn spike_here(bundle: &str, limit_mib: usize) {
    let (start, _) = rss();
    let (instance, deadline) = instance_limited(std::fs::read(bundle).unwrap(), vec![], vec![], limit_mib << 20).await.unwrap();
    line("loaded", start);
    let one = |function| Call { function, params: "{}".into(), settings: "{}".into(), id: function, rows: 10, stop_after: None };
    let started = Instant::now();
    let (outcome, _) = call(&instance, &deadline, one("bomb")).await;
    println!("  bomb: {} in {:.0?}", describe(&outcome), started.elapsed());
    line("after the bomb", start);
    println!("  {}", settle(&instance).await);
    let (outcome, _) = call(&instance, &deadline, one("nothing")).await;
    println!("  the next call on the same runtime: {}", describe(&outcome));
    tokio::time::sleep(Duration::from_secs(2)).await;
    line("two seconds later", start);
    purge();
    line("after a purge", start);
    instance.unload().await;
}
