//! A version of [`std::string::String`] which uses the specified allocator.
//!
//! The purpose of this is not to implement a string to be competitive with the
//! standard [`std::string::String`] type, but this is what we have to do since
//! at the time of writing the standard library doesn't expose the Vec's type
//! parameter (for whatever reason). This is mostly duplicated effort for
//! really no reason unfortunately.

use std::{
    alloc::{Allocator, Global},
    collections::TryReserveError,
    string::String as StdString,
};

#[derive(PartialEq, PartialOrd, Eq, Ord)]
pub struct String<A: Allocator = Global> {
    vec: Vec<u8, A>,
}

impl String {
    #[inline]
    #[must_use]
    pub const fn new() -> StdString {
        StdString::new()
    }

    #[inline]
    pub fn try_with_capacity(
        capacity: usize,
    ) -> Result<StdString, TryReserveError> {
        StdString::try_with_capacity(capacity)
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> StdString {
        StdString::with_capacity(capacity)
    }

    #[inline]
    #[must_use]
    pub const fn new_in<A: Allocator>(alloc: A) -> String<A> {
        String {
            vec: Vec::<u8, A>::new_in(alloc),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity_in<A: Allocator>(
        capacity: usize,
        alloc: A,
    ) -> String<A> {
        String {
            vec: Vec::<u8, A>::with_capacity_in(capacity, alloc),
        }
    }

    #[inline]
    #[must_use]
    pub fn try_with_capacity_in<A: Allocator>(
        capacity: usize,
        alloc: A,
    ) -> Result<String<A>, TryReserveError> {
        Ok(String {
            vec: Vec::<u8, A>::try_with_capacity_in(capacity, alloc)?,
        })
    }
}

impl<A: Allocator> String<A> {
    #[inline]
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.vec.as_slice()
    }

    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.vec.len()
    }

    #[inline]
    pub fn truncate(&mut self, new_len: usize) {
        if new_len <= self.len() {
            assert!(self.is_char_boundary(new_len));
            self.vec.truncate(new_len);
        }
    }

    /// Returns true if the given byte index is a UTF-8 character boundary.
    pub const fn is_char_boundary(&self, index: usize) -> bool {
        if index == 0 {
            return true;
        }

        if index < self.len() {
            return is_utf8_char_boundary(self.as_bytes()[index]);
        }

        index == self.len()
    }
}

const fn is_utf8_char_boundary(x: u8) -> bool {
    (x as i8) >= -0x40
}
