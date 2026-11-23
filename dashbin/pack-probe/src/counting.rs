//! The system's allocator with a count of the bytes that are allocated now, which is how the probe
//! says what a loaded table keeps. The count costs each allocation an atomic add, the same for
//! every table measured.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

pub struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);

pub fn live() -> usize {
    LIVE.load(Relaxed)
}

// SAFETY: every call is the system allocator's own, with the layout it was given.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let given = unsafe { System.alloc(layout) };
        if !given.is_null() {
            LIVE.fetch_add(layout.size(), Relaxed);
        }
        given
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        LIVE.fetch_sub(layout.size(), Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let given = unsafe { System.realloc(pointer, layout, size) };
        if !given.is_null() {
            LIVE.fetch_add(size, Relaxed);
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
        given
    }
}
