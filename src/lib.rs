#![no_std]
#![feature(strict_provenance, allocator_api)] // Used in alloc

pub mod alloc;
pub mod vec;

#[cfg(test)]
mod tests {}
