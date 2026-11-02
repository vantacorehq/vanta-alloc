# VANTA-ALLOC

**ZERO-DEPENDENCY CUSTOM MEMORY ALLOCATOR.**

## Overview

The standard system `malloc` is a general-purpose allocator designed to handle a massive variety of allocation patterns. In high-performance systems (like HFT engines or real-time data pipelines), the overhead of system calls and lock contention in `malloc` is unacceptable.

**VANTA-ALLOC** is a custom global memory allocator written in pure Rust. It replaces the standard library allocator with a high-performance, lock-free **Bump Allocator** (Arena Allocator).

## Architecture

1. **mmap Direct Access:** Bypasses the standard C library and requests memory directly from the Linux kernel via the `mmap` syscall.
2. **Lock-Free Allocation:** Uses atomic CAS (Compare-And-Swap) operations to manage the heap offset, allowing multiple threads to allocate memory without locking.
3. **Bump Allocation:** Simply increments a pointer to allocate memory. Extremely fast (O(1) allocations).

## Build & Run

```bash
cargo build --release
./target/release/vanta-alloc
