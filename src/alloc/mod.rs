use core::{marker::PhantomData, ptr::NonNull, slice};

pub mod malloc;

/// This just exists to make the return type of [Allocator::alloc] nicer.
#[derive(Debug)]
pub struct OutOfMemory;

/// A trait representing a generic memory allocator.
pub trait Allocator {
    /// Tries to allocate at least `size` bytes of data with the specified
    /// alignment. Returns a newly allocated slice of bytes whose size is the
    /// actual number of bytes allocated.
    ///
    /// # Safety
    ///
    /// Behavior is undefined if any of the following conditions are violated:
    ///
    /// - The given `size` is positive
    /// - The given `align` is a positive power of 2.
    /// - `size`, when rounded up to the nearest multiple of `align`, must be
    ///   less than or equal to [isize::MAX].
    unsafe fn alloc(&mut self, size: isize, align: isize) -> Result<NonNull<[u8]>, OutOfMemory>;

    /// Attempts to expand a block of memory returned by [Allocator::alloc] in
    /// place to the given size.
    ///
    /// If the request was not possible, this function returns `false`.
    ///
    /// # Safety
    ///
    /// Behavior is undefined if any of the following conditions are violated:
    ///
    /// - The given `ptr` must be a pointer returned from a previous call to
    ///   [Allocator::alloc].
    /// - The given `size` must be a number that is either equal to or between
    ///   the requested allocation size and the actual allocation size
    ///    returned by [Allocator::alloc].
    /// - The given `new_size` must be positive.
    /// - The given alignment must be the alignment specified when `ptr` was
    ///   allocated using [Allocator::alloc].
    /// - The given `ptr` must not have been previously deallocated using this
    ///   method.
    #[allow(unused_variables)]
    unsafe fn realloc(
        &mut self,
        ptr: NonNull<u8>,
        old_size: isize,
        align: isize,
        new_size: isize,
    ) -> bool {
        false
    }

    /// Deallocates the memory referenced by `ptr` with the given size and
    /// alignment.
    ///
    /// # Safety
    ///
    /// Behavior is undefined if any of the following conditions are violated:
    ///
    /// - The given `ptr` must be a pointer returned from a previous call to
    ///   [Allocator::alloc].
    /// - The given `size` must be a number that is either equal to or between
    ///   the requested allocation size and the actual allocation size
    ///    returned by [Allocator::alloc].
    /// - The given alignment must be the alignment specified when `ptr` was
    ///   allocated using [Allocator::alloc].
    /// - The given `ptr` must not have been previously deallocated using this
    ///   method.
    unsafe fn dealloc(&mut self, ptr: NonNull<u8>, size: isize, align: isize);
}

/// An allocator backed by some `[u8]` which simply bumps a pointer within that
/// buffer to allocate memory.
///
/// [Allocator::realloc] and [Allocator::dealloc] are implemented such that
/// they are a no-op unless called with the last allocated pointer.
#[derive(Copy, Clone)]
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
    begin: *mut u8,
    /// `end` points to one byte past the end of the buffer. In other words, if
    /// `begin >= end`, `begin` does not point to valid memory.
    end: *mut u8,
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
    pub unsafe fn from_raw_parts(begin: *mut u8, len: usize) -> FixedBufferAllocator<'a> {
        let slice = slice::from_raw_parts_mut(begin, len);
        FixedBufferAllocator::from_slice(slice)
    }

    /// Creates a [FixedBufferAllocator] using the given slice as its backing
    /// memory.
    pub fn from_slice(mem: &'a mut [u8]) -> FixedBufferAllocator<'a> {
        FixedBufferAllocator {
            begin: mem.as_mut_ptr(),
            end: unsafe { mem.as_mut_ptr().add(mem.len()) },
            _marker: PhantomData,
        }
    }
}

impl<'a> Allocator for FixedBufferAllocator<'a> {
    #[inline(always)]
    unsafe fn alloc(&mut self, size: isize, align: isize) -> Result<NonNull<[u8]>, OutOfMemory> {
        assert!(size > 0);
        assert!(align > 0);
        assert_eq!(align & (align - 1), 0); // ensure align is a power of 2

        let begin = self.begin;
        // SAFETY: We know that this won't overflow isize::MAX since that's a
        //         safety requirement.
        let begin_aligned = (begin.addr() as isize + (align - 1)) & !(align - 1);
        // SAFETY: We know that isize + isize < usize::MAX so as a pointer's
        //         address, this should never overflow.
        let new_begin = self.begin.with_addr(begin_aligned as usize + size as usize);
        if new_begin.addr() > self.end.addr() {
            return Err(OutOfMemory);
        }

        self.begin = new_begin;
        Ok(NonNull::new_unchecked(slice::from_raw_parts_mut(
            begin.with_addr(begin_aligned as usize),
            size as usize,
        )))
    }

