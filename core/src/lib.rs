#![cfg_attr(not(doc), no_std)]
// If these ever all give the message "this feature has been stable since X",
// note down the version used in rust-version in cargo.toml and take us off of
// "nightly".
#![feature(allocator_api, cfg_version, alloc_layout_extra)]

//! # `stdx`
//! A set of extensions to `std`.

/// A dynamic array as well as building blocks for creating data structures
/// containing them.
pub mod array;
/// Operations and utilities which allow access to implementation details of a
/// particular Rust compiler version.
pub mod unstable;

#[cfg(test)]
mod tests {}
