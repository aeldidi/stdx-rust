#![allow(clippy::len_without_is_empty)]

use core::{
    alloc::{self, Allocator, Layout},
    cmp,
    marker::PhantomData,
    mem, ops,
    ptr::{self, slice_from_raw_parts_mut, NonNull},
    slice,
};

#[inline(always)]
fn reserve<T>(
    data: &mut NonNull<T>,
    length: &mut usize,
    capacity: &mut usize,
    additional: usize,
    alloc: &impl Allocator,
) -> Result<(), alloc::AllocError> {
    if *capacity == 0 {
        // Allocate for the first time.
        let size = match mem::size_of::<T>().checked_mul(additional) {
            Some(sz) => sz,
            None => return Err(alloc::AllocError),
        };
        let layout = match Layout::from_size_align(size, mem::align_of::<T>())
        {
            Ok(layout) => layout,
            Err(_) => return Err(alloc::AllocError),
        };
        let result = alloc.allocate(layout)?;
        let size = result.len();
        *data = result.cast();
        *capacity = size;
        *length = 0;
        return Ok(());
    }
    let old_size = match mem::size_of::<T>().checked_mul(*capacity) {
        Some(sz) => sz,
        None => return Err(alloc::AllocError),
    };
    let new_size = {
        let tmp = match capacity.checked_add(additional) {
            Some(x) => x,
            None => return Err(alloc::AllocError),
        };
        match mem::size_of::<T>().checked_mul(tmp) {
            Some(x) => x,
            None => return Err(alloc::AllocError),
        }
    };

    let old_layout =
        match Layout::from_size_align(old_size, mem::align_of::<T>()) {
            Ok(l) => l,
            Err(_) => return Err(alloc::AllocError),
        };
    let new_layout =
        match Layout::from_size_align(new_size, mem::align_of::<T>()) {
            Ok(l) => l,
            Err(_) => return Err(alloc::AllocError),
        };
    // SAFETY: we know ptr is currently allocated, we know old_layout is the
    //         layout of ptr, and we know
    //         new_layout.size() >= old_layout.size().
    let result = unsafe { alloc.grow(data.cast(), old_layout, new_layout)? };
    let size = result.len();
    *data = result.cast();
    *capacity = size;
    Ok(())
}

#[inline(always)]
unsafe fn push<T>(
    data: &mut NonNull<T>,
    length: &mut usize,
    capacity: &mut usize,
    value: T,
    alloc: &impl Allocator,
) -> Result<(), alloc::AllocError> {
    if *capacity == 0 {
        reserve(data, length, capacity, 16, alloc)?;
    } else {
        reserve(data, length, capacity, 1, alloc)?;
    }

    let offset = match mem::size_of::<T>().checked_mul(*capacity) {
        Some(x) => {
            if x > isize::MAX as usize {
                return Err(alloc::AllocError);
            }
            x as isize
        }
        None => return Err(alloc::AllocError),
    };
    *data.as_ptr().byte_offset(offset) = value;
    Ok(())
}

#[inline(always)]
fn with_capacity<T>(
    capacity: usize,
    alloc: &impl Allocator,
) -> Result<(NonNull<T>, usize), alloc::AllocError> {
    let layout = match Layout::from_size_align(
        mem::size_of::<T>() * capacity,
        mem::align_of::<T>(),
    ) {
        Ok(l) => l,
        Err(_) => return Err(alloc::AllocError),
    };
    let mem = alloc.allocate(layout)?;
    let capacity = mem.len();
    Ok((mem.cast(), capacity))
}

#[inline(always)]
const unsafe fn pop<T>(data: &mut NonNull<T>, length: &mut usize) -> T {
    *length -= 1;
    let offset = *length * mem::size_of::<T>();
    ptr::read(mem::transmute(data.as_ptr().byte_add(offset)))
}

#[inline(always)]
fn clear<T>(data: &mut NonNull<T>, length: &mut usize) {
    let elements = slice_from_raw_parts_mut(data.as_ptr(), *length);
    // SAFETY: we set the length to 0 before dropping all the elements because
    //         in the case where drop panics on an element, we don't want to
    //         try to drop the element again when dropping the array.
    unsafe {
        *length = 0;
        ptr::drop_in_place(elements)
    }
}

#[inline(always)]
fn append<T>(
    data: &mut NonNull<T>,
    length: &mut usize,
    capacity: &mut usize,
    alloc: &impl Allocator,
    other_data: NonNull<T>,
    other_length: usize,
) -> Result<(), alloc::AllocError> {
    reserve(data, length, capacity, other_length, alloc)?;
    for i in 0..other_length {
        // SAFETY: we can use unchecked arithmetic here because these things
        //         are already located at the computed offsets, meaning they
        //         can't overflow here.
        //
        //         For the write, we know it can't overflow since we already
        //         computed the size in reserve().
        unsafe {
            let value = ptr::read(
                other_data
                    .as_ptr()
                    .byte_add(i.unchecked_mul(mem::size_of::<T>())),
            );
            ptr::write(
                data.as_ptr().byte_add(
                    (i.unchecked_add(*length))
                        .unchecked_mul(mem::size_of::<T>()),
                ),
                value,
            );
        }
    }
    Ok(())
}

