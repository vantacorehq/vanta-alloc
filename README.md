<p align="center">
  <img src="banner.svg" alt="banner" width="100%">
</p>

# VANTA-ALLOC

**ZERO-DEPENDENCY BUMP ALLOCATOR.**

## Overview

`VANTA-ALLOC` is a global bump allocator. `BumpAllocator::new` maps one region with `mmap` on first use and serves allocations by advancing an atomic cursor. Individual `deallocate` of a pointer inside that region does not rewind the cursor. If the region cannot satisfy a layout, `try_alloc` returns `Err(())` and the `GlobalAlloc` impl falls back to the system allocator.

The release binary prints the cursor before and after it builds two `Vec`s, the bump-allocation counter, and the mapped base address.

## Behavior

1. **Mapped region:** the global allocator requests its bytes with `mmap`.
2. **Bump allocation:** compare-and-swap reserves an aligned span and returns a pointer into the region.
3. **No individual reclaim:** `deallocate` of an in-region pointer leaves the cursor where it is.
4. **Exhaustion:** `try_alloc` fails when the cursor would pass the end of the region.

Unit tests construct the allocator over a caller-provided buffer. They do not call `mmap`.

## Build & Run

```bash
cargo test
cargo build --release
./target/release/vanta-alloc
```
