use core::{
    alloc::{self, Allocator, Layout, LayoutError},
    marker::PhantomData,
    mem,
    ops::{self, Deref, DerefMut},
    ptr::{self, NonNull},
    slice::{self, SliceIndex},
};

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
/// The point of this is to avoid storing multiple of the same [Allocator] when
/// stored in a struct.
pub struct SubArray<T, A: Allocator> {
    arr: RawArray<T>,
    _marker: PhantomData<A>,
}

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
    pub unsafe fn from_raw_parts(
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
        alloc: impl Allocator,
        capacity: usize,
    ) -> Result<RawArray<T>, alloc::AllocError> {
        let layout = match Layout::from_size_align(
            mem::size_of::<T>() * capacity,
            mem::align_of::<T>(),
        ) {
            Ok(l) => l,
            Err(_) => return Err(alloc::AllocError),
        };
        let mem = alloc.allocate(layout)?;
        let capacity = mem.len();

        Ok(RawArray {
            data: mem.cast(),
            length: 0,
            capacity,
        })
    }

    /// Returns the length of the [RawArray].
    #[inline]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Returns the total number of elements the vector can hold without
    /// reallocating.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns true if the [RawArray] is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.length == 0
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
        if self.capacity == 0 {
            // Allocate for the first time.
            let size = match mem::size_of::<T>().checked_mul(additional) {
                Some(sz) => sz,
                None => return Err(alloc::AllocError),
            };

            let result = alloc.allocate(Layout::from_size_align_unchecked(
                size,
                mem::align_of::<T>(),
            ))?;
            let size = result.len();
            self.data = result.cast();
            self.capacity = size;
            self.length = 0;
            return Ok(());
        }
        let old_size = match mem::size_of::<T>().checked_mul(self.capacity) {
            Some(sz) => sz,
            None => return Err(alloc::AllocError),
        };
        let new_size = {
            let tmp = match self.capacity.checked_add(additional) {
                Some(x) => x,
                None => return Err(alloc::AllocError),
            };
            match mem::size_of::<T>().checked_mul(tmp) {
                Some(x) => x,
                None => return Err(alloc::AllocError),
            }
        };

        let result = alloc.grow(
            self.data.cast(),
            Layout::from_size_align_unchecked(old_size, mem::align_of::<T>()),
            Layout::from_size_align_unchecked(new_size, mem::align_of::<T>()),
        )?;
        let size = result.len();
        self.data = result.cast();
        self.capacity = size;
        Ok(())
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
    #[inline]
    pub unsafe fn push(
        &mut self,
        value: T,
        alloc: impl Allocator,
    ) -> Result<(), alloc::AllocError> {
        if self.capacity == 0 {
            self.reserve(16, alloc)?;
        } else {
            self.reserve(1, alloc)?;
        }

        let offset = match mem::size_of::<T>().checked_mul(self.capacity) {
            Some(x) => {
                if x > isize::MAX as usize {
                    return Err(alloc::AllocError);
                }
                x as isize
            }
            None => return Err(alloc::AllocError),
        };
        *self.data.as_ptr().byte_offset(offset) = value;
        Ok(())
    }

    /// Removes the last element from a vector and returns it, or [`None`] if
    /// it is empty.
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.length == 0 {
            return None;
        }

        self.length -= 1;
        let offset = self.length * mem::size_of::<T>();
        // SAFETY: We're just manually moving it here. The memory underneath it
        //         can no longer be validly accessed so there's no worry about
        //         someone still reading the value from the array.
        Some(unsafe { ptr::read(self.data.as_ptr().byte_add(offset)) })
    }

    #[inline]
    pub fn clear(&mut self) {
        let elements = self.deref_mut().as_mut_ptr();
        // SAFETY: We decrease the len before calling drop_in_place because if
        //         dropping the value panics we don't want to also call drop on
        //         the RawArray (which would cause all the elements to be
        //         dropped again).
        unsafe {
            self.length = 0;
            ptr::drop_in_place(elements)
        }
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
        self.reserve(other.length, alloc)?;
        for i in 0..other.length {
            let value = ptr::read(
                other.data.as_ptr().byte_add(i * mem::size_of::<T>()),
            );
            ptr::write(
                self.data
                    .as_ptr()
                    .byte_add((i + self.length) * mem::size_of::<T>()),
                value,
            );
        }

        Ok(())
    }

    /// Inserts an element at the given `index` in the [RawArray], shifting
    /// everything after it to the right.
    ///
    /// # Safety
    ///
    /// This method is safe to use as long as you use the same allocator for
    /// all methods on this object.
    #[inline]
    pub unsafe fn insert(
        &mut self,
        index: usize,
        value: T,
        alloc: impl Allocator,
    ) -> Result<Option<()>, alloc::AllocError> {
        if index > self.length {
            return Ok(None);
        }

        self.insert_unchecked(index, value, alloc)?;
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
        if index == self.length {
            return self.push(value, alloc);
        }

        self.reserve(1, alloc)?;

        // SAFETY: At this point we know this won't overflow because
        //         1. This is the unsafe version of the function so we're
        //            already assuming index < self.len().
        //         2. Since reserve allocated successfully, we know the offset
        //            won't overflow.
        let offset = mem::size_of::<T>().unchecked_mul(index);
        let base = self.data.as_ptr().map_addr(|a| a.unchecked_add(offset));
        let count = mem::size_of::<T>()
            .unchecked_mul(self.length.unchecked_sub(index));
        ptr::copy(
            base,
            base.map_addr(|a| a.unchecked_add(mem::size_of::<T>())),
            count,
        );

        ptr::write(base, value);
        Ok(())
    }

    /// Removes and returns the element at the given `index`, shifting
    /// everything after it to the left.
    ///
    /// If the given `index` is out of bounds, returns [`Option::None`].
    #[inline]
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index == self.length - 1 {
            return self.pop();
        }

        if index >= self.length {
            return None;
        }

        // SAFETY: this is safe because we already checked that the index is in
        //         bounds.
        Some(unsafe { self.remove_unchecked(index) })
    }

    /// Removes and returns the element at the given `index`, shifting
    /// everything after it to the left.
    ///
    /// # Safety
    ///
    /// This function is safe to use if the `index` is in bounds.
    pub unsafe fn remove_unchecked(&mut self, index: usize) -> T {
        // SAFETY: we immediately set this before it is ever actually used so
        //         this is fine.
        let mut result =
            unsafe { mem::MaybeUninit::<T>::uninit().assume_init() };
        // SAFETY: we know this is safe because the index is in bounds. We are
        //         just manually moving the object out.
        unsafe {
            let offset = mem::size_of::<T>().unchecked_mul(index);
            let addr =
                self.data.as_ptr().map_addr(|a| a.unchecked_add(offset));
            result = ptr::read(addr);
            ptr::copy(
                addr.map_addr(|a| a.unchecked_add(mem::size_of::<T>())),
                addr,
                self.length.unchecked_sub(index),
            );
        }

        self.length -= 1;

        result
    }

    /// Removes and returns the element at the given `index`, swapping it with
    /// the last element in the array.
    ///
    /// If the given `index` is out of bounds, returns [`Option::None`].
    pub fn swap_remove(&mut self, index: usize) -> Option<T> {
        if index == self.length - 1 {
            return self.pop();
        }

        if index >= self.length {
            return None;
        }

        // SAFETY: we know this is safe because we've checked that the index is
        //         in bounds.
        return Some(unsafe { self.swap_remove_unchecked(index) });
    }

    /// Removes and returns the element at the given `index`, swapping it with
    /// the last element in the array.
    ///
    /// If the given `index` is out of bounds, returns [`Option::None`].
    ///
    /// # Safety
    ///
    /// This function is safe to use if the `index` is in bounds.
    pub unsafe fn swap_remove_unchecked(&mut self, index: usize) -> T {
        // SAFETY: we immediately set this before it is ever actually used so
        //         this is fine.
        let mut result =
            unsafe { mem::MaybeUninit::<T>::uninit().assume_init() };
        // SAFETY: we know this is safe because the index is in bounds. We are
        //         just manually moving the object out.
        unsafe {
            let offset = mem::size_of::<T>().unchecked_mul(index);
            let end_offset = mem::size_of::<T>()
                .unchecked_mul(self.length.unchecked_sub(1));
            let addr =
                self.data.as_ptr().map_addr(|a| a.unchecked_add(offset));
            let end_addr =
                self.data.as_ptr().map_addr(|a| a.unchecked_add(end_offset));
            result = ptr::read(addr);
            ptr::write(addr, ptr::read(end_addr));
        }

        self.length -= 1;

        result
    }

    /// Truncates the [RawArray] to be less than or equal to the given `len`.
    /// If the [RawArray]'s length is greater than the given `len`, the extra
    /// elements are dropped.
    pub fn truncate(&mut self, len: usize) {
        if self.length <= len {
            return;
        }

        for i in (len - 1)..self.length {
            // SAFETY: we know this is safe because we've already checked that
            //         the index is in bounds.
            unsafe {
                let offset = mem::size_of::<T>().unchecked_mul(i);
                let addr =
                    self.data.as_ptr().map_addr(|a| a.unchecked_add(offset));
                ptr::drop_in_place(addr);
            }
        }
        self.length = len;
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
    capacity: usize,
    length: usize,
    _marker: PhantomData<T>,
}

