use core::{
    alloc::{self, AllocError, Allocator},
    ffi::c_int,
    mem,
    ptr::{self, NonNull},
    slice,
};

#[link(name = "c")]
extern "C" {
    fn posix_memalign(
        memptr: *mut *mut u8,
        alignment: usize,
        size: usize,
    ) -> c_int;
    fn free(ptr: *mut u8);
}

/// An [Allocator] implementation using the system C allocator.
pub struct Mallocator;

unsafe impl Send for Mallocator {}
unsafe impl Sync for Mallocator {}

unsafe impl Allocator for Mallocator {
    #[inline(always)]
    fn allocate(
        &self,
        layout: alloc::Layout,
    ) -> Result<NonNull<[u8]>, alloc::AllocError> {
        let align =
            layout.align().next_multiple_of(mem::align_of::<*mut u8>());
        let mut memptr = ptr::null_mut();
        let ret = unsafe {
            posix_memalign(ptr::addr_of_mut!(memptr), align, layout.size())
        };
        if ret != 0 {
            return Err(AllocError);
        }

        unsafe {
            Ok(NonNull::new_unchecked(slice::from_raw_parts_mut(
                memptr,
                layout.size(),
            )))
        }
    }

    #[inline(always)]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, _: alloc::Layout) {
        free(ptr.as_ptr());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLOC: Mallocator = Mallocator;

    fn layout(size: usize, align: usize) -> alloc::Layout {
        alloc::Layout::from_size_align(size, align).unwrap()
    }

    /// The purpose of this test is not to evaluate if the system's malloc/free
    /// work, rather, its just to ensure we can use the functions successfully.
    #[test]
    fn malloc_works() {
        let result = ALLOC.allocate(layout(1, 1));
        assert!(result.is_ok());
        let alloc = result.unwrap();
        assert_eq!(alloc.len(), 1);
        unsafe { ALLOC.deallocate(alloc.cast(), layout(1, 1)) };
    }
}