#[inline(always)]
const unsafe fn push_within_capacity<T>(
    data: &mut NonNull<T>,
    length: &mut usize,
    value: T,
) {
    let offset = mem::size_of::<T>().unchecked_mul(*length);
    let dst = data.byte_add(offset);
    ptr::write(dst.as_ptr(), value);

    *length += 1;
}

#[inline(always)]
unsafe fn insert_unchecked<T>(
    data: &mut NonNull<T>,
    length: &mut usize,
    capacity: &mut usize,
    alloc: &impl Allocator,
    index: usize,
    value: T,
) -> Result<(), alloc::AllocError> {
    if index == *length {
        return push(data, length, capacity, value, alloc);
    }

    reserve(data, length, capacity, 1, alloc)?;

    // SAFETY: At this point we know this won't overflow because
    //         1. This is the unsafe version of the function so we're
    //            already assuming index < self.len().
    //         2. Since reserve allocated successfully, we know the offset
    //            won't overflow.
    let offset = mem::size_of::<T>().unchecked_mul(index);
    let base = data.byte_add(offset);
    let count = mem::size_of::<T>().unchecked_mul(length.unchecked_sub(index));
    ptr::copy(
        base.as_ptr(),
        base.byte_add(mem::size_of::<T>()).as_ptr(),
        count,
    );

    ptr::write(base.as_ptr(), value);
    Ok(())
}

#[inline(always)]
const unsafe fn remove_unchecked<T>(
    data: &mut NonNull<T>,
    length: &mut usize,
    index: usize,
) -> T {
    // SAFETY: we know this is safe because the index is in bounds. We are
    //         just manually moving the object out.
    let result = {
        let offset = mem::size_of::<T>().unchecked_mul(index);
        let addr = data.byte_add(offset);
        let result = ptr::read(addr.as_ptr());
        ptr::copy(
            addr.byte_add(mem::size_of::<T>()).as_ptr(),
            addr.as_ptr(),
            length.unchecked_sub(index),
        );
        result
    };

    *length -= 1;

    result
}

#[inline(always)]
const unsafe fn swap_remove_unchecked<T>(
    data: &mut NonNull<T>,
    length: &mut usize,
    index: usize,
) -> T {
    // SAFETY: we know this is safe because the index is in bounds. We are
    //         just manually moving the object out.
    let result = unsafe {
        let offset = mem::size_of::<T>().unchecked_mul(index);
        let end_offset =
            mem::size_of::<T>().unchecked_mul(length.unchecked_sub(1));
        let addr = data.byte_add(offset);
        let end_addr = data.byte_add(end_offset);
        let result = ptr::read(addr.as_ptr());
        ptr::write(addr.as_ptr(), ptr::read(end_addr.as_ptr()));
        result
    };

    *length -= 1;

    result
}

#[inline(always)]
fn truncate<T>(data: &mut NonNull<T>, length: &mut usize, len: usize) {
    if *length <= len {
        return;
    }

    for i in (len - 1)..*length {
        // SAFETY: we know this is safe because we've already checked that
        //         the index is in bounds.
        unsafe {
            let offset = mem::size_of::<T>().unchecked_mul(i);
            let addr = data.as_ptr().map_addr(|a| a.unchecked_add(offset));
            ptr::drop_in_place(addr);
        }
    }
    *length = len;
}

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
pub struct Array<'a, T> {
    data: NonNull<T>,
    length: usize,
    capacity: usize,
    alloc: &'a dyn Allocator,
}

impl<T> ops::Deref for Array<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.data.as_ptr(), self.length) }
    }
}

impl<T> ops::DerefMut for Array<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { slice::from_raw_parts_mut(self.data.as_ptr(), self.length) }
    }
}

