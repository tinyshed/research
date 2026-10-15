use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

pub fn path() -> String {
    std::env::var("BENCH_DB").expect("BENCH_DB on disk-backed scratch")
}
pub fn open(path: &str) -> Connection {
    let c = Connection::open(path).unwrap();
    c.execute_batch(
        "pragma journal_mode=wal; pragma synchronous=FULL; pragma cache_size=-4096;
        pragma wal_autocheckpoint=0; pragma busy_timeout=5000; pragma foreign_keys=on;",
    )
    .unwrap();
    c
}
pub fn memory() -> Value {
    let text = std::fs::read_to_string("/proc/self/status").unwrap();
    let get = |key: &str| {
        text.lines()
            .find(|l| l.starts_with(key))
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse::<u64>()
            .unwrap()
    };
    json!({"rss_kib":get("VmRSS:"),"hwm_kib":get("VmHWM:")})
}
pub fn cpu() -> f64 {
    let mut r = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // getrusage writes its complete result after a successful call.
    unsafe {
        assert_eq!(libc::getrusage(libc::RUSAGE_SELF, r.as_mut_ptr()), 0);
        let r = r.assume_init();
        (r.ru_utime.tv_sec + r.ru_stime.tv_sec) as f64
            + (r.ru_utime.tv_usec + r.ru_stime.tv_usec) as f64 / 1e6
    }
}
pub fn us(start: Instant) -> u64 {
    start.elapsed().as_micros() as u64
}
pub fn distribution(values: &mut [u64]) -> Value {
    values.sort_unstable();
    let n = values.len();
    let p = |q: f64| {
        if n == 0 {
            0
        } else {
            values[((n - 1) as f64 * q).ceil() as usize]
        }
    };
    json!({"n":n,"p50_us":p(0.5),"p95_us":p(0.95),"p99_us":p(0.99),"max_us":p(1.)})
}
pub fn objects(c: &Connection) -> Vec<Value> {
    c.prepare("select name,sum(pgsize) from dbstat group by name order by name")
        .unwrap()
        .query_map([], |r| {
            Ok(json!({"object":r.get::<_,String>(0)?,"bytes":r.get::<_,i64>(1)?}))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}
pub fn bytes(path: &str) -> u64 {
    std::fs::metadata(path).map(|v| v.len()).unwrap_or(0)
}
pub fn wal_bytes(path: &str) -> u64 {
    bytes(&format!("{path}-wal"))
}
static WAL_SALT: AtomicU64 = AtomicU64::new(0);
pub static WAL_LAST: AtomicU64 = AtomicU64::new(0);
pub static WAL_FRAMES: AtomicU64 = AtomicU64::new(0);
pub static WAL_COMMIT_FRAMES: AtomicU64 = AtomicU64::new(0);
pub static WAL_COMMITS: AtomicU64 = AtomicU64::new(0);
pub static WAL_PEAK: AtomicU64 = AtomicU64::new(0);
pub fn observe_wal(c: &Connection) {
    c.wal_hook(Some(|_, frames| {
        let now = frames as u64;
        let previous = WAL_LAST.swap(now, Ordering::Relaxed);
        use std::os::unix::fs::FileExt;
        let mut header = [0u8; 32];
        let f = std::fs::File::open(format!("{}-wal", path())).unwrap();
        f.read_exact_at(&mut header, 0).unwrap();
        let salt = u64::from_be_bytes(header[16..24].try_into().unwrap());
        let oldsalt = WAL_SALT.swap(salt, Ordering::Relaxed);
        let delta = if salt != oldsalt || now < previous {
            now
        } else {
            now - previous
        };
        WAL_FRAMES.fetch_add(delta, Ordering::Relaxed);
        WAL_COMMIT_FRAMES.store(delta, Ordering::Relaxed);
        WAL_COMMITS.fetch_add(1, Ordering::Relaxed);
        WAL_PEAK.fetch_max(now * 4120 + 32, Ordering::Relaxed);
        Ok(())
    }));
}
pub fn wal_stats() -> Value {
    json!({"frames":WAL_FRAMES.load(Ordering::Relaxed),
    "frame_bytes":WAL_FRAMES.load(Ordering::Relaxed)*4120,"commits":WAL_COMMITS.load(Ordering::Relaxed),
    "peak_logical_bytes":WAL_PEAK.load(Ordering::Relaxed)})
}
pub fn checkpoint(c: &Connection) {
    c.execute_batch("pragma wal_checkpoint(truncate)").unwrap();
    WAL_LAST.store(0, Ordering::Relaxed);
}
pub fn maybe_checkpoint(c: &Connection) {
    if WAL_LAST.load(Ordering::Relaxed) > 8192 {
        c.execute_batch("pragma wal_checkpoint(passive)").unwrap();
    }
}
pub fn validate(c: &Connection) {
    assert_eq!(
        c.query_row("pragma integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
}
pub fn hash_ids(ids: &[i64]) -> u64 {
    ids.iter().fold(0xcbf29ce484222325u64, |h, id| {
        h.wrapping_mul(0x100000001b3) ^ *id as u64
    })
}

#[cfg(feature = "alloc-metrics")]
mod allocations {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};
    pub struct Count;
    pub static CALLS: AtomicU64 = AtomicU64::new(0);
    pub static BYTES: AtomicU64 = AtomicU64::new(0);
    // System owns each allocation; counters never allocate and observe successful requests only.
    unsafe impl GlobalAlloc for Count {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            let p = unsafe { System.alloc(l) };
            if !p.is_null() {
                CALLS.fetch_add(1, Ordering::Relaxed);
                BYTES.fetch_add(l.size() as u64, Ordering::Relaxed);
            }
            p
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            let r = unsafe { System.realloc(p, l, n) };
            if !r.is_null() {
                CALLS.fetch_add(1, Ordering::Relaxed);
                BYTES.fetch_add(n as u64, Ordering::Relaxed);
            }
            r
        }
    }
}
#[cfg(feature = "alloc-metrics")]
#[global_allocator]
static ALLOCATOR: allocations::Count = allocations::Count;
pub fn allocations() -> Value {
    #[cfg(feature = "alloc-metrics")]
    return json!({"rust_calls":allocations::CALLS.load(Ordering::Relaxed),"rust_requested_bytes":allocations::BYTES.load(Ordering::Relaxed),"scope":"Rust System allocations only; excludes SQLite C and mmap"});
    #[cfg(not(feature = "alloc-metrics"))]
    json!(null)
}
