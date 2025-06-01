//! Facilities for working with Rust source code, particularly for use in
//! procedural macros.

use crate::array::Array;
use core::{
    alloc::{self, Allocator},
    iter::Peekable,
};

pub mod compat {
    /// The analogue for `proc_macro::TokenTree`.
    #[derive(Clone, PartialEq, Eq, Debug)]
    pub enum TokenTree<Span: Clone, Stream: Iterator<Item = Self> + Clone> {
        Group(Group<Span, Stream>),
        Ident(Ident<Span>),
        Punct(Punct<Span>),
        Literal(Span),
    }

    /// The analogue for `proc_macro::Ident`.
    #[derive(Clone, PartialEq, Eq, Debug)]
    pub struct Ident<Span: Clone> {
        pub ident: String,
        pub span: Span,
    }

    /// The analogue for `proc_macro::Group`.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Group<
        Span: Clone,
        Stream: Iterator<Item = TokenTree<Span, Stream>> + Clone,
    > {
        pub stream: Stream,
        pub delimiter: Delimiter,
        pub span: Span,
    }

    /// The analogue for `proc_macro::Delimiter`.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Delimiter {
        Parenthesis,
        Brace,
        Bracket,
        None,
    }

    /// The analogue for `proc_macro::Spacing`.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum Spacing {
        Joint,
        Alone,
    }

    /// The analogue for `proc_macro::Punct`.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Punct<Span: Clone> {
        pub ch: char,
        pub spacing: Spacing,
        pub span: Span,
    }
}

/// Represents a Rust type. Currently doesn't support any operations on it.
pub enum Type {
    /// `!` type.
    NeverType,
    /// Either a generic type, a type with bounds, or a macro.
    ComplexType,
    /// A type specified by an identifier (like `bool`, `i32`, `str`, `Mutex`).
    SimpleType(String),
}

pub enum Error {
    ParseError,
    AllocError,
}

/// ```
/// Syntax
/// Type :
///       TypeNoBounds
///    | ImplTraitType
///    | TraitObjectType
///
/// TypeNoBounds :
///       ParenthesizedType
///    | ImplTraitTypeOneBound
///    | TraitObjectTypeOneBound
///    | TypePath
///    | TupleType
///    | NeverType
///    | RawPointerType
///    | ReferenceType
///    | ArrayType
///    | SliceType
///    | InferredType
///    | QualifiedPathInType
///    | BareFunctionType
///    | MacroInvocation
/// ```
pub fn parse_type<
    Span: Clone,
    Stream: Iterator<Item = compat::TokenTree<Span, Stream>> + Clone,
    A: Allocator,
>(
    mut input: Peekable<Stream>,
    alloc: A,
) -> Result<Type, Error> {
    use compat::*;

    let next = match input.next() {
        Some(x) => x,
        None => return Err(ParseError),
    };

    match next {
        TokenTree::Group(Group {
            stream: _,
            delimiter: _,
            span: _,
        }) => {
            todo!()
        }
        TokenTree::Ident(Ident { ident, span: _ }) => {
            return Ok(Type::SimpleType(ident));
        }
        TokenTree::Punct(Punct {
            ch,
            spacing: _,
            span: _,
        }) => {
            if ch == '!' {
                return Ok(Type::NeverType);
            }
        }
        TokenTree::Literal(_) => todo!(),
    }

    Err(ParseError)
}

pub enum SimplePathSegment {
    Ident(String),
    Super,
    SelfKW,
    Crate,
    DollarCrate,
}

/// ```
///   SimplePathSegment :
///      IDENTIFIER | super | self | crate | $crate
/// ```
pub fn parse_simple_path_segment<
    Span: Clone,
    Stream: Iterator<Item = compat::TokenTree<Span, Stream>> + Clone,
    A: Allocator,
>(
    mut input: Peekable<Stream>,
    alloc: A,
) -> Result<SimplePathSegment, Error> {
    use compat::*;
    let next = match input.peek() {
        Some(x) => x.clone(),
        None => return Err(ParseError),
    };

    match next {
        TokenTree::Ident(Ident { ident, .. }) => {
            _ = input.next().unwrap();
            return match ident.as_str() {
                "super" => Ok(SimplePathSegment::Super),
                "self" => Ok(SimplePathSegment::SelfKW),
                "crate" => Ok(SimplePathSegment::Crate),
                ident => Ok(SimplePathSegment::Ident(ident.to_string())),
            };
        }
        TokenTree::Punct(Punct {
            ch: ':',
            spacing: Spacing::Joint,
            ..
        }) => {
            // is it a $crate?
            _ = input.next().unwrap();

            let next = match input.next() {
                Some(x) => x,
                None => return Err(ParseError),
            };
            if let TokenTree::Ident(Ident { ident, .. }) = next {
                if ident == "crate" {
                    return Ok(SimplePathSegment::DollarCrate);
                }
            }
        }
        _ => (),
    }

    return Err(ParseError);
}

pub struct SimplePath {
    pub elements: Array<SimplePathSegment>,
}

/// ```
/// SimplePath :
///   ::? SimplePathSegment (:: SimplePathSegment)*
/// ```
pub fn parse_simple_path<
    Span: Clone,
    Stream: Iterator<Item = compat::TokenTree<Span, Stream>> + Clone,
    A: Allocator,
>(
    mut input: Peekable<Stream>,
    alloc: A,
) -> Result<Type, Error> {
    use compat::*;

    let mut result = Array::new(&alloc);
    loop {
        match input.next() {
            Some(TokenTree::Punct(':')) => {
                if let Some(TokenTree::Punct(':')) = input.peek() {
                    input.next().unwrap();
                    continue;
                }

                return Err(ParseError);
            }
            Some(_) => match parse_simple_path_segment(input, alloc) {
                Ok(sps) => result.push(sps).map_err(|| Error::AllocError)?,
                Err(Error::AllocError) => return Err(Error::AllocError),
                Err(_) => break,
            },
            None => return Err(ParseError),
        }
    }

    if result.len() == 0 {
        return Err(ParseError);
    }

    return Ok(SimplePath { elements: result });
}
