//! One process setting, established before any timed operation.
use crate::engine::Result;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
static THREADS: AtomicUsize = AtomicUsize::new(0);
static FORCE: AtomicBool = AtomicBool::new(false);
static INITIALIZED: OnceLock<std::result::Result<usize, String>> = OnceLock::new();

pub(super) fn configure(threads: usize, force: bool) -> Result<()> {
    if threads > 2 {
        return Err("parallel metrics prototype supports zero, one or two worker threads".into());
    }
    if threads > 0 {
        let initialized = INITIALIZED.get_or_init(|| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .thread_name(|i| format!("metrics-decode-{i}"))
                .build_global()
                .map(|()| threads)
                .map_err(|e| e.to_string())
        });
        match initialized {
            Ok(count) if *count == threads => {}
            Ok(_) => {
                return Err("parallel metrics pool already has a different thread count".into());
            }
            Err(error) => return Err(error.clone().into()),
        }
    }
    THREADS.store(threads, Ordering::Relaxed);
    FORCE.store(force, Ordering::Relaxed);
    Ok(())
}
pub(super) fn active() -> bool {
    THREADS.load(Ordering::Relaxed) > 0
}
pub(super) fn enabled(series: usize, samples: usize, _summary_blocks: usize) -> bool {
    THREADS.load(Ordering::Relaxed) > 0
        && (FORCE.load(Ordering::Relaxed) || (series >= 2 && samples >= 4096))
}
pub(super) fn batch_size() -> usize {
    (THREADS.load(Ordering::Relaxed) * 4).clamp(1, 8)
}
