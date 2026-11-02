// VANTA-ALLOC - Custom Global Memory Allocator
// Compile: cargo build --release
// Run: ./target/release/vanta-alloc

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::os::raw::c_void;

// Constants for memory mapping
const PAGE_SIZE: usize = 4096;
const HEAP_SIZE: usize = 1024 * 1024 * 16; // 16 MB heap for demonstration

// FFI bindings to libc mmap
extern "C" {
    fn mmap(addr: *mut c_void, length: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut c_void;
}

const PROT_READ: i32 = 1;
const PROT_WRITE: i32 = 2;
const MAP_PRIVATE: i32 = 0x02;
const MAP_ANONYMOUS: i32 = 0x20;

// Bump Allocator Structure
struct BumpAllocator {
    offset: AtomicUsize,
    start: AtomicUsize, // We store the pointer as usize to satisfy Send/Sync
}

unsafe impl Sync for BumpAllocator {}

impl BumpAllocator {
    const fn new() -> Self {
        BumpAllocator {
            offset: AtomicUsize::new(0),
            start: AtomicUsize::new(0),
        }
    }

    fn init(&self) {
        if self.start.load(Ordering::Relaxed) == 0 {
            unsafe {
                let ptr = mmap(
                    std::ptr::null_mut(),
                    HEAP_SIZE,
                    PROT_READ | PROT_WRITE,
                    MAP_PRIVATE | MAP_ANONYMOUS,
                    -1,
                    0,
                );
                if ptr == usize::MAX as *mut c_void {
                    panic!("mmap failed to allocate heap");
                }
                self.start.store(ptr as usize, Ordering::Relaxed);
            }
        }
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.init();
        
        let mut current_offset = self.offset.load(Ordering::Relaxed);
        loop {
            // Align the current offset based on the requested layout alignment
            let aligned_offset = (current_offset + layout.align() - 1) & !(layout.align() - 1);
            let new_offset = aligned_offset + layout.size();

            if new_offset > HEAP_SIZE {
                // Heap exhausted, fallback to system allocator
                return System.alloc(layout);
            }

            // Try to atomically update the offset
            match self.offset.compare_exchange_weak(
                current_offset,
                new_offset,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let start_ptr = self.start.load(Ordering::Relaxed);
                    return (start_ptr + aligned_offset) as *mut u8;
                }
                Err(actual) => {
                    current_offset = actual;
                }
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocators typically don't free individual blocks.
        // Memory is reclaimed when the entire arena is dropped.
        // For this PoC, we do nothing, leaving memory to be freed by the OS on exit.
    }
}

// Declare our custom allocator as the global allocator
#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new();

fn main() {
    println!("[VANTA-ALLOC] Initializing custom global allocator...");
    
    // Allocate some vectors to trigger the custom allocator
    let mut vec1 = Vec::with_capacity(1000);
    for i in 0..1000 {
        vec1.push(i);
    }
    
    let mut vec2 = Vec::with_capacity(5000);
    for i in 0..5000 {
        vec2.push(i * 2);
    }

    println!("[VANTA-ALLOC] Allocated {} bytes for Vec 1", vec1.capacity());
    println!("[VANTA-ALLOC] Allocated {} bytes for Vec 2", vec2.capacity());
    println!("[SUCCESS] Execution complete. Custom mmap allocator bypassed standard system malloc.");
}
