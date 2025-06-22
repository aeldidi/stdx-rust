//! A minimal shim for the types we use from `proc_macro`.
#![cfg(not(proc_macro))]

use alloc::{
    string::String,
    vec::{IntoIter, Vec},
};
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Delimiter {
    Parenthesis,
    Brace,
    Bracket,
    None,
}

impl Into<super::Delimiter> for Delimiter {
    fn into(self) -> super::Delimiter {
        match self {
            Delimiter::Parenthesis => super::Delimiter::Parenthesis,
            Delimiter::Brace => super::Delimiter::Brace,
            Delimiter::Bracket => super::Delimiter::Bracket,
            Delimiter::None => super::Delimiter::None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TokenStream {
    pub tokens: Vec<TokenTree>,
}

impl TokenStream {
    #[inline(always)]
    pub(crate) fn new() -> Self {
        TokenStream { tokens: Vec::new() }
    }

    #[inline(always)]
    pub(crate) fn into_iter(self) -> IntoIter<TokenTree> {
        self.tokens.into_iter()
    }
}

impl fmt::Display for TokenStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..self.tokens.len() {
            write!(f, "{}", self.tokens[i])?;
            if i != self.tokens.len() - 1 {
                write!(f, " ")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Span {
    start: u32,
    end: u32,
}

#[derive(Debug, Clone)]
pub(crate) enum TokenTree {
    Group(Group),
    Ident(Ident),
    Punct(Punct),
    Literal(Literal),
}

impl TokenTree {
    pub(crate) fn span(&self) -> Span {
        match self {
            TokenTree::Group(x) => x.span,
            TokenTree::Ident(x) => x.span,
            TokenTree::Punct(x) => x.span,
            TokenTree::Literal(x) => x.span,
        }
    }
}

impl fmt::Display for TokenTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenTree::Group(group) => write!(f, "{}", group),
            TokenTree::Ident(ident) => write!(f, "{}", ident),
            TokenTree::Punct(punct) => write!(f, "{}", punct),
            TokenTree::Literal(literal) => write!(f, "{}", literal),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Literal {
    text: String,
    span: Span,
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

impl Literal {
    pub(crate) fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Ident {
    string: String,
    stream: TokenStream,
    span: Span,
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.string)
    }
}

impl Ident {
    pub(crate) fn span(&self) -> Span {
        self.span
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Punct {
    ch: char,
    spacing: super::Spacing,
    span: Span,
}

impl fmt::Display for Punct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.ch)
    }
}

impl Punct {
    pub(crate) fn span(&self) -> Span {
        self.span
    }

    pub(crate) fn as_char(&self) -> char {
        self.ch
    }

    pub(crate) fn spacing(&self) -> super::Spacing {
        self.spacing
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Group {
    delimiter: Delimiter,
    stream: TokenStream,
    span: Span,
}

impl Group {
    pub(crate) fn new(delimiter: Delimiter, stream: TokenStream) -> Self {
        Group {
            delimiter,
            stream,
            span: Span { start: 0, end: 0 },
        }
    }

    pub(crate) fn delimiter(&self) -> Delimiter {
        self.delimiter.clone()
    }

    pub(crate) fn stream(&self) -> TokenStream {
        self.stream.clone()
    }

    pub(crate) fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.delimiter {
            Delimiter::Parenthesis => write!(f, "({})", self.stream),
            Delimiter::Brace => write!(f, "{{{}}}", self.stream),
            Delimiter::Bracket => write!(f, "[{}]", self.stream),
            Delimiter::None => write!(f, "{}", self.stream),
        }
    }
}
