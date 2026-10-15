mod channels;
mod indexes;
mod leases;

use serde_json::{Value, json};
use std::time::Duration;

fn percentile(v: &mut [u64], p: f64) -> u64 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    v[((v.len() - 1) as f64 * p).ceil() as usize]
}

fn memory() -> Value {
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    let read = |name| {
        status
            .lines()
            .find(|l| l.starts_with(name))
            .and_then(|l| l.split_whitespace().nth(1))
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    json!({"rss_kib": read("VmRSS:"), "peak_rss_kib": read("VmHWM:")})
}

fn cpu_seconds() -> f64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    unsafe {
        assert_eq!(libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()), 0);
        let u = usage.assume_init();
        (u.ru_utime.tv_sec + u.ru_stime.tv_sec) as f64
            + (u.ru_utime.tv_usec + u.ru_stime.tv_usec) as f64 / 1e6
    }
}

fn us(d: Duration) -> u64 {
    d.as_micros().min(u64::MAX as u128) as u64
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a[0].as_str() {
        "channels" => channels::run(&a[1..]),
        "route" => channels::route(&a[1..]),
        "channel-check" => channels::check(),
        "leases" => leases::run(&a[1..]),
        "lease-check" => leases::check(),
        "indexes" => indexes::run(&a[1..]),
        "commit-check" => leases::commit_check(),
        "lease-owner" => leases::crash_owner(&a[1]),
        "lease-recover" => leases::crash_recover(&a[1]),
        "lease-memory" => leases::waiter_memory(&a[1..]),
        _ => panic!("unknown experiment"),
    }
}
