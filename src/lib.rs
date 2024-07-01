#![cfg_attr(not(doc), no_std)]
// If these ever all give the message "this feature has been stable since X",
// note down the version used in rust-version in cargo.toml and take us off of
// "nightly".
#![feature(
    strict_provenance,
    allocator_api,
    cfg_version,
    const_mut_refs,
    const_ptr_write,
    const_intrinsic_copy,
    alloc_layout_extra
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

/// Facilities for working with Rust source code, particularly for use in
/// procedural macros.
pub mod rust {
    pub use stdx_rust::*;
}

#[cfg(test)]
mod tests {}
