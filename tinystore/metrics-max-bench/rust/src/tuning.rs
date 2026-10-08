//! Independent prototype switches, fixed before opening the engine.
use std::sync::atomic::{AtomicUsize, Ordering};
static MASK: AtomicUsize = AtomicUsize::new(0);
pub fn configure(mask: usize) {
    MASK.store(mask, Ordering::Relaxed);
}
pub fn bits() -> bool {
    MASK.load(Ordering::Relaxed) & 1 != 0
}
pub fn buffers() -> bool {
    MASK.load(Ordering::Relaxed) & 2 != 0
}
pub fn fused() -> bool {
    MASK.load(Ordering::Relaxed) & 4 != 0
}
pub fn specialized() -> bool {
    MASK.load(Ordering::Relaxed) & 8 != 0
}
