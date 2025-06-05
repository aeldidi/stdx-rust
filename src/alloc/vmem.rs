use core::{
    alloc::{self, AllocError, Allocator},
    ptr::NonNull,
};

use super::FixedBufferAllocator;

/// An allocator which reserves a range of virtual memory with the size given,
/// only committing it as needed.
pub struct VirtualMemoryAllocator {
    addr: *const u8,
    size: usize,
    fba: FixedBufferAllocator<'static>,
}

impl VirtualMemoryAllocator {
    /// Reserves `size` bytes of virtual memory and returns a
    /// [FixedBufferAllocator] using that memory.
    pub fn new(
        size: usize,
    ) -> Result<VirtualMemoryAllocator, alloc::AllocError> {
        unsafe { internal::virtual_memory_alloc(size) }.map(|addr| {
            VirtualMemoryAllocator {
                addr,
                size,
                fba: unsafe {
                    FixedBufferAllocator::from_raw_parts(addr, size)
                },
            }
        })
    }
}

impl Drop for VirtualMemoryAllocator {
    #[inline(always)]
    fn drop(&mut self) {
        unsafe { internal::virtual_memory_free(self.addr, self.size) }
    }
}

unsafe impl Allocator for VirtualMemoryAllocator {
    #[inline(always)]
    fn allocate(
        &self,
        layout: alloc::Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        self.fba.allocate(layout)
    }

    #[inline(always)]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: alloc::Layout) {
        self.fba.deallocate(ptr, layout)
    }
}

#[cfg(unix)]
mod internal {
    use core::{alloc::AllocError, ffi::c_int, ptr};

    const PROT_READ: c_int = 1 << 0;
    const PROT_WRITE: c_int = 1 << 1;

    const MAP_PRIVATE: c_int = 1 << 1;
    #[cfg(any(
        target_os = "macos",
        target_os = "openbsd",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "dragonfly",
    ))]
    const MAP_ANON: c_int = 1 << 12;
    #[cfg(target_os = "linux")]
    const MAP_ANON: c_int = 1 << 5;

    const MAP_FAILED: usize = usize::MAX;

    #[link(name = "c")]
    extern "C" {
        fn mmap(
            addr: *mut u8,
            size: usize,
            prot: c_int,
            flags: c_int,
            fildes: c_int,
            offset: isize,
        ) -> *mut u8;
        fn munmap(addr: *const u8, len: usize) -> c_int;
    }

    #[inline(always)]
    pub unsafe fn virtual_memory_alloc(
        size: usize,
    ) -> Result<*mut u8, AllocError> {
        let ret = mmap(
            ptr::null_mut(),
            size,
            PROT_READ | PROT_WRITE,
            MAP_PRIVATE | MAP_ANON,
            -1,
            0,
        );
        if ret.addr() == MAP_FAILED {
            Err(AllocError)
        } else {
            Ok(ret)
        }
    }

    #[inline(always)]
    pub unsafe fn virtual_memory_free(addr: *const u8, size: usize) {
        munmap(addr, size);
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     fn layout(size: usize, align: usize) -> alloc::Layout {
//         alloc::Layout::from_size_align(size, align).unwrap()
//     }

//     /// The purpose of this test is not to evaluate if the system's
//     /// virtual_malloc/free work, rather, its just to ensure we can use the
//     /// functions successfully.
//     #[test]
//     fn virutal_memory_works() {
//         let vm = VirtualMemoryAllocator::new(1);
//         assert!(vm.is_ok());
//         let vm = vm.unwrap();
//         let result = vm.allocate(layout(1, 1));
//         assert!(result.is_ok());
//         let alloc = result.unwrap();
//         unsafe { alloc.cast::<u8>().as_ptr().write(0) };
//         assert_eq!(alloc.len(), 1);
//         unsafe { vm.deallocate(alloc.cast(), layout(1, 1)) };
//     }
// }
