mod ingest;
mod metrics;
mod records;

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

static TRACK: AtomicBool = AtomicBool::new(false);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static ALLOC_COUNT: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "counting-allocator")]
struct CountingAllocator;
#[cfg(feature = "counting-allocator")]
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() && TRACK.load(Ordering::Relaxed) {
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() && TRACK.load(Ordering::Relaxed) {
            ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let ptr = unsafe { System.realloc(ptr, layout, size) };
        if !ptr.is_null() && TRACK.load(Ordering::Relaxed) {
            ALLOC_BYTES.fetch_add(size as u64, Ordering::Relaxed);
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        ptr
    }
}
#[global_allocator]
#[cfg(feature = "counting-allocator")]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy)]
pub struct Sample {
    pub at: i64,
    pub value: f64,
}
pub enum Payload {
    Bytes(Vec<u8>),
    Words(Vec<u64>),
    Samples(Vec<Sample>),
}
pub struct Case {
    pub name: String,
    pub units: usize,
    pub input_bytes: usize,
    pub input_hash: u64,
    pub run: Box<dyn FnMut() -> Payload>,
}

pub fn fingerprint_bytes(data: &[u8], parameters: &[u64]) -> u64 {
    let mut h = 14695981039346656037u64;
    for b in data
        .iter()
        .copied()
        .chain(parameters.iter().flat_map(|v| v.to_le_bytes()))
    {
        h = (h ^ b as u64).wrapping_mul(1099511628211);
    }
    h
}
pub fn fingerprint_words(words: &[u64], parameters: &[u64]) -> u64 {
    let mut h = 14695981039346656037u64;
    for b in words.iter().chain(parameters).flat_map(|v| v.to_le_bytes()) {
        h = (h ^ b as u64).wrapping_mul(1099511628211);
    }
    h
}
pub fn fingerprint_samples(points: &[Sample]) -> u64 {
    let mut h = 14695981039346656037u64;
    for p in points {
        for b in
            p.at.to_le_bytes()
                .into_iter()
                .chain(p.value.to_bits().to_le_bytes())
        {
            h = (h ^ b as u64).wrapping_mul(1099511628211);
        }
    }
    h
}

impl Payload {
    fn len_bytes(&self) -> usize {
        match self {
            Self::Bytes(v) => v.len(),
            Self::Words(v) => v.len() * 8,
            Self::Samples(v) => v.len() * 16,
        }
    }
    fn capacity_bytes(&self) -> usize {
        match self {
            Self::Bytes(v) => v.capacity(),
            Self::Words(v) => v.capacity() * 8,
            Self::Samples(v) => v.capacity() * 16,
        }
    }
    fn bytes(&self) -> Vec<u8> {
        match self {
            Self::Bytes(v) => v.clone(),
            Self::Words(v) => v.iter().flat_map(|n| n.to_le_bytes()).collect(),
            Self::Samples(v) => v
                .iter()
                .flat_map(|p| {
                    p.at.to_le_bytes()
                        .into_iter()
                        .chain(p.value.to_bits().to_le_bytes())
                })
                .collect(),
        }
    }
}

fn rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmRSS:")
                .map(|rest| rest.split_whitespace().next().unwrap().parse().unwrap())
        })
        .unwrap()
}

fn run_bench(c: &mut Case, seconds: f64) {
    let mut iterations = 1u64;
    loop {
        let start = Instant::now();
        let mut size = 0;
        for _ in 0..iterations {
            let p = black_box((c.run)());
            size = p.len_bytes();
            drop(p);
        }
        let elapsed = start.elapsed().as_secs_f64();
        if elapsed >= seconds {
            println!(
                "{{\"mode\":\"bench\",\"language\":\"rust\",\"case\":\"{}\",\"units\":{},\"iterations\":{},\"ns_per_op\":{},\"output_bytes\":{}}}",
                c.name,
                c.units,
                iterations,
                elapsed * 1e9 / iterations as f64,
                size
            );
            return;
        }
        iterations *= 2;
    }
}

fn run_alloc(c: &mut Case, n: usize) {
    assert!(
        cfg!(feature = "counting-allocator"),
        "use allocation-instrumented binary"
    );
    for _ in 0..8 {
        black_box((c.run)());
    }
    ALLOC_BYTES.store(0, Ordering::Relaxed);
    ALLOC_COUNT.store(0, Ordering::Relaxed);
    TRACK.store(true, Ordering::Relaxed);
    let mut size = 0;
    let mut capacity = 0;
    for _ in 0..n {
        let p = black_box((c.run)());
        size = p.len_bytes();
        capacity = p.capacity_bytes();
        drop(p);
    }
    TRACK.store(false, Ordering::Relaxed);
    println!(
        "{{\"mode\":\"alloc\",\"language\":\"rust\",\"case\":\"{}\",\"iterations\":{},\"bytes_per_op\":{},\"allocs_per_op\":{},\"output_bytes\":{},\"output_capacity_bytes\":{}}}",
        c.name,
        n,
        ALLOC_BYTES.load(Ordering::Relaxed) as f64 / n as f64,
        ALLOC_COUNT.load(Ordering::Relaxed) as f64 / n as f64,
        size,
        capacity
    );
}

fn run_memory(c: &mut Case, n: usize) {
    for _ in 0..8 {
        black_box((c.run)());
    }
    let baseline = rss_kib();
    let mut retained = Vec::with_capacity(n);
    let mut logical = 0;
    let mut capacity = 0;
    for _ in 0..n {
        let p = black_box((c.run)());
        logical += p.len_bytes();
        capacity += p.capacity_bytes();
        retained.push(p);
    }
    println!(
        "{{\"mode\":\"memory\",\"language\":\"rust\",\"case\":\"{}\",\"retained\":{},\"logical_output_bytes\":{},\"capacity_output_bytes\":{},\"retained_harness_metadata_bytes\":{},\"baseline_rss_kib\":{},\"rss_kib\":{}}}",
        c.name,
        n,
        logical,
        capacity,
        n * std::mem::size_of::<Payload>(),
        baseline,
        rss_kib()
    );
    black_box(&retained);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let option = |key: &str, default: &str| -> String {
        args.windows(2)
            .find(|a| a[0] == key)
            .map_or_else(|| default.to_owned(), |a| a[1].clone())
    };
    let mode = option("--mode", "verify");
    let filter = option("--case", "");
    let seconds = option("--seconds", "0.15").parse().unwrap();
    let n = option("--iterations", "256").parse().unwrap();
    let mut cases = metrics::cases();
    cases.extend(records::cases());
    cases.extend(ingest::cases());
    if !filter.is_empty() {
        cases.retain(|c| c.name == filter);
    }
    let mut found = false;
    for c in &mut cases {
        if !filter.is_empty() && c.name != filter {
            continue;
        }
        found = true;
        match mode.as_str() {
            "verify" => {
                let p = (c.run)();
                let hex = p
                    .bytes()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>();
                println!(
                    "{{\"mode\":\"verify\",\"case\":\"{}\",\"units\":{},\"input_bytes\":{},\"input_fingerprint\":\"{:016x}\",\"hex\":\"{}\"}}",
                    c.name, c.units, c.input_bytes, c.input_hash, hex
                );
            }
            "bench" => run_bench(c, seconds),
            "alloc" => run_alloc(c, n),
            "memory" => run_memory(c, n),
            _ => panic!("unknown mode"),
        }
    }
    assert!(found, "case not found");
}