/// Any tuple of length x such that 2 <= x <= 12 implements this.
pub trait MultiArrayTypes<const N: usize> {
    fn lengths_and_alignments() -> [(usize, usize); N];
}

// The following is a failure in language design. This could be mitigated if we
// had compile-time reflection. Then all this could be replaced with some
// generic-length tuple type and a `for (typeinfo, name) in tuple` loop.

impl<T1, T2> MultiArrayTypes<2> for (T1, T2) {
    fn lengths_and_alignments() -> [(usize, usize); 2] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
        ]
    }
}

impl<T1, T2, T3> MultiArrayTypes<3> for (T1, T2, T3) {
    fn lengths_and_alignments() -> [(usize, usize); 3] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
            (mem::size_of::<T3>(), mem::align_of::<T3>()),
        ]
    }
}

impl<T1, T2, T3, T4> MultiArrayTypes<4> for (T1, T2, T3, T4) {
    fn lengths_and_alignments() -> [(usize, usize); 4] {
        [
            (mem::size_of::<T1>(), mem::align_of::<T1>()),
            (mem::size_of::<T2>(), mem::align_of::<T2>()),
            (mem::size_of::<T3>(), mem::align_of::<T3>()),
            (mem::size_of::<T4>(), mem::align_of::<T4>()),
        ]
    }
}

impl<T1, T2, T3, T4, T5> MultiArrayTypes<5> for (T1, T2, T3, T4, T5) {
    fn lengths_and_alignments() -> [(usize, usize); 5] {
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
    fn lengths_and_alignments() -> [(usize, usize); 6] {
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
    fn lengths_and_alignments() -> [(usize, usize); 7] {
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
    fn lengths_and_alignments() -> [(usize, usize); 8] {
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
    fn lengths_and_alignments() -> [(usize, usize); 9] {
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
    fn lengths_and_alignments() -> [(usize, usize); 10] {
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
    fn lengths_and_alignments() -> [(usize, usize); 11] {
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
    fn lengths_and_alignments() -> [(usize, usize); 12] {
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
