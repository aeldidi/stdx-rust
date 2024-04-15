#![no_std]
#![feature(strict_provenance)] // Used in alloc

pub mod alloc;
pub mod vec;

#[cfg(test)]
mod tests {}
