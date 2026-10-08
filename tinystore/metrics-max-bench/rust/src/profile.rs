//! Diagnostic-only instrumentation, compiled out of timed release variants.
#[cfg(feature = "telemetry")]
mod imp {
    use serde_json::{Value, json};
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::Instant;
    static ENABLED: AtomicBool = AtomicBool::new(false);
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static BYTES: AtomicU64 = AtomicU64::new(0);
    pub struct MeteredAllocator;
    unsafe impl GlobalAlloc for MeteredAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let p = unsafe { System.alloc(layout) };
            if !p.is_null() && ENABLED.load(Ordering::Relaxed) {
                CALLS.fetch_add(1, Ordering::Relaxed);
                BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            }
            p
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let p = unsafe { System.alloc_zeroed(layout) };
            if !p.is_null() && ENABLED.load(Ordering::Relaxed) {
                CALLS.fetch_add(1, Ordering::Relaxed);
                BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
            }
            p
        }
        unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
            unsafe { System.dealloc(p, layout) }
        }
        unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            let p = unsafe { System.realloc(p, layout, size) };
            if !p.is_null() && ENABLED.load(Ordering::Relaxed) {
                CALLS.fetch_add(1, Ordering::Relaxed);
                BYTES.fetch_add(size as u64, Ordering::Relaxed);
            }
            p
        }
    }
    #[global_allocator]
    static ALLOCATOR: MeteredAllocator = MeteredAllocator;
    #[derive(Default)]
    struct Record {
        calls: u64,
        ns: u128,
        allocations: u64,
        requested_bytes: u64,
    }
    thread_local! { static RECORDS: RefCell<BTreeMap<&'static str,Record>> = RefCell::new(BTreeMap::new()); }
    pub struct Scope {
        name: &'static str,
        start: Option<Instant>,
        allocations: u64,
        bytes: u64,
    }
    pub fn scope(name: &'static str) -> Scope {
        let active = ENABLED.load(Ordering::Relaxed);
        Scope {
            name,
            start: active.then(Instant::now),
            allocations: if active {
                CALLS.load(Ordering::Relaxed)
            } else {
                0
            },
            bytes: if active {
                BYTES.load(Ordering::Relaxed)
            } else {
                0
            },
        }
    }
    impl Drop for Scope {
        fn drop(&mut self) {
            let Some(start) = self.start else { return };
            let elapsed = start.elapsed().as_nanos();
            let allocations = CALLS.load(Ordering::Relaxed) - self.allocations;
            let bytes = BYTES.load(Ordering::Relaxed) - self.bytes;
            RECORDS.with(|records| {
                let mut records = records.borrow_mut();
                let record = records.entry(self.name).or_default();
                record.calls += 1;
                record.ns += elapsed;
                record.allocations += allocations;
                record.requested_bytes += bytes;
            });
        }
    }
    pub fn configure(enabled: bool) {
        ENABLED.store(enabled, Ordering::Relaxed);
    }
    pub fn enabled() -> bool {
        ENABLED.load(Ordering::Relaxed)
    }
    pub fn reset() {
        configure(false);
        RECORDS.with(|r| {
            let mut records = r.borrow_mut();
            records.clear();
            for name in [
                "operation",
                "read_total",
                "read_process",
                "aggregate_total",
                "aggregate_process",
                "snapshot",
                "match_registry",
                "fetch_heads",
                "fetch_group_rows",
                "fetch_payloads",
                "fold_series",
                "decode_block",
                "parse_head",
                "decode_head_chunk",
            ] {
                records.insert(name, Record::default());
            }
        });
        CALLS.store(0, Ordering::Relaxed);
        BYTES.store(0, Ordering::Relaxed);
    }
    pub fn finish() -> Value {
        configure(false);
        RECORDS.with(|records| {
            let entries: BTreeMap<_,_> = records.borrow().iter().map(|(name,r)|
                (*name,json!({"calls":r.calls,"inclusive_ns":r.ns,
                    "rust_allocation_calls":r.allocations,"rust_requested_bytes":r.requested_bytes}))).collect();
            json!({"stages":entries,"rust_allocation_calls":CALLS.load(Ordering::Relaxed),
                "rust_requested_bytes":BYTES.load(Ordering::Relaxed)})
        })
    }
}
#[cfg(not(feature = "telemetry"))]
mod imp {
    pub struct Scope;
    #[inline(always)]
    pub fn scope(_: &'static str) -> Scope {
        Scope
    }
    pub fn configure(_: bool) {}
    pub fn enabled() -> bool {
        false
    }
    pub fn reset() {}
    pub fn finish() -> serde_json::Value {
        serde_json::json!({})
    }
}
pub use imp::*;
