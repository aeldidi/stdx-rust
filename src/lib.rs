#![no_std]
#![feature(strict_provenance, allocator_api)] // Used in alloc

//! # `stdx`
//! A set of extensions to `std`.

/// A set of allocators implementing [Allocator] for different use
/// cases.
pub mod alloc;
/// A dynamic array which explicitly takes an [Allocator] and allows handling
/// of out-of-memory situations.
pub mod vec;

#[cfg(test)]
mod tests {}
