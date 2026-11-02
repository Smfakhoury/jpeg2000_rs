//! Test-only allocation accounting. Production code keeps its unsafe-code ban.
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default)]
pub struct Counts {
    pub allocations: usize,
    pub live_blocks: isize,
    pub live_bytes: isize,
}

thread_local! {
    static COUNTS: Cell<Option<Counts>> = const { Cell::new(None) };
}

fn update(blocks: isize, bytes: isize, allocated: bool) {
    // Const TLS needs no allocation; ignore callbacks during thread teardown.
    let _ = COUNTS.try_with(|cell| {
        if let Some(mut counts) = cell.get() {
            counts.allocations += usize::from(allocated);
            counts.live_blocks += blocks;
            counts.live_bytes += bytes;
            cell.set(Some(counts));
        }
    });
}

struct CountingSystem;

// Each operation delegates with the original pointer/layout to System.
unsafe impl GlobalAlloc for CountingSystem {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            update(1, layout.size() as isize, true);
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            update(1, layout.size() as isize, true);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        update(-1, -(layout.size() as isize), false);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let replacement = unsafe { System.realloc(pointer, layout, size) };
        if !replacement.is_null() {
            update(0, size as isize - layout.size() as isize, true);
        }
        replacement
    }
}

#[global_allocator]
static ALLOCATOR: CountingSystem = CountingSystem;

struct Reset;

impl Drop for Reset {
    fn drop(&mut self) {
        COUNTS.with(|cell| cell.set(None));
    }
}

/// Count only this thread, only during `call`; returned owners remain live.
pub fn measure<T>(call: impl FnOnce() -> T) -> (T, Counts) {
    COUNTS.with(|cell| {
        assert!(cell.get().is_none(), "nested allocation measurement");
        cell.set(Some(Counts::default()));
    });
    let reset = Reset;
    let result = call();
    let counts = COUNTS.with(|cell| cell.get().unwrap());
    drop(reset);
    (result, counts)
}