impl<'a, T> Array<'a, T> {
    /// # Safety
    ///
    /// This function is safe to use if the following is true:
    /// - If capacity is some x >= 0, data points to enough memory to contain x
    ///   elements of type T.
    /// - If length is some y >= 0, y <= x.
    /// - If length is 0, so is capacity.
    /// - If capacity is 0, data's value is irrelevant.
    #[inline]
    pub const unsafe fn from_raw_parts(
        data: NonNull<T>,
        capacity: usize,
        length: usize,
        alloc: &'a impl Allocator,
    ) -> Array<'a, T> {
        Array {
            data,
            length,
            capacity,
            alloc,
        }
    }

    /// Returns a new empty [Array] using the allocator `alloc`.
    #[inline]
    pub const fn new(alloc: &'a impl Allocator) -> Array<'a, T> {
        Array {
            data: NonNull::dangling(),
            length: 0,
            capacity: 0,
            alloc,
        }
    }

    /// Returns a new [Array] with the given capacity.
    #[inline]
    pub fn with_capacity(
        capacity: usize,
        alloc: &'a impl Allocator,
    ) -> Result<Array<'a, T>, alloc::AllocError> {
        let (data, capacity) = with_capacity(capacity, &alloc)?;
        Ok(Array {
            data,
            length: 0,
            capacity,
            alloc,
        })
    }

    /// Returns the length of the [Array].
    #[inline]
    pub const fn len(&self) -> usize {
        self.length
    }

    /// Returns the total number of elements the vector can hold without
    /// reallocating.
    #[inline]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Tries to reserve enough memory for at least `additional` extra elements
    /// to be appended to the end of the [Array]. That is, after calling
    /// this you can be sure that the next `additional` calls to
    /// [Array::push] will not allocate and complete in `O(1)` time.
    ///
    /// Returns an error if:
    ///
    /// - `mem::size_of::<T>() * self.capacity + additional` would overflow.
    ///
    /// - `mem::size_of::<T>() * self.capacity + additional > isize::MAX`.
    ///
    /// - An allocation failed.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    pub unsafe fn reserve(
        &mut self,
        additional: usize,
    ) -> Result<(), alloc::AllocError> {
        reserve(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            additional,
            &self.alloc,
        )
    }

    /// Appends an element to the back of the [Array].
    ///
    /// Returns an error if:
    ///
    /// - `mem::size_of::<T>() * self.capacity` would overflow.
    ///
    /// - `mem::size_of::<T>() * self.capacity > isize::MAX`.
    ///
    /// - `mem::size_of::<T>() * self.capacity + 1` would overflow.
    ///
    /// - An allocation failed.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    pub unsafe fn push(&mut self, value: T) -> Result<(), alloc::AllocError> {
        push(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            value,
            &self.alloc,
        )
    }

    /// Removes the last element from a vector and returns it, or [`None`] if
    /// it is empty.
    pub const fn pop(&mut self) -> Option<T> {
        if self.length == 0 {
            return None;
        }

        // SAFETY: we already checked that there is actually something to pop.
        Some(unsafe { pop(&mut self.data, &mut self.length) })
    }

    /// Clears and drops all the elements in the [Array] without freeing any
    /// memory.
    pub fn clear(&mut self) {
        clear(&mut self.data, &mut self.length)
    }

    /// Moves all elements out of `other` into `self`, leaving `other` empty.
    ///
    /// Returns an error if:
    ///
    /// - An allocation failure occurs.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    pub unsafe fn append(
        &mut self,
        other: &mut Array<T>,
    ) -> Result<(), alloc::AllocError> {
        append(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            &self.alloc,
            other.data,
            other.length,
        )
    }

    /// Appends an element to the end of the [Array] only if there is enough
    /// capacity to do so, otherwise the element is returned.
    ///
    /// Guaranteed to never allocate memory.
    pub const fn push_within_capacity(&mut self, value: T) -> Result<(), T> {
        if self.length >= self.capacity {
            return Err(value);
        }

        // SAFETY: we know this is safe because we've already checked that
        //         self.length is less than self.capacity.
        unsafe {
            push_within_capacity(&mut self.data, &mut self.length, value)
        };
        Ok(())
    }

    /// Inserts an element at the given `index` in the [Array], shifting
    /// everything after it to the right.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    pub unsafe fn insert(
        &mut self,
        index: usize,
        value: T,
    ) -> Result<Option<()>, alloc::AllocError> {
        if index > self.length {
            return Ok(None);
        }

        insert_unchecked(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            &self.alloc,
            index,
            value,
        )?;
        Ok(Some(()))
    }

    /// Inserts an element at the given `index` in the [Array], shifting
    /// everything after it to the right.
    ///
    /// # Safety
    ///
    /// This function is safe to use if the following is true:
    /// - `index` is within the bounds of the [Array].
    /// - The same allocator is used for all methods on this object.
    pub unsafe fn insert_unchecked(
        &mut self,
        index: usize,
        value: T,
    ) -> Result<(), alloc::AllocError> {
        insert_unchecked(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            &self.alloc,
            index,
            value,
        )
    }

    /// Removes and returns the element at the given `index`, shifting
    /// everything after it to the left.
    ///
    /// If the given `index` is out of bounds, returns [`Option::None`].
    pub const fn remove(&mut self, index: usize) -> Option<T> {
        if index == self.length - 1 {
            return Some(unsafe { pop(&mut self.data, &mut self.length) });
        }

        if index >= self.length {
            return None;
        }

        // SAFETY: this is safe because we already checked that the index is in
        //         bounds.
        Some(unsafe {
            remove_unchecked(&mut self.data, &mut self.length, index)
        })
    }

    /// Removes and returns the element at the given `index`, shifting
    /// everything after it to the left.
    ///
    /// # Safety
    ///
    /// This function is safe to use if the `index` is in bounds.
    pub const unsafe fn remove_unchecked(&mut self, index: usize) -> T {
        remove_unchecked(&mut self.data, &mut self.length, index)
    }

    /// Removes and returns the element at the given `index`, swapping it with
    /// the last element in the array.
    ///
    /// If the given `index` is out of bounds, returns [`Option::None`].
    pub const fn swap_remove(&mut self, index: usize) -> Option<T> {
        if index == self.length - 1 {
            return self.pop();
        }

        if index >= self.length {
            return None;
        }

        // SAFETY: we know this is safe because we've checked that the index is
        //         in bounds.
        Some(unsafe {
            swap_remove_unchecked(&mut self.data, &mut self.length, index)
        })
    }

    /// Removes and returns the element at the given `index`, swapping it with
    /// the last element in the array.
    ///
    /// If the given `index` is out of bounds, returns [`Option::None`].
    ///
    /// # Safety
    ///
    /// This function is safe to use if the `index` is in bounds.
    pub const unsafe fn swap_remove_unchecked(&mut self, index: usize) -> T {
        swap_remove_unchecked(&mut self.data, &mut self.length, index)
    }

    /// Truncates the [Array] to be less than or equal to the given `len`.
    /// If the [Array]'s length is greater than the given `len`, the extra
    /// elements are dropped.
    pub fn truncate(&mut self, len: usize) {
        truncate(&mut self.data, &mut self.length, len)
    }
}

