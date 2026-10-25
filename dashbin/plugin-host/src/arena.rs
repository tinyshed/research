// quickjs's heaps in a jemalloc arena of their own that returns freed pages at once,
// while the rest of the process keeps the decay that makes it fast
use std::ffi::{CString, c_void};
use std::sync::OnceLock;

use rquickjs::allocator::Allocator;
use tikv_jemalloc_sys::{MALLOCX_ARENA, MALLOCX_TCACHE_NONE, MALLOCX_ZERO, dallocx, mallctl, mallocx, rallocx, sallocx};

static ARENA: OnceLock<usize> = OnceLock::new();

fn arena() -> usize {
    *ARENA.get_or_init(|| unsafe {
        let mut index: u32 = 0;
        let mut len = size_of::<u32>();
        let created = mallctl(c"arenas.create".as_ptr(), &mut index as *mut u32 as *mut c_void, &mut len, std::ptr::null_mut(), 0);
        assert_eq!(created, 0, "jemalloc would not create an arena");
        for key in ["dirty_decay_ms", "muzzy_decay_ms"] {
            let name = CString::new(format!("arena.{index}.{key}")).unwrap();
            let mut zero: isize = 0;
            mallctl(name.as_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), &mut zero as *mut isize as *mut c_void, size_of::<isize>());
        }
        index as usize
    })
}

pub struct JsArena {
    flags: i32,
}

impl JsArena {
    pub fn new() -> Self {
        // no thread cache, so a freed region goes straight back to the arena it came from
        JsArena { flags: MALLOCX_ARENA(arena()) | MALLOCX_TCACHE_NONE }
    }
}

unsafe impl Allocator for JsArena {
    fn alloc(&mut self, size: usize) -> *mut u8 {
        unsafe { mallocx(size.max(1), self.flags) as *mut u8 }
    }

    fn calloc(&mut self, count: usize, size: usize) -> *mut u8 {
        match count.checked_mul(size) {
            Some(total) => unsafe { mallocx(total.max(1), self.flags | MALLOCX_ZERO) as *mut u8 },
            None => std::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8) {
        if !ptr.is_null() {
            unsafe { dallocx(ptr as *mut c_void, self.flags) }
        }
    }

    unsafe fn realloc(&mut self, ptr: *mut u8, new_size: usize) -> *mut u8 {
        if ptr.is_null() {
            return self.alloc(new_size);
        }
        unsafe { rallocx(ptr as *mut c_void, new_size.max(1), self.flags) as *mut u8 }
    }

    unsafe fn usable_size(ptr: *mut u8) -> usize {
        unsafe { sallocx(ptr as *const c_void, 0) }
    }
}
