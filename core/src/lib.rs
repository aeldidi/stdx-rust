#![cfg_attr(not(doc), no_std)]
// If these ever all give the message "this feature has been stable since X",
// note down the version used in rust-version in cargo.toml and take us off of
// "nightly".
#![feature(allocator_api, alloc_layout_extra)]

//! # `stdx`
//! A set of extensions to `std`.

extern crate alloc;

/// A dynamic array as well as building blocks for creating data structures
/// containing them.
pub mod array;
/// A rust parser meant for procedural macros.
pub mod rust;
