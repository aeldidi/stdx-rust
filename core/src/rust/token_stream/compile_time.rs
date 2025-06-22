//! A set of newtype wrappers around the `proc_macro` types. Runtime code can
//! access `proc_macro` types, it just panics when calling most functions. We
//! can even define a safe `From` implementation by using `mem::transmute`,
//! since we have `repr(transparent on everything)`.
extern crate proc_macro;

use core::fmt;

use proc_macro::{
    token_stream::IntoIter as PMIntoIter, Delimiter as PMDelimiter,
    Group as PMGroup, Ident as PMIdent, Literal as PMLiteral,
    Punct as PMPunct, Spacing as PMSpacing, Span as PMSpan,
    TokenStream as PMTokenStream, TokenTree as PMTokenTree,
};

#[derive(Debug, Clone)]
#[repr(transparent)]
pub(crate) struct TokenStream(pub(crate) PMTokenStream);

impl TokenStream {
    pub(crate) fn new() -> Self {
        TokenStream(PMTokenStream::new())
    }
}

impl IntoIterator for TokenStream {
    type Item = TokenTree;
    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.0.into_iter())
    }
}

impl fmt::Display for TokenStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[repr(transparent)]
pub(crate) struct IntoIter(pub(crate) PMIntoIter);

impl Iterator for IntoIter {
    type Item = TokenTree;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(TokenTree)
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub(crate) struct Delimiter(pub(crate) PMDelimiter);

impl Into<super::Delimiter> for Delimiter {
    fn into(self) -> super::Delimiter {
        match self.0 {
            PMDelimiter::Parenthesis => super::Delimiter::Parenthesis,
            PMDelimiter::Brace => super::Delimiter::Brace,
            PMDelimiter::Bracket => super::Delimiter::Bracket,
            PMDelimiter::None => super::Delimiter::None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub(crate) struct Span(pub(crate) PMSpan);

#[derive(Debug, Clone)]
#[repr(transparent)]
pub(crate) struct TokenTree(pub(crate) PMTokenTree);

impl TokenTree {
    pub(crate) fn span(&self) -> Span {
        Span(self.0.span())
    }
}

impl fmt::Display for TokenTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub(crate) struct Literal(pub(crate) PMLiteral);

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Literal {
    pub(crate) fn span(&self) -> Span {
        Span(self.0.span())
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub(crate) struct Ident(pub(crate) PMIdent);

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Ident {
    pub(crate) fn span(&self) -> Span {
        Span(self.0.span())
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub(crate) struct Punct(pub(crate) PMPunct);

impl PartialEq<char> for Punct {
    fn eq(&self, other: &char) -> bool {
        self.0 == *other
    }
}

impl fmt::Display for Punct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Punct {
    pub(crate) fn span(&self) -> Span {
        Span(self.0.span())
    }

    pub(crate) fn as_char(&self) -> char {
        self.0.as_char()
    }

    pub(crate) fn spacing(&self) -> super::Spacing {
        match self.0.spacing() {
            PMSpacing::Joint => super::Spacing::Joint,
            PMSpacing::Alone => super::Spacing::Alone,
        }
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
pub(crate) struct Group(pub(crate) PMGroup);

impl Group {
    pub(crate) fn new(delimiter: Delimiter, stream: TokenStream) -> Self {
        Group(PMGroup::new(delimiter.0, stream.0))
    }

    pub(crate) fn delimiter(&self) -> Delimiter {
        Delimiter(self.0.delimiter())
    }

    pub(crate) fn stream(&self) -> TokenStream {
        TokenStream(self.0.stream())
    }

    pub(crate) fn span(&self) -> Span {
        Span(self.0.span())
    }
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