/// An [Array] without the allocator stored inline. Useful for embedding in
/// other data structures. Most methods on this are unsafe since they assume
/// the same allocator is passed every time.
pub struct RawArray<T> {
    data: NonNull<T>,
    capacity: usize,
    length: usize,
}

impl<T> ops::Deref for RawArray<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.data.as_ptr(), self.length) }
    }
}

impl<T> ops::DerefMut for RawArray<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { slice::from_raw_parts_mut(self.data.as_ptr(), self.length) }
    }
}

impl<T> RawArray<T> {
    /// # Safety
    ///
    /// This function is safe to use if the following is true:
    /// - If capacity is some x >= 0, data points to enough memory to contain x
    ///   elements of type T.
    /// - If length is some y >= 0, y <= x.
    /// - If length is 0, so is capacity.
    /// - If capacity is 0, data's value is irrelevant.
    #[inline]
    pub const unsafe fn from_raw_parts(
        data: NonNull<T>,
        capacity: usize,
        length: usize,
    ) -> RawArray<T> {
        RawArray {
            data,
            capacity,
            length,
        }
    }

    /// Returns a new empty [RawArray]
    #[inline]
    pub const fn new() -> RawArray<T> {
        RawArray {
            data: NonNull::dangling(),
            length: 0,
            capacity: 0,
        }
    }

    /// Returns a new [RawArray] with the given capacity.
    #[inline]
    pub fn with_capacity(
        capacity: usize,
        alloc: impl Allocator,
    ) -> Result<RawArray<T>, alloc::AllocError> {
        let (data, capacity) = with_capacity(capacity, &alloc)?;
        Ok(RawArray {
            data,
            length: 0,
            capacity,
        })
    }

    /// Returns the length of the [RawArray].
    #[inline]
    pub const fn len(&self) -> usize {
        self.length
    }

