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
use core::{
    alloc::{self, Allocator},
    cell::UnsafeCell,
    marker::PhantomData,
    ptr::{self, NonNull},
    slice,
};

mod malloc;
mod pool;
mod vmem;

pub use malloc::*;
pub use pool::*;
pub use vmem::*;

/// An allocator backed by some `[u8]` which simply bumps a pointer within that
/// buffer to allocate memory.
///
/// [Allocator::grow], [Allocator::shrink] and [Allocator::deallocate] are
/// implemented such that they are a no-op unless called with the last
/// allocated pointer.
pub struct FixedBufferAllocator<'a> {
    /// There's a reason these aren't [core::ptr::NonNull] pointers. Namely,
    /// empty slices are represented as `{ptr: nullptr, size: 0}`, and making
    /// a [FixedBufferAllocator] from an empty slice is perfectly OK, since
    /// it'll just result in a [FixedBufferAllocator] which always returns
    /// [alloc::AllocError] when it allocates.
    ///
    /// However, it also means we lose that static guarantee that the pointer's
    /// aren't `null`. In practice this doesn't mean much since that's just a
    /// single invalid pointer value and we're already doing all the necessary
    /// leg work to make this safe to use.
    begin: UnsafeCell<*mut u8>,
    /// `end` points to one byte past the end of the buffer. In other words, if
    /// `begin >= end`, `begin` does not point to valid memory.
    end: *const u8,
    /// Just so Rust knows that the FixedBufferAllocator's lifetime is tied to
    /// some other memory buffer.
    _marker: PhantomData<&'a u8>,
}

unsafe impl<'a> Send for FixedBufferAllocator<'a> {}

impl<'a> FixedBufferAllocator<'a> {
    /// Constructs a [FixedBufferAllocator] given a pointer to the
    /// beginning and the end of the memory range to allocate from.
    ///
    /// # Safety
    ///
    /// Behavior is undefined if any of the following conditions are violated:
    ///
    /// - `begin` must not be `null`.
    /// - `begin` must point to `len` bytes of readable and writable memory.
    /// - The memory referenced by `begin` must not be accessed through any
    ///   other pointer for the duration of the lifetime `'a`. Both read and
    ///   write accesses are forbidden.
    pub unsafe fn from_raw_parts(
        begin: *mut u8,
        len: usize,
    ) -> FixedBufferAllocator<'a> {
        let slice = slice::from_raw_parts_mut(begin, len);
        FixedBufferAllocator::from_slice(slice)
    }

    /// Creates a [FixedBufferAllocator] using the given slice as its backing
    /// memory.
    pub fn from_slice(mem: &'a mut [u8]) -> FixedBufferAllocator<'a> {
        FixedBufferAllocator {
            begin: UnsafeCell::new(mem.as_mut_ptr()),
            end: unsafe { mem.as_ptr().add(mem.len()) },
            _marker: PhantomData,
        }
    }
}

unsafe impl<'a> Allocator for FixedBufferAllocator<'a> {
    fn allocate(
        &self,
        layout: alloc::Layout,
    ) -> Result<NonNull<[u8]>, alloc::AllocError> {
        let size = layout.size();
        let align = layout.align();
        let begin = unsafe { *self.begin.get() };
        // SAFETY: We know that this won't overflow since alloc::Layout says
        //         that the size (after being aligned) will not exceed
        //         isize::MAX.
        let begin_aligned = (begin.addr() + (align - 1)) & !(align - 1);
        // SAFETY: We know that isize + isize <= usize::MAX so as a pointer's
        //         address, this should never overflow.
        let new_begin = begin.with_addr(begin_aligned + size);
        if self.end.addr().checked_sub(new_begin.addr()).is_none() {
            return Err(alloc::AllocError);
        }

        unsafe {
            self.begin.get().write(new_begin);
            Ok(NonNull::new_unchecked(slice::from_raw_parts_mut(
                begin.with_addr(begin_aligned),
                size,
            )))
        }
    }

