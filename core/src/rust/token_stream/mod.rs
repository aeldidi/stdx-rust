use core::{fmt, mem::transmute};

extern crate proc_macro;

mod compile_time;
mod runtime;

#[derive(Debug, Clone, Copy)]
pub enum Span {
    CompileTime(compile_time::Span),
    Runtime(runtime::Span),
}

#[derive(Debug, Clone)]
pub enum TokenStream {
    CompileTime(compile_time::TokenStream),
    Runtime(runtime::TokenStream),
}

impl From<proc_macro::TokenStream> for TokenStream {
    fn from(value: proc_macro::TokenStream) -> Self {
        TokenStream::CompileTime(compile_time::TokenStream(value))
    }
}

pub enum IntoIter {
    CompileTime(compile_time::IntoIter),
    Runtime(alloc::vec::IntoIter<runtime::TokenTree>),
}

impl TokenStream {
    pub fn new() -> TokenStream {
        match proc_macro::is_available() {
            true => TokenStream::CompileTime(compile_time::TokenStream::new()),
            false => TokenStream::Runtime(runtime::TokenStream::new()),
        }
    }
}

impl fmt::Display for TokenStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenStream::CompileTime(token_stream) => {
                write!(f, "{}", token_stream)
            }
            TokenStream::Runtime(token_stream) => {
                write!(f, "{}", token_stream)
            }
        }
    }
}

impl IntoIterator for TokenStream {
    type Item = TokenTree;

    type IntoIter = IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            TokenStream::CompileTime(token_stream) => {
                IntoIter::CompileTime(token_stream.into_iter())
            }
            TokenStream::Runtime(token_stream) => {
                IntoIter::Runtime(token_stream.into_iter())
            }
        }
    }
}

impl Iterator for IntoIter {
    type Item = TokenTree;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            IntoIter::CompileTime(x) => x.next().map(|x| x.into()),
            IntoIter::Runtime(x) => x.next().map(|x| x.into()),
        }
    }
}

impl Into<TokenTree> for compile_time::TokenTree {
    fn into(self) -> TokenTree {
        let tt = unsafe { transmute::<_, proc_macro::TokenTree>(self.0) };
        match tt {
            proc_macro::TokenTree::Group(group) => TokenTree::Group(
                Group::CompileTime(compile_time::Group(group)),
            ),
            proc_macro::TokenTree::Ident(ident) => TokenTree::Ident(
                Ident::CompileTime(compile_time::Ident(ident)),
            ),
            proc_macro::TokenTree::Punct(punct) => TokenTree::Punct(
                Punct::CompileTime(compile_time::Punct(punct)),
            ),
            proc_macro::TokenTree::Literal(literal) => TokenTree::Literal(
                Literal::CompileTime(compile_time::Literal(literal)),
            ),
        }
    }
}

impl Into<TokenTree> for runtime::TokenTree {
    fn into(self) -> TokenTree {
        match self {
            runtime::TokenTree::Group(group) => {
                TokenTree::Group(Group::Runtime(group))
            }
            runtime::TokenTree::Ident(ident) => {
                TokenTree::Ident(Ident::Runtime(ident))
            }
            runtime::TokenTree::Punct(punct) => {
                TokenTree::Punct(Punct::Runtime(punct))
            }
            runtime::TokenTree::Literal(literal) => {
                TokenTree::Literal(Literal::Runtime(literal))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum TokenTree {
    Group(Group),
    Ident(Ident),
    Punct(Punct),
    Literal(Literal),
}

impl TokenTree {
    pub fn span(&self) -> Span {
        match self {
            TokenTree::Group(group) => group.span(),
            TokenTree::Ident(ident) => ident.span(),
            TokenTree::Punct(punct) => punct.span(),
            TokenTree::Literal(literal) => literal.span(),
        }
    }
}

impl fmt::Display for TokenTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenTree::Group(group) => group.fmt(f),
            TokenTree::Ident(ident) => ident.fmt(f),
            TokenTree::Punct(punct) => punct.fmt(f),
            TokenTree::Literal(literal) => literal.fmt(f),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Group {
    CompileTime(compile_time::Group),
    Runtime(runtime::Group),
}

impl Group {
    pub fn span(&self) -> Span {
        match self {
            Group::CompileTime(group) => Span::CompileTime(group.span()),
            Group::Runtime(group) => Span::Runtime(group.span()),
        }
    }

    pub fn delimiter(&self) -> Delimiter {
        match self {
            Group::CompileTime(group) => group.delimiter().into(),
            Group::Runtime(group) => group.delimiter().into(),
        }
    }

    pub fn stream(&self) -> TokenStream {
        match self {
            Group::CompileTime(group) => {
                TokenStream::CompileTime(group.stream())
            }
            Group::Runtime(group) => TokenStream::Runtime(group.stream()),
        }
    }
}

impl fmt::Display for Group {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Group::CompileTime(group) => write!(f, "{}", group),
            Group::Runtime(group) => write!(f, "{}", group),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delimiter {
    Parenthesis,
    Brace,
    Bracket,
    None,
}

#[derive(Debug, Clone)]
pub enum Ident {
    CompileTime(compile_time::Ident),
    Runtime(runtime::Ident),
}

impl Ident {
    pub fn span(&self) -> Span {
        match self {
            Ident::CompileTime(ident) => Span::CompileTime(ident.span()),
            Ident::Runtime(ident) => Span::Runtime(ident.span()),
        }
    }
}

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ident::CompileTime(ident) => write!(f, "{}", ident),
            Ident::Runtime(ident) => write!(f, "{}", ident),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Spacing {
    Joint,
    Alone,
}

#[derive(Debug, Clone)]
pub enum Punct {
    CompileTime(compile_time::Punct),
    Runtime(runtime::Punct),
}

impl Punct {
    pub fn span(&self) -> Span {
        match self {
            Punct::CompileTime(punct) => Span::CompileTime(punct.span()),
            Punct::Runtime(punct) => Span::Runtime(punct.span()),
        }
    }

    pub fn as_char(&self) -> char {
        match self {
            Punct::CompileTime(punct) => punct.as_char(),
            Punct::Runtime(punct) => punct.as_char(),
        }
    }

    pub fn spacing(&self) -> Spacing {
        match self {
            Punct::CompileTime(punct) => punct.spacing(),
            Punct::Runtime(punct) => punct.spacing(),
        }
    }
}

impl fmt::Display for Punct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Punct::CompileTime(punct) => write!(f, "{}", punct),
            Punct::Runtime(punct) => write!(f, "{}", punct),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Literal {
    CompileTime(compile_time::Literal),
    Runtime(runtime::Literal),
}

impl Literal {
    pub fn span(&self) -> Span {
        match self {
            Literal::CompileTime(literal) => Span::CompileTime(literal.span()),
            Literal::Runtime(literal) => Span::Runtime(literal.span()),
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::CompileTime(literal) => write!(f, "{}", literal),
            Literal::Runtime(literal) => write!(f, "{}", literal),
        }
    }
}
