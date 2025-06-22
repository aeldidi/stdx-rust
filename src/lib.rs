// If these ever all give the message "this feature has been stable since X",
// note down the version used in rust-version in cargo.toml and take us off of
// "nightly".
#![feature(allocator_api, cfg_version, alloc_layout_extra, try_with_capacity)]

pub use stdx_core::*;

/// A url parser and utilities.
pub mod url;

/// A useful set of allocators which can provide better performance than
/// general purpose allocators depending on your usage pattern.
pub mod alloc;

pub mod array {
    pub use stdx_soa::Soa;
}

#[derive(array::Soa)]
struct Foo {
    bar: i32,
    baz: String,
}
