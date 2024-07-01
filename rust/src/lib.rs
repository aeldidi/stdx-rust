//! Facilities for working with Rust source code, particularly for use in
//! procedural macros.
//!
//! This needs to be in its own crate because we use it from proc_macros in
//! stdx, so stdx itself can't depend on it.
//!
//! Also, we pass spans around as [Any], since we can't actually look at the
//! span type here.

use core::any::Any;

/// The analogue for `proc_macro::TokenTree`.
pub enum TokenTree<Span: Any> {
    Group(),
    Ident(),
    Punct(Punct<Span>),
    Literal(Span),
}

/// The analogue for `proc_macro::Group`.
pub struct Group<Span: Any> {
    pub stream: 
    pub delimiter: Delimiter,
    pub span: Span,
}

/// The analogue for `proc_macro::Delimiter`.
pub enum Delimiter {
    Parenthesis,
    Brace,
    Bracket,
    None,
}

/// The analogue for `proc_macro::Spacing`.
pub enum Spacing {
    Joint,
    Alone,
}

/// The analogue for `proc_macro::Punct`.
pub struct Punct<Span: Any> {
    pub ch: char,
    pub spacing: Spacing,
    pub span: Span,
}
