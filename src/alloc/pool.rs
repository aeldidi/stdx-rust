use core::{alloc::Allocator, marker::PhantomData};

/// An allocator which allocates objects of only a single type, but does so in
/// a way that they will all be located in contiguous memory.
///
/// Additionally, memory is only deallocated when the pool goes out of scope.
pub struct Pool<'a, T> {
    _alloc: &'a dyn Allocator,
    _marker: PhantomData<T>,
}

// TODO: implement this
