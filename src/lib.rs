// If these ever all give the message "this feature has been stable since X",
// note down the version used in rust-version in cargo.toml and take us off of
// "nightly".
#![feature(
    allocator_api,
    cfg_version,
    alloc_layout_extra,
    const_vec_string_slice,
    try_with_capacity
)]

/// A useful set of allocators which can provide better performance than
/// general purpose allocators depending on your usage pattern.
pub mod alloc;

pub mod array {
    pub use stdx_soa::Soa;
}
