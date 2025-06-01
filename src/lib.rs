/// Everything is actually coreemented in `stdx_core`. This crate just exists
/// so we can export proc macros with it.

pub mod unstable {
    pub use stdx_core::unstable;
}

pub mod rust {
    pub use stdx_core::rust;
}

pub mod alloc {
    pub use stdx_core::alloc;
}

pub mod array {
    pub use stdx_core::array;
    pub use stdx_soa::Soa;
}