    #[inline(always)]
    unsafe fn realloc(
        &mut self,
        ptr: NonNull<u8>,
        old_size: isize,
        align: isize,
        new_size: isize,
    ) -> bool {
        assert!(old_size > 0);
        assert!(align > 0);
        assert_eq!(align & (align - 1), 0); // ensure align is a power of 2
        assert!(new_size > 0);

        // SAFETY: A condition of this function is that ptr was previously
        //         allocated, meaning we don't have to worry about the case
        //         where this underflows.
        let prev_begin = (self.begin.addr() as isize - old_size) & !(align - 1);
        if ptr.as_ptr() != self.begin.with_addr(prev_begin as usize) {
            // We can only resize if it was the last thing we allocated.
            return false;
        }

        // SAFETY: Since we require that both new_size and old_size are
        // positive, this can never underflow.
        let difference = new_size - old_size;
        self.begin = self.begin.offset(difference);
        true
    }

    #[inline(always)]
    unsafe fn dealloc(&mut self, ptr: NonNull<u8>, size: isize, align: isize) {
        assert!(size > 0);
        assert!(align > 0);
        assert_eq!(align & (align - 1), 0); // ensure align is a power of 2

        // SAFETY: A condition of this function is that ptr was previously
        //         allocated, meaning we don't have to worry about the case
        //         where this underflows.
        let prev_begin = (self.begin.addr() as isize - size) & !(align - 1);
        if ptr.as_ptr() != self.begin.with_addr(prev_begin as usize) {
            // We can only deallocate if it was the last thing we allocated.
            return;
        }

        self.begin = self.begin.with_addr(prev_begin as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pointer_is_aligned_to<T: ?Sized>(ptr: *const T, align: usize) -> bool {
        ptr.addr() & (align - 1) == 0
    }

    #[test]
    fn fba_alloc_result_length_is_correct() {
        let mut buffer = [0u8; 8];
        let mut fba = FixedBufferAllocator::from_slice(&mut buffer);
        let result = unsafe { fba.alloc(1, 1) };
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
        let result = unsafe { fba.alloc(7, 1) };
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 7);
    }

    #[test]
    fn fba_alloc_alignment_is_correct() {
        let mut buffer = [0u8; 8];
        let mut fba = FixedBufferAllocator::from_slice(&mut buffer);
        let result = unsafe { fba.alloc(1, 4) };
        assert!(result.is_ok());
        assert!(pointer_is_aligned_to(result.unwrap().as_ptr(), 4));
    }

    #[test]
    fn fba_alloc_err_begin_eq_end() {
        let mut buffer = [0u8; 1];
        let mut fba = FixedBufferAllocator::from_slice(&mut buffer);
        assert!(unsafe { fba.alloc(1, 1) }.is_ok());
        assert_eq!(fba.begin.addr(), fba.end.addr());
        assert!(unsafe { fba.alloc(1, 1) }.is_err());
    }

    #[test]
    fn fba_alloc_err_begin_greater_than_end() {
        let mut buffer = [0u8; 1];
        let mut fba = FixedBufferAllocator::from_slice(&mut buffer);
        assert!(unsafe { fba.alloc(2, 1) }.is_err());
    }

    #[test]
    fn fba_dealloc_works() {
        let mut buffer = [0u8; 2];
        let mut fba = FixedBufferAllocator::from_slice(&mut buffer);

        let size = fba.end.addr() - fba.begin.addr();

        let alloc1 = unsafe { fba.alloc(1, 1) };
        assert!(alloc1.is_ok());
        let alloc2 = unsafe { fba.alloc(1, 1) };
        assert!(alloc2.is_ok());
        let alloc1 = alloc1.unwrap();
        let alloc2 = alloc2.unwrap();

        assert_eq!(fba.end.addr() - fba.begin.addr(), 0);

        unsafe { fba.dealloc(alloc1.cast(), 1, 1) };
        assert_eq!(fba.end.addr() - fba.begin.addr(), 0);

        unsafe { fba.dealloc(alloc2.cast(), 1, 1) };
        assert_eq!(fba.end.addr() - fba.begin.addr(), alloc1.len());

        unsafe { fba.dealloc(alloc1.cast(), 1, 1) };
        assert_eq!(fba.end.addr() - fba.begin.addr(), size);
    }

    #[test]
    fn fba_realloc_works() {
        let mut buffer = [0u8; 3];
        let mut fba = FixedBufferAllocator::from_slice(&mut buffer);

        let alloc1 = unsafe { fba.alloc(1, 1) };
        assert!(alloc1.is_ok());
        let alloc2 = unsafe { fba.alloc(1, 1) };
        assert!(alloc2.is_ok());
        let alloc1 = alloc1.unwrap();
        let alloc2 = alloc2.unwrap();

        assert_eq!(fba.end.addr() - fba.begin.addr(), 1);

        assert!(!unsafe { fba.realloc(alloc1.cast(), 1, 1, 2) });
        assert_eq!(fba.end.addr() - fba.begin.addr(), 1);

        assert!(unsafe { fba.realloc(alloc2.cast(), 1, 1, 2) });
        assert_eq!(fba.end.addr() - fba.begin.addr(), 0);
    }
}
