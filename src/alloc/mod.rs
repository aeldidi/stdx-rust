//! Module alloc contains a collection of memory allocators tailored to
//! different use cases.
//!
//! Here's a quick overview on when you should use the following allocators and
//! why they might be more efficient or convenient:
//!
//! ## `FixedBufferAllocator`
//!
//! In cases where you have a pre-allocated buffer where an output should be
//! placed into, or perhaps where you want to enforce some upper bound on
//! memory usage, the `FixedBufferAllocator` might be a good choice. The
//! `FixedBufferAllocator` allocates memory from a `[u8]`, giving an out of
//! memory error when the buffer gets filled. Nothing in the
//! `FixedBufferAllocator` can live longer than it, since once the backing
//! buffer goes out of scope, the slice memory cannot be relied on.
//!
//! ## `Pool`
//!
//! When you have a lot of a specific type whose lifetime is the same, use a
//! `Pool` to efficiently allocate many of them, ensuring they're placed next
//! to each other in memory. If you're frequently accessing these, this will
//! result in less cache misses and better performance.
//!
//! ## `VirtualMemoryAllocator`
//!
//! On modern operating systems, the computer's actual memory is typically
//! abstracted away from processes, hiding the fact that they exist alongside
//! other programs and do not have access to all of RAM. Taking advantage of
//! this fact allows us to "allocate" extremely large amounts of continuous
//! memory which may not actually be availible, which the OS will provide to us
//! if and when we use it.
//!
//! Namely, this is useful in cases where we have some dynamically sized data
//! which lives for the duration of the entire program. We don't want to
//! statically allocate to some reasonable upper limit, since this would result
//! in wasted memory when only a little is used. It's also not ideal to
//! individually allocate each object, since then we have to deal with the
//! lifetimes of each object individually.
//!
//! By using the `VirtualMemoryAllocator`, we can get the best of both worlds
//! by simply allocating some ridiculously large amount of virtual memory
//! (64-bit computers typically have obscene amounts of virtual memory
//! addresses, meaning you don't have to worry about reserving too much) and
//! allocating objects within it.
//!
//! The most common use case for this is having many typed
//! `Pool<VirtualMemoryAllocator>` allocators for each object type, which would
//! give you memory locality, and free you from worrying about lifetimes (since
//! everything could be `'static` if the `VirtualMemoryAllocator` is).
//!
//! ## `Mallocator`
//!
//! This is just the libc `malloc`/`free` wrapped up to implement the
//! `Allocator` trait. Use it whenever you would use the normal `malloc`.
//!

mod fixed_buffer;
mod malloc;
mod pool;
mod string;
mod vmem;

pub use fixed_buffer::*;
pub use malloc::*;
pub use pool::*;
pub use string::*;
pub use vmem::*;
