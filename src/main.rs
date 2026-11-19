// VANTA-ALLOC - bump allocator backed by one mmap region.
// Compile: cargo build --release
// Run: ./target/release/vanta-alloc

use vanta_alloc::BumpAllocator;

const HEAP_SIZE: usize = 1024 * 1024 * 16; // 16 MB heap for demonstration

#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new(HEAP_SIZE);

fn main() {
    println!("[VANTA-ALLOC] Initializing custom global allocator...");

    let cursor_before = ALLOCATOR.cursor();
    let mut vec1 = Vec::with_capacity(1000);
    for i in 0..1000 {
        vec1.push(i);
    }
    let cursor_after_vec1 = ALLOCATOR.cursor();

    let mut vec2 = Vec::with_capacity(5000);
    for i in 0..5000 {
        vec2.push(i * 2);
    }
    let cursor_after_vec2 = ALLOCATOR.cursor();

    println!(
        "[VANTA-ALLOC] Vec 1 len {} capacity {} ({} bytes of i32 slots)",
        vec1.len(),
        vec1.capacity(),
        vec1.capacity() * std::mem::size_of_val(&vec1[0])
    );
    println!(
        "[VANTA-ALLOC] Vec 2 len {} capacity {} ({} bytes of i32 slots)",
        vec2.len(),
        vec2.capacity(),
        vec2.capacity() * std::mem::size_of_val(&vec2[0])
    );
    println!(
        "[VANTA-ALLOC] Arena cursor {cursor_before} -> {cursor_after_vec1} -> {cursor_after_vec2} / {} bytes",
        ALLOCATOR.capacity()
    );
    println!(
        "[VANTA-ALLOC] Bump allocations served from the arena: {}",
        ALLOCATOR.bump_allocs()
    );
    println!(
        "[VANTA-ALLOC] Mapped region base: {:#x}",
        ALLOCATOR.region_start()
    );
    if ALLOCATOR.bump_allocs() > 0
        && ALLOCATOR.region_start() != 0
        && cursor_after_vec2 > cursor_before
    {
        println!(
            "[VANTA-ALLOC] Cursor advanced while the Vecs were built; those bump allocations came from the mmap region."
        );
    }
    println!("[SUCCESS] Execution complete.");
    std::hint::black_box((vec1, vec2));
}