    /// Returns the total number of elements the vector can hold without
    /// reallocating.
    #[inline]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Tries to reserve enough memory for at least `additional` extra elements
    /// to be appended to the end of the [RawArray]. That is, after calling
    /// this you can be sure that the next `additional` calls to
    /// [RawArray::push] will not allocate and complete in `O(1)` time.
    ///
    /// Returns an error if:
    ///
    /// - `mem::size_of::<T>() * self.capacity + additional` would overflow.
    ///
    /// - `mem::size_of::<T>() * self.capacity + additional > isize::MAX`.
    ///
    /// - An allocation failed.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    pub unsafe fn reserve(
        &mut self,
        additional: usize,
        alloc: impl Allocator,
    ) -> Result<(), alloc::AllocError> {
        reserve(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            additional,
            &alloc,
        )
    }

    /// Appends an element to the back of the [RawArray].
    ///
    /// Returns an error if:
    ///
    /// - `mem::size_of::<T>() * self.capacity` would overflow.
    ///
    /// - `mem::size_of::<T>() * self.capacity > isize::MAX`.
    ///
    /// - `mem::size_of::<T>() * self.capacity + 1` would overflow.
    ///
    /// - An allocation failed.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    pub unsafe fn push(
        &mut self,
        value: T,
        alloc: impl Allocator,
    ) -> Result<(), alloc::AllocError> {
        push(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            value,
            &alloc,
        )
    }

    /// Removes the last element from a vector and returns it, or [`None`] if
    /// it is empty.
    pub const fn pop(&mut self) -> Option<T> {
        if self.length == 0 {
            return None;
        }

        // SAFETY: we already checked that there is actually something to pop.
        Some(unsafe { pop(&mut self.data, &mut self.length) })
    }

    /// Clears and drops all the elements in the [RawArray] without freeing any
    /// memory.
    pub fn clear(&mut self) {
        clear(&mut self.data, &mut self.length)
    }

    /// Moves all elements out of `other` into `self`, leaving `other` empty.
    ///
    /// Returns an error if:
    ///
    /// - An allocation failure occurs.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    pub unsafe fn append(
        &mut self,
        other: &mut RawArray<T>,
        alloc: impl Allocator,
    ) -> Result<(), alloc::AllocError> {
        append(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            &alloc,
            other.data,
            other.length,
        )
    }

    /// Appends an element to the end of the [RawArray] only if there is enough
    /// capacity to do so, otherwise the element is returned.
    ///
    /// Guaranteed to never allocate memory.
    pub const fn push_within_capacity(&mut self, value: T) -> Result<(), T> {
        if self.length >= self.capacity {
            return Err(value);
        }

        // SAFETY: we know this is safe because we've already checked that
        //         self.length is less than self.capacity.
        unsafe {
            push_within_capacity(&mut self.data, &mut self.length, value)
        };
        Ok(())
    }

    /// Inserts an element at the given `index` in the [RawArray], shifting
    /// everything after it to the right.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    pub unsafe fn insert(
        &mut self,
        index: usize,
        value: T,
        alloc: impl Allocator,
    ) -> Result<Option<()>, alloc::AllocError> {
        if index > self.length {
            return Ok(None);
        }

        insert_unchecked(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            &alloc,
            index,
            value,
        )?;
        Ok(Some(()))
    }

    /// Inserts an element at the given `index` in the [RawArray], shifting
    /// everything after it to the right.
    ///
    /// # Safety
    ///
    /// This function is safe to use if the following is true:
    /// - `index` is within the bounds of the [RawArray].
    /// - The same allocator is used for all methods on this object.
    pub unsafe fn insert_unchecked(
        &mut self,
        index: usize,
        value: T,
        alloc: impl Allocator,
    ) -> Result<(), alloc::AllocError> {
        insert_unchecked(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            &alloc,
            index,
            value,
        )
    }

    /// Removes and returns the element at the given `index`, shifting
    /// everything after it to the left.
    ///
    /// If the given `index` is out of bounds, returns [`Option::None`].
    pub const fn remove(&mut self, index: usize) -> Option<T> {
        if index == self.length - 1 {
            return Some(unsafe { pop(&mut self.data, &mut self.length) });
        }

        if index >= self.length {
            return None;
        }

        // SAFETY: this is safe because we already checked that the index is in
        //         bounds.
        Some(unsafe {
            remove_unchecked(&mut self.data, &mut self.length, index)
        })
    }

    /// Removes and returns the element at the given `index`, shifting
    /// everything after it to the left.
    ///
    /// # Safety
    ///
    /// This function is safe to use if the `index` is in bounds.
    pub const unsafe fn remove_unchecked(&mut self, index: usize) -> T {
        remove_unchecked(&mut self.data, &mut self.length, index)
    }

    /// Removes and returns the element at the given `index`, swapping it with
    /// the last element in the array.
    ///
    /// If the given `index` is out of bounds, returns [`Option::None`].
    pub const fn swap_remove(&mut self, index: usize) -> Option<T> {
        if index == self.length - 1 {
            return self.pop();
        }

        if index >= self.length {
            return None;
        }

        // SAFETY: we know this is safe because we've checked that the index is
        //         in bounds.
        Some(unsafe {
            swap_remove_unchecked(&mut self.data, &mut self.length, index)
        })
    }

    /// Removes and returns the element at the given `index`, swapping it with
    /// the last element in the array.
    ///
    /// If the given `index` is out of bounds, returns [`Option::None`].
    ///
    /// # Safety
    ///
    /// This function is safe to use if the `index` is in bounds.
    pub const unsafe fn swap_remove_unchecked(&mut self, index: usize) -> T {
        swap_remove_unchecked(&mut self.data, &mut self.length, index)
    }

    /// Truncates the [RawArray] to be less than or equal to the given `len`.
    /// If the [RawArray]'s length is greater than the given `len`, the extra
    /// elements are dropped.
    pub fn truncate(&mut self, len: usize) {
        truncate(&mut self.data, &mut self.length, len)
    }
}

/// Controls the state of one or more `*mut T` pointers to enable use of the
/// Struct-of-Arrays pattern.
///
/// Since it's hard to safely write data structures making use of this pattern
/// without derive (and even then its clunky), [MultiArray] is provided to
/// make it easy for you to put one together yourself without much friction,
/// although we don't guarantee total safety.
///
/// So you'll simply call the array methods on [MultiArray], providing the type
/// as a struct. For example, a [MultiArray] managing 3 arrays with `i32`,
/// `String`, `SomeType` in them:
///
/// ```no_run
/// struct SomeDataStructure {
///     ints: NonNull<i32>,
///     strings: NonNull<String>,
///     sometypes: NonNull<SomeType>,
///     arr: MultiArray<3, (i32, String, SomeType)>
/// }
///
/// impl SomeDataStructure {
///     fn new() -> SomeDataStructure {
///         SomeDataStructure {        
///             ...,
///             arr: MultiArray::new((&mut ints, &mut strings, &mut sometypes)),
///         }
///     }
/// }
///
/// // in your code later...
/// fn some_function() {
///     (&mut ints, &mut strings, &mut sometypes) = arr.push(
///         (0i32, String::new(), SomeType::SomeValue)).unwrap();
/// }
/// ```
///
/// This is unsafe because I've been dissatisfied with the availible safe
/// solutions. Unfortunately Rust has no compile time reflection so unsafe is
/// as good as it's gonna get.
pub struct MultiArray<const N: usize, T: MultiArrayTypes<N>> {
    length: usize,
    capacity: usize,
    _marker: PhantomData<T>,
}

