//! A minimal shim for the types we use from `proc_macro`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Delimiter {
    Parenthesis,
    Brace,
    Bracket,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TokenStream {
    pub tokens: Vec<TokenTree>,
}

impl TokenStream {
    pub fn new() -> Self {
        TokenStream { tokens: Vec::new() }
    }
    pub fn from_tokens(tokens: Vec<TokenTree>) -> Self {
        TokenStream { tokens }
    }
    pub fn into_iter(self) -> std::vec::IntoIter<TokenTree> {
        self.tokens.into_iter()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenTree {
    Group(Group),
    Ident(String),
    Punct(char),
    Literal(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub delimiter: Delimiter,
    pub stream: TokenStream,
}

impl Group {
    pub fn new(delimiter: Delimiter, stream: TokenStream) -> Self {
        Group { delimiter, stream }
    }
    pub fn delimiter(&self) -> Delimiter {
        self.delimiter.clone()
    }
    pub fn stream(&self) -> TokenStream {
        self.stream.clone()
    }
}
