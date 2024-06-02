use core::{alloc::Allocator, marker::PhantomData, mem, ptr::NonNull};

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
    data: NonNull<T>,
    capacity: usize,
    length: usize,
}

trait MultiArrayTypes<const N: usize> {
    fn lengths_and_alignments() -> [(usize, usize); N];
}

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
///     arr: MultiArray<(i32, String, SomeType)>
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
///     arr.push((0i32, String::new(), SomeType::SomeValue));
/// }
/// ```
pub struct MultiArray<T: MultiArrayTypes> {
    capacity: usize,
    length: usize,
    _marker: PhantomData<T>,
}