impl<const N: usize, T: MultiArrayTypes<N>> MultiArray<N, T> {
    /// # Safety
    ///
    /// This function is safe to use if the following is true:
    /// - If capacity is some x >= 0, data points to enough memory to contain x
    ///   elements of type T.
    /// - If length is some y >= 0, y <= x.
    /// - If length is 0, so is capacity.
    /// - If capacity is 0, data's value is irrelevant.
    #[inline]
    pub const unsafe fn from_raw_parts(
        capacity: usize,
        length: usize,
    ) -> MultiArray<N, T> {
        MultiArray {
            length,
            capacity,
            _marker: PhantomData,
        }
    }

    /// Returns a new empty [MultiArray]
    #[inline]
    pub const fn new() -> MultiArray<N, T> {
        MultiArray {
            length: 0,
            capacity: 0,
            _marker: PhantomData,
        }
    }

    /// Returns a new [MultiArray] with the given capacity, as well as an array
    /// containing the raw pointers to the allocated memory for each type.
    #[inline]
    pub fn with_capacity(
        capacity: usize,
        alloc: impl Allocator,
    ) -> Result<(MultiArray<N, T>, [*mut (); N]), alloc::AllocError> {
        let mut offsets = [0usize; N];

        let layout = {
            let mut result = Layout::new::<()>();
            for (i, (size, align)) in
                T::sizes_and_alignments().iter().enumerate()
            {
                offsets[i] = result.size();
                // SAFETY: we know this is safe because sizes_and_alignments
                //         just calls mem::size_of and mem::align_of for each
                //         type.
                let (layout, _) = match unsafe {
                    Layout::from_size_align_unchecked(*size, *align)
                }
                .repeat(capacity)
                {
                    Ok((l, s)) => (l, s),
                    Err(_) => return Err(alloc::AllocError),
                };

                let size = match result.size().checked_add(layout.size()) {
                    Some(x) => x,
                    None => return Err(alloc::AllocError),
                };
                result = unsafe {
                    Layout::from_size_align_unchecked(
                        size,
                        cmp::max(result.align(), *align),
                    )
                };
            }

            result
        };

        // TODO: allocate the memory, return the MultiArray structure, and then
        //       return each offsetted pointer.
        let data = alloc.allocate(layout)?;

        let ptrs = {
            let mut result = [ptr::null_mut::<()>(); N];

            for i in 0..N {
                // SAFETY: We know this is safe because we computed each offset
                //         from valid Layouts.
                result[i] =
                    unsafe { data.byte_add(offsets[i]).as_ptr().cast() };
            }

            result
        };

        Ok((
            MultiArray {
                length: 0,
                capacity,
                _marker: PhantomData,
            },
            ptrs,
        ))
    }

    /// Returns the length of the [MultiArray].
    #[inline]
    pub const fn len(&self) -> usize {
        self.length
    }

