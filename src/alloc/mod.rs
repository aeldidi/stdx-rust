use core::{
    alloc::{self, Allocator},
    cell::UnsafeCell,
    marker::PhantomData,
    ptr::{self, NonNull},
    slice,
};

pub mod malloc;
pub mod pool;
pub mod vmem;

/// An allocator backed by some `[u8]` which simply bumps a pointer within that
/// buffer to allocate memory.
///
/// [Allocator::realloc] and [Allocator::dealloc] are implemented such that
/// they are a no-op unless called with the last allocated pointer.
pub struct FixedBufferAllocator<'a> {
    /// There's a reason these aren't [core::ptr::NonNull] pointers. Namely,
    /// empty slices are represented as `{ptr: nullptr, size: 0}`, and making
    /// a FixedBufferAllocator from an empty slice is perfectly OK, since it'll
    /// just result in a [FixedBufferAllocator] which always returns
    /// [OutOfMemory] when it allocates.
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
    /// Constructs a [FixedFixedBufferAllocator] given a pointer to the
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
