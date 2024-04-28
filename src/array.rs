use core::{alloc::Allocator, marker::PhantomData, ptr::NonNull};

/// A dynamic array type whose elements are placed in contiguous memory.
///
/// # Why this instead of [Vec]?
///
/// Since the API for [Vec] was not designed with custom allocators in mind,
/// it's very clunky to use them with it. Specifically, placing any vectors
/// with custom allocators in other data structures who also might want to use
/// custom allocators require adding a type parameter for the [Allocator]. This
/// isn't really done most of the time though, meaning no one really does it.
///
/// So since `stdx` is meant to have custom allocators be a first-class
/// feature, the API will not be exactly the same, justifying the difference in
/// name.
pub struct Array<T> {
    arr: RawArray<T>,
    alloc: dyn Allocator,
}

/// An [Array<T>] without the allocator contained inside. All methods on this
/// will take an allocator as a parameter. Care must be taken to ensure the
/// same allocator is used each time.
///
/// This is similar to how [Vec] takes an [Allocator] as a parameter, but with
/// the caveat that the allocator is not stored anywhere and is assumed to be
/// stored somewhere else.
///
/// The point of this is to save storage space when an [Array<T>] is supposed
/// to be stored in a struct.
pub struct SubArray<T, A: Allocator> {
    arr: RawArray<T>,
    _marker: PhantomData<A>,
}

struct RawArray<T> {
    data: NonNull<*mut T>,
    capacity: usize,
    length: usize,
}

/// Controls the state of one or more `*mut T` pointers to enable use of the
/// Struct-of-Arrays pattern.
///
/// Since it's hard to safely write data structures making use of this pattern
/// without derive (and even then its clunky), [MultiArray] is provided to
/// make it easy for you to put one together yourself without much friction,
/// although we don't guarantee total safety.
///
/// So you'll simply call
pub struct MultiArray {
    capacity: usize,
    length: usize,
}