    /// Returns the total number of elements each array can hold without
    /// reallocating.
    #[inline]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Tries to reserve enough memory for at least `additional` extra elements
    /// to be appended to the end of the [MultiArray]. That is, after calling
    /// this you can be sure that the next `additional` calls to
    /// [MultiArray::push] will not allocate and complete in `O(1)` time.
    ///
    /// Returns an error if:
    ///
    /// - `mem::size_of::<T>() * self.capacity + additional` would overflow.
    ///
    /// - `mem::size_of::<T>() * self.capacity + additional > isize::MAX`.
    ///
    /// - An allocation failed.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    pub unsafe fn reserve(
        &mut self,
        additional: usize,
        alloc: impl Allocator,
    ) -> Result<(), alloc::AllocError> {
        reserve(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            additional,
            &alloc,
        )
    }

    /// Appends an element to the back of the [MultiArray].
    ///
    /// Returns an error if:
    ///
    /// - `mem::size_of::<T>() * self.capacity` would overflow.
    ///
    /// - `mem::size_of::<T>() * self.capacity > isize::MAX`.
    ///
    /// - `mem::size_of::<T>() * self.capacity + 1` would overflow.
    ///
    /// - An allocation failed.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    pub unsafe fn push(
        &mut self,
        value: T,
        alloc: impl Allocator,
    ) -> Result<(), alloc::AllocError> {
        push(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            value,
            &alloc,
        )
    }

    /// Removes the last element from a vector and returns it, or [`None`] if
    /// it is empty.
    pub const fn pop(&mut self) -> Option<T> {
        if self.length == 0 {
            return None;
        }

        // SAFETY: we already checked that there is actually something to pop.
        Some(unsafe { pop(&mut self.data, &mut self.length) })
    }

    /// Clears and drops all the elements in the [MultiArray] without freeing any
    /// memory.
    pub fn clear(&mut self) {
        clear(&mut self.data, &mut self.length)
    }

    /// Moves all elements out of `other` into `self`, leaving `other` empty.
    ///
    /// Returns an error if:
    ///
    /// - An allocation failure occurs.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    pub unsafe fn append(
        &mut self,
        other: &mut MultiArray<T>,
        alloc: impl Allocator,
    ) -> Result<(), alloc::AllocError> {
        append(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            &alloc,
            other.data,
            other.length,
        )
    }

    /// Appends an element to the end of the [MultiArray] only if there is enough
    /// capacity to do so, otherwise the element is returned.
    ///
    /// Guaranteed to never allocate memory.
    pub const fn push_within_capacity(&mut self, value: T) -> Result<(), T> {
        if self.length >= self.capacity {
            return Err(value);
        }

        // SAFETY: we know this is safe because we've already checked that
        //         self.length is less than self.capacity.
        unsafe {
            push_within_capacity(&mut self.data, &mut self.length, value)
        };
        Ok(())
    }

    /// Inserts an element at the given `index` in the [MultiArray], shifting
    /// everything after it to the right.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    pub unsafe fn insert(
        &mut self,
        index: usize,
        value: T,
        alloc: impl Allocator,
    ) -> Result<Option<()>, alloc::AllocError> {
        if index > self.length {
            return Ok(None);
        }

        insert_unchecked(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            &alloc,
            index,
            value,
        )?;
        Ok(Some(()))
    }

    /// Inserts an element at the given `index` in the [MultiArray], shifting
    /// everything after it to the right.
    ///
    /// # Safety
    ///
    /// This function is safe to use if the following is true:
    /// - `index` is within the bounds of the [MultiArray].
    /// - The same allocator is used for all methods on this object.
    pub unsafe fn insert_unchecked(
        &mut self,
        index: usize,
        value: T,
        alloc: impl Allocator,
    ) -> Result<(), alloc::AllocError> {
        insert_unchecked(
            &mut self.data,
            &mut self.length,
            &mut self.capacity,
            &alloc,
            index,
            value,
        )
    }

    /// Removes and returns the element at the given `index`, shifting
    /// everything after it to the left.
    ///
    /// If the given `index` is out of bounds, returns [`Option::None`].
    pub const fn remove(&mut self, index: usize) -> Option<T> {
        if index == self.length - 1 {
            return Some(unsafe { pop(&mut self.data, &mut self.length) });
        }

        if index >= self.length {
            return None;
        }

        // SAFETY: this is safe because we already checked that the index is in
        //         bounds.
        Some(unsafe {
            remove_unchecked(&mut self.data, &mut self.length, index)
        })
    }

    /// Removes and returns the element at the given `index`, shifting
    /// everything after it to the left.
    ///
    /// # Safety
    ///
    /// This function is safe to use if the `index` is in bounds.
    pub const unsafe fn remove_unchecked(&mut self, index: usize) -> T {
        remove_unchecked(&mut self.data, &mut self.length, index)
    }

    /// Removes and returns the element at the given `index`, swapping it with
    /// the last element in the array.
    ///
    /// If the given `index` is out of bounds, returns [`Option::None`].
    pub const fn swap_remove(&mut self, index: usize) -> Option<T> {
        if index == self.length - 1 {
            return self.pop();
        }

        if index >= self.length {
            return None;
        }

        // SAFETY: we know this is safe because we've checked that the index is
        //         in bounds.
        Some(unsafe {
            swap_remove_unchecked(&mut self.data, &mut self.length, index)
        })
    }

    /// Removes and returns the element at the given `index`, swapping it with
    /// the last element in the array.
    ///
    /// If the given `index` is out of bounds, returns [`Option::None`].
    ///
    /// # Safety
    ///
    /// This function is safe to use if the `index` is in bounds.
    pub const unsafe fn swap_remove_unchecked(&mut self, index: usize) -> T {
        swap_remove_unchecked(&mut self.data, &mut self.length, index)
    }

    /// Truncates the [MultiArray] to be less than or equal to the given `len`.
    /// If the [MultiArray]'s length is greater than the given `len`, the extra
    /// elements are dropped.
    pub fn truncate(&mut self, len: usize) {
        truncate(&mut self.data, &mut self.length, len)
    }
}

/// Any tuple of length x such that 2 <= x <= 12 implements this.
pub trait MultiArrayTypes<const N: usize> {
    fn sizes_and_alignments() -> [(usize, usize); N];
}

// The following is a failure in language design. This could be mitigated if we
// had compile-time reflection. Then all this could be replaced with some
// generic-length tuple type and a `for (typeinfo, name) in tuple` loop.

impl<T1, T2> MultiArrayTypes<2> for (T1, T2) {
    fn sizes_and_alignments() -> [(usize, usize); 2] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
        ]
    }
}

impl<T1, T2, T3> MultiArrayTypes<3> for (T1, T2, T3) {
    fn sizes_and_alignments() -> [(usize, usize); 3] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
            (mem::size_of::<T3>(), mem::align_of::<T3>()),
        ]
    }
}

impl<T1, T2, T3, T4> MultiArrayTypes<4> for (T1, T2, T3, T4) {
    fn sizes_and_alignments() -> [(usize, usize); 4] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
            (mem::size_of::<T3>(), mem::align_of::<T3>()),
            (mem::size_of::<T4>(), mem::align_of::<T4>()),
        ]
    }
}

impl<T1, T2, T3, T4, T5> MultiArrayTypes<5> for (T1, T2, T3, T4, T5) {
    fn sizes_and_alignments() -> [(usize, usize); 5] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
            (mem::size_of::<T3>(), mem::align_of::<T3>()),
            (mem::size_of::<T4>(), mem::align_of::<T4>()),
            (mem::size_of::<T5>(), mem::align_of::<T5>()),
        ]
    }
}

