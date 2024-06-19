#![cfg_attr(not(doc), no_std)]
#![feature(
    strict_provenance,
    allocator_api,
    cfg_version,
    const_mut_refs,
    const_ptr_write,
    non_null_convenience,
    const_intrinsic_copy
)]

//! # `stdx`
//! A set of extensions to `std`.

/// A useful set of allocators which can provide better performance than
/// general purpose allocators depending on your usage pattern.
pub mod alloc;
/// A dynamic array as well as building blocks for creating data structures
/// containing them.
pub mod array;
/// Operations and utilities which allow access to implementation details of a
/// particular Rust compiler version.
pub mod unstable;

#[cfg(test)]
mod tests {}