    // TODO: do something similar for shrink
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: alloc::Layout,
        new_layout: alloc::Layout,
    ) -> Result<NonNull<[u8]>, alloc::AllocError> {
        if old_layout.align() != new_layout.align() {
            let new_ptr = self.allocate(new_layout)?;
            ptr::copy_nonoverlapping(
                ptr.as_ptr(),
                new_ptr.as_ptr() as *mut u8,
                old_layout.size(),
            );
            self.deallocate(ptr, old_layout);
            return Ok(new_ptr);
        }

        let begin = *self.begin.get();
        let old_size = old_layout.size();
        let new_size = new_layout.size();
        let align = new_layout.align();

        // SAFETY: A condition of this function is that ptr was previously
        //         allocated, meaning we don't have to worry about the case
        //         where this underflows.
        let prev_begin = (begin.addr() - old_size) & !(align - 1);
        if ptr.as_ptr().cast_const() != begin.with_addr(prev_begin) {
            // We can only resize if it was the last thing we allocated.
            return Err(alloc::AllocError);
        }

        // SAFETY: Layout guarantees the following: sizes are less than or
        //         equal to isize::MAX, new_layout.size() >= old_layout.size().
        //         These mean this shouldn't overflow.
        let difference = new_size as isize - old_size as isize;
        if self.end.byte_offset_from(ptr.as_ptr().offset(difference)) < 0 {
            return Err(alloc::AllocError);
        }

        self.begin.get().write(begin.offset(difference));
        Ok(NonNull::new_unchecked(slice::from_raw_parts_mut(
            begin.with_addr(prev_begin),
            new_layout.size(),
        )))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: alloc::Layout) {
        let begin = *self.begin.get();
        let size = layout.size();
        let align = layout.align();

        // SAFETY: A condition of this function is that ptr was previously
        //         allocated, meaning we don't have to worry about the case
        //         where this underflows.
        let prev_begin = (begin.addr() - size) & !(align - 1);
        if ptr.as_ptr().cast_const() != begin.with_addr(prev_begin) {
            // We can only deallocate if it was the last thing we allocated.
            return;
        }

        self.begin.get().write(begin.with_addr(prev_begin));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer_is_aligned_to<T: ?Sized>(ptr: *const T, align: usize) -> bool {
        ptr.addr() & (align - 1) == 0
    }

    fn layout(size: usize, align: usize) -> alloc::Layout {
        alloc::Layout::from_size_align(size, align).unwrap()
    }

    fn remaining_size(fba: &FixedBufferAllocator<'_>) -> usize {
        fba.end.addr() - unsafe { *fba.begin.get() }.addr()
    }

    #[test]
    fn fba_alloc_result_length_is_correct() {
        let mut buffer = [0u8; 8];
        let fba = FixedBufferAllocator::from_slice(&mut buffer);

        let result = fba.allocate(layout(1, 1));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
        let result = fba.allocate(layout(7, 1));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 7);
        assert_eq!(remaining_size(&fba), 0);
    }

    #[test]
    fn fba_alloc_alignment_is_correct() {
        let mut buffer = [0u8; 8];
        let fba = FixedBufferAllocator::from_slice(&mut buffer);
        let result = fba.allocate(layout(1, 4));
        assert!(result.is_ok());
        assert!(pointer_is_aligned_to(result.unwrap().as_ptr(), 4));
    }

    #[test]
    fn fba_alloc_err_begin_eq_end() {
        let mut buffer = [0u8; 1];
        let fba = FixedBufferAllocator::from_slice(&mut buffer);
        assert!(fba.allocate(layout(1, 1)).is_ok());
        assert_eq!(remaining_size(&fba), 0);
        assert!(fba.allocate(layout(1, 1)).is_err());
    }

    #[test]
    fn fba_alloc_err_begin_greater_than_end() {
        let mut buffer = [0u8; 1];
        let fba = FixedBufferAllocator::from_slice(&mut buffer);
        assert!(fba.allocate(layout(2, 1)).is_err());
    }

    #[test]
    fn fba_dealloc_works() {
        let mut buffer = [0u8; 2];
        let fba = FixedBufferAllocator::from_slice(&mut buffer);

        let size = remaining_size(&fba);

        let alloc1 = fba.allocate(layout(1, 1));
        assert!(alloc1.is_ok());
        let alloc2 = fba.allocate(layout(1, 1));
        assert!(alloc2.is_ok());
        let alloc1 = alloc1.unwrap();
        let alloc2 = alloc2.unwrap();

        assert_eq!(remaining_size(&fba), 0);

        unsafe { fba.deallocate(alloc1.cast(), layout(1, 1)) };
        assert_eq!(remaining_size(&fba), 0);

        unsafe { fba.deallocate(alloc2.cast(), layout(1, 1)) };
        assert_eq!(remaining_size(&fba), alloc1.len());

        unsafe { fba.deallocate(alloc1.cast(), layout(1, 1)) };
        assert_eq!(remaining_size(&fba), size);
    }

    #[test]
    fn fba_realloc_works() {
        let mut buffer = [0u8; 3];
        let fba = FixedBufferAllocator::from_slice(&mut buffer);

        let alloc1 = fba.allocate(layout(1, 1));
        assert!(alloc1.is_ok());
        let alloc2 = fba.allocate(layout(1, 1));
        assert!(alloc2.is_ok());
        let alloc1 = alloc1.unwrap();
        let alloc2 = alloc2.unwrap();

        assert_eq!(remaining_size(&fba), 1);

        assert!(unsafe {
            fba.grow(alloc1.cast(), layout(1, 1), layout(2, 1))
        }
        .is_err());
        assert_eq!(remaining_size(&fba), 1);

        assert!(unsafe {
            fba.grow(alloc2.cast(), layout(1, 1), layout(2, 1))
        }
        .is_ok());
        assert_eq!(remaining_size(&fba), 0);
    }
}
