//! Things which are not guaranteed to work on every rust version.
//!
//! Nothing here is guaranteed to be availible in every rust version. I'll try
//! to test everything when a new version comes out, but if something breaks,
//! it's gonna be tough to find out why. You have been warned.
#![cfg(not(version("1.80")))] // version 1.79 or lower

#[cfg(doc)]
use core::{alloc::Allocator, mem};

/// A `&dyn Trait` object, allowing a struct to "own" a type-erased `dyn Trait`.
/// A caveat is that the struct needs to manually ensure that destructors are
/// called and all that.
///
/// You can convert a `&dyn Trait` to one of these by using [mem::transmute].
#[repr(C)]
#[derive(Copy, Clone)]
pub struct TraitObject {
    pub data: *mut (),
    pub vtable: *mut (),
}

/// The object header for the `vtable` element of [TraitObject].
///
/// Following this header are the other function pointers, but since they
/// differ between the specific trait which the [TraitObject] is representing,
/// we don't include a way to get those.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct TraitObjectVTableHeader {
    pub drop: fn(*mut ()),
    pub size: usize,
    pub align: usize,
}

/// The in-memory representation of `&[T]`.
///
/// You can convert a `&[T]` to one of these by using [mem::transmute].
#[repr(C)]
#[derive(Copy, Clone)]
pub struct Slice<T> {
    pub data: *mut T,
    pub len: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    use core::mem;

    #[test]
    #[cfg_attr(miri, ignore)]
    fn trait_obj_works() {
        let x = 0u128;
        let traitobj: TraitObject = unsafe { mem::transmute(&x as &dyn Send) };
        let vtableheader: &TraitObjectVTableHeader =
            unsafe { mem::transmute(traitobj.vtable) };
        assert_eq!(vtableheader.align, mem::align_of::<u128>());
        assert_eq!(vtableheader.size, mem::size_of::<u128>());
        // Just ensuring it doesn't drop when we call this.
        unsafe { (vtableheader.drop)(mem::transmute(vtableheader)) };
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn slice_obj_works() {
        let x = [0u8; 127];
        let sliceobj: Slice<u8> = unsafe { mem::transmute(&x[..]) };
        let data = sliceobj.data;
        assert_eq!(sliceobj.len, 127);
        let sliceobj: Slice<u8> = unsafe { mem::transmute(&x[..2]) };
        assert_eq!(data, sliceobj.data);
        assert_eq!(sliceobj.len, 2);
    }
}