impl<T1, T2, T3, T4, T5, T6> MultiArrayTypes<6> for (T1, T2, T3, T4, T5, T6) {
    fn sizes_and_alignments() -> [(usize, usize); 6] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
            (mem::size_of::<T3>(), mem::align_of::<T3>()),
            (mem::size_of::<T4>(), mem::align_of::<T4>()),
            (mem::size_of::<T5>(), mem::align_of::<T5>()),
            (mem::size_of::<T6>(), mem::align_of::<T6>()),
        ]
    }
}

impl<T1, T2, T3, T4, T5, T6, T7> MultiArrayTypes<7>
    for (T1, T2, T3, T4, T5, T6, T7)
{
    fn sizes_and_alignments() -> [(usize, usize); 7] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
            (mem::size_of::<T3>(), mem::align_of::<T3>()),
            (mem::size_of::<T4>(), mem::align_of::<T4>()),
            (mem::size_of::<T5>(), mem::align_of::<T5>()),
            (mem::size_of::<T6>(), mem::align_of::<T6>()),
            (mem::size_of::<T7>(), mem::align_of::<T7>()),
        ]
    }
}

impl<T1, T2, T3, T4, T5, T6, T7, T8> MultiArrayTypes<8>
    for (T1, T2, T3, T4, T5, T6, T7, T8)
{
    fn sizes_and_alignments() -> [(usize, usize); 8] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
            (mem::size_of::<T3>(), mem::align_of::<T3>()),
            (mem::size_of::<T4>(), mem::align_of::<T4>()),
            (mem::size_of::<T5>(), mem::align_of::<T5>()),
            (mem::size_of::<T6>(), mem::align_of::<T6>()),
            (mem::size_of::<T7>(), mem::align_of::<T7>()),
            (mem::size_of::<T8>(), mem::align_of::<T8>()),
        ]
    }
}

impl<T1, T2, T3, T4, T5, T6, T7, T8, T9> MultiArrayTypes<9>
    for (T1, T2, T3, T4, T5, T6, T7, T8, T9)
{
    fn sizes_and_alignments() -> [(usize, usize); 9] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
            (mem::size_of::<T3>(), mem::align_of::<T3>()),
            (mem::size_of::<T4>(), mem::align_of::<T4>()),
            (mem::size_of::<T5>(), mem::align_of::<T5>()),
            (mem::size_of::<T6>(), mem::align_of::<T6>()),
            (mem::size_of::<T7>(), mem::align_of::<T7>()),
            (mem::size_of::<T8>(), mem::align_of::<T8>()),
            (mem::size_of::<T9>(), mem::align_of::<T9>()),
        ]
    }
}

impl<T1, T2, T3, T4, T5, T6, T7, T8, T9, T10> MultiArrayTypes<10>
    for (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10)
{
    fn sizes_and_alignments() -> [(usize, usize); 10] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
            (mem::size_of::<T3>(), mem::align_of::<T3>()),
            (mem::size_of::<T4>(), mem::align_of::<T4>()),
            (mem::size_of::<T5>(), mem::align_of::<T5>()),
            (mem::size_of::<T6>(), mem::align_of::<T6>()),
            (mem::size_of::<T7>(), mem::align_of::<T7>()),
            (mem::size_of::<T8>(), mem::align_of::<T8>()),
            (mem::size_of::<T9>(), mem::align_of::<T9>()),
            (mem::size_of::<T10>(), mem::align_of::<T10>()),
        ]
    }
}

impl<T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11> MultiArrayTypes<11>
    for (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11)
{
    fn sizes_and_alignments() -> [(usize, usize); 11] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
            (mem::size_of::<T3>(), mem::align_of::<T3>()),
            (mem::size_of::<T4>(), mem::align_of::<T4>()),
            (mem::size_of::<T5>(), mem::align_of::<T5>()),
            (mem::size_of::<T6>(), mem::align_of::<T6>()),
            (mem::size_of::<T7>(), mem::align_of::<T7>()),
            (mem::size_of::<T8>(), mem::align_of::<T8>()),
            (mem::size_of::<T9>(), mem::align_of::<T9>()),
            (mem::size_of::<T10>(), mem::align_of::<T10>()),
            (mem::size_of::<T11>(), mem::align_of::<T11>()),
        ]
    }
}

impl<T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12> MultiArrayTypes<12>
    for (T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12)
{
    fn sizes_and_alignments() -> [(usize, usize); 12] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
            (mem::size_of::<T3>(), mem::align_of::<T3>()),
            (mem::size_of::<T4>(), mem::align_of::<T4>()),
            (mem::size_of::<T5>(), mem::align_of::<T5>()),
            (mem::size_of::<T6>(), mem::align_of::<T6>()),
            (mem::size_of::<T7>(), mem::align_of::<T7>()),
            (mem::size_of::<T8>(), mem::align_of::<T8>()),
            (mem::size_of::<T9>(), mem::align_of::<T9>()),
            (mem::size_of::<T10>(), mem::align_of::<T10>()),
            (mem::size_of::<T11>(), mem::align_of::<T11>()),
            (mem::size_of::<T12>(), mem::align_of::<T12>()),
        ]
    }
}
