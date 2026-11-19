//! Bump allocator used by the `vanta-alloc` demo.
//!
//! Individual deallocation does not rewind the cursor. The mapped region is
//! requested with `mmap` on first use of a [`BumpAllocator::new`] value.
//! A caller-provided region ([`BumpAllocator::from_region`]) is used by tests
//! and does not call `mmap`. When the bump region cannot satisfy a layout,
//! [`try_alloc`] returns `Err`. The `GlobalAlloc` impl falls back to the
//! system allocator in that case so a process is not aborted, and
//! `dealloc` of those system pointers is forwarded. Pointers inside the bump
//! region are not reclaimed.

use std::alloc::{GlobalAlloc, Layout, System};
use std::os::raw::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

const PROT_READ: i32 = 1;
const PROT_WRITE: i32 = 2;
const MAP_PRIVATE: i32 = 0x02;
const MAP_ANONYMOUS: i32 = 0x20;

extern "C" {
    fn mmap(
        addr: *mut c_void,
        length: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut c_void;
    fn munmap(addr: *mut c_void, length: usize) -> i32;
}

/// Atomic bump cursor over a fixed region.
pub struct BumpAllocator {
    offset: AtomicUsize,
    start: AtomicUsize,
    capacity: usize,
    /// Successful bump allocations (not system-allocator fallbacks).
    bump_allocs: AtomicUsize,
}

unsafe impl Sync for BumpAllocator {}
unsafe impl Send for BumpAllocator {}

impl BumpAllocator {
    /// Empty mmap-backed arena. The mapping is created on the first allocation.
    pub const fn new(capacity: usize) -> Self {
        Self {
            offset: AtomicUsize::new(0),
            start: AtomicUsize::new(0),
            capacity,
            bump_allocs: AtomicUsize::new(0),
        }
    }

    /// Bump allocator over an existing writable region.
    ///
    /// # Safety
    ///
    /// `start` must point at `capacity` writable bytes that stay alive and
    /// are not mutated except through this allocator, for the allocator's
    /// lifetime. `start` should be aligned for the layouts that will be
    /// requested.
    pub unsafe fn from_region(start: *mut u8, capacity: usize) -> Self {
        Self {
            offset: AtomicUsize::new(0),
            start: AtomicUsize::new(start as usize),
            capacity,
            bump_allocs: AtomicUsize::new(0),
        }
    }

    pub fn cursor(&self) -> usize {
        self.offset.load(Ordering::Relaxed)
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn bump_allocs(&self) -> usize {
        self.bump_allocs.load(Ordering::Relaxed)
    }

    /// Base address of the region, or 0 if `mmap` has not run yet.
    pub fn region_start(&self) -> usize {
        self.start.load(Ordering::Acquire)
    }

    fn init_mmap(&self) {
        if self.start.load(Ordering::Acquire) != 0 {
            return;
        }
        let ptr = unsafe {
            mmap(
                std::ptr::null_mut(),
                self.capacity,
                PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        if ptr.is_null() || ptr == usize::MAX as *mut c_void {
            panic!("mmap failed to allocate heap");
        }
        if self
            .start
            .compare_exchange(0, ptr as usize, Ordering::Release, Ordering::Acquire)
            .is_err()
        {
            unsafe {
                munmap(ptr, self.capacity);
            }
        }
    }

    /// Reserve `layout` from the bump region.
    ///
    /// Returns `Err(())` when the region is exhausted. Does not fall back to
    /// the system allocator and does not rewind on failure.
    pub fn try_alloc(&self, layout: Layout) -> Result<*mut u8, ()> {
        if self.start.load(Ordering::Acquire) == 0 {
            self.init_mmap();
        }
        let size = layout.size();
        let align = layout.align();
        let mut current = self.offset.load(Ordering::Relaxed);
        loop {
            let aligned = (current + align - 1) & !(align - 1);
            let new_offset = aligned.checked_add(size).ok_or(())?;
            if new_offset > self.capacity {
                return Err(());
            }
            match self.offset.compare_exchange_weak(
                current,
                new_offset,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let start = self.start.load(Ordering::Acquire);
                    self.bump_allocs.fetch_add(1, Ordering::Relaxed);
                    return Ok((start + aligned) as *mut u8);
                }
                Err(actual) => current = actual,
            }
        }
    }

    /// Drop a block.
    ///
    /// Pointers inside the bump region are ignored: the cursor is not rewound.
    /// Pointers outside the region (system-allocator fallback) are passed to
    /// the system allocator.
    pub fn deallocate(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }
        let start = self.start.load(Ordering::Acquire);
        let addr = ptr as usize;
        let in_region = start != 0 && addr >= start && addr < start.saturating_add(self.capacity);
        if !in_region {
            unsafe { System.dealloc(ptr, layout) };
        }
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match self.try_alloc(layout) {
            Ok(ptr) => ptr,
            Err(()) => System.alloc(layout),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.deallocate(ptr, layout);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::{alloc, dealloc, Layout};

    struct Region {
        ptr: *mut u8,
        layout: Layout,
    }

    impl Region {
        fn new(size: usize, align: usize) -> Self {
            let layout = Layout::from_size_align(size, align).unwrap();
            let ptr = unsafe { alloc(layout) };
            assert!(!ptr.is_null());
            Self { ptr, layout }
        }

        fn arena(&self) -> BumpAllocator {
            unsafe { BumpAllocator::from_region(self.ptr, self.layout.size()) }
        }
    }

    impl Drop for Region {
        fn drop(&mut self) {
            unsafe { dealloc(self.ptr, self.layout) }
        }
    }

    #[test]
    fn two_blocks_do_not_overlap() {
        let region = Region::new(4096, 64);
        let arena = region.arena();
        let first_layout = Layout::from_size_align(100, 8).unwrap();
        let second_layout = Layout::from_size_align(50, 8).unwrap();
        let a = arena.try_alloc(first_layout).unwrap();
        let b = arena.try_alloc(second_layout).unwrap();
        assert!(!a.is_null() && !b.is_null());
        let a_end = a as usize + first_layout.size();
        let b_addr = b as usize;
        assert!(a_end <= b_addr, "blocks overlap: {a:p} then {b:p}");
    }

    #[test]
    fn cursor_advances() {
        let region = Region::new(1024, 64);
        let arena = region.arena();
        assert_eq!(arena.cursor(), 0);
        let layout = Layout::from_size_align(64, 8).unwrap();
        arena.try_alloc(layout).unwrap();
        let mid = arena.cursor();
        assert!(mid >= 64, "cursor {mid}");
        arena.try_alloc(layout).unwrap();
        assert!(arena.cursor() > mid);
        assert_eq!(arena.bump_allocs(), 2);
    }

    #[test]
    fn exhaustion_returns_err() {
        let region = Region::new(128, 16);
        let arena = region.arena();
        let layout = Layout::from_size_align(80, 8).unwrap();
        assert!(arena.try_alloc(layout).is_ok());
        assert_eq!(arena.try_alloc(layout), Err(()));
        assert!(arena.cursor() <= arena.capacity());
    }

    #[test]
    fn dealloc_does_not_rewind_cursor() {
        let region = Region::new(256, 16);
        let arena = region.arena();
        let layout = Layout::from_size_align(32, 8).unwrap();
        let first = arena.try_alloc(layout).unwrap();
        let cursor = arena.cursor();
        assert!(cursor > 0);
        arena.deallocate(first, layout);
        assert_eq!(arena.cursor(), cursor, "dealloc rewound the bump cursor");
        let second = arena.try_alloc(layout).unwrap();
        assert_ne!(first, second);
        assert!(arena.cursor() > cursor);
    }
}
