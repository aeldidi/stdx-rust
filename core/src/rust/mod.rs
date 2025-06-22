//! A minimal rust parser suitable for writing basic proc macros. Currently
//! only supports parsing type declarations and function declarations.

mod token_stream;

pub use token_stream::{Delimiter, TokenStream, TokenTree};

use core::iter;

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

#[derive(Debug, Clone, PartialEq)]
pub enum TypeDecl {
    Struct {
        name: String,
        fields: Vec<Field>,
    },
    Enum {
        name: String,
        variants: Vec<EnumVariant>,
    },
    TypeAlias {
        name: String,
    },
    Function {
        sig: FunctionSig,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSig {
    pub vis: Option<String>,
    pub is_async: bool,
    pub is_const: bool,
    pub name: String,
    pub args: Vec<Field>,
    pub ret: Option<String>,
}

/// Parse top-level type declarations from a TokenStream.
/// Returns an error string if parsing fails.
pub fn parse_type_decls(tokens: TokenStream) -> Result<Vec<TypeDecl>, String> {
    let mut decls = Vec::new();
    let mut iter = tokens.into_iter().peekable();

    while let Some(token) = iter.next() {
        match &token {
            TokenTree::Ident(ident) if ident.to_string() == "struct" => {
                // Parse struct name
                let name = if let Some(TokenTree::Ident(name)) = iter.next() {
                    name.to_string()
                } else {
                    return Err(
                        "Expected struct name after 'struct'".to_string()
                    );
                };
                // Parse struct fields
                let mut fields = Vec::new();
                let mut found_brace = false;
                while let Some(token) = iter.next() {
                    if let TokenTree::Group(group) = &token {
                        if group.delimiter() == Delimiter::Brace {
                            fields = parse_struct_fields(group.stream())?;
                            found_brace = true;
                            break;
                        }
                    }
                }
                if !found_brace {
                    return Err(format!(
                        "Expected '{{' with fields for struct '{}'",
                        name
                    ));
                }
                decls.push(TypeDecl::Struct { name, fields });
            }
            TokenTree::Ident(ident) if ident.to_string() == "enum" => {
                // Parse enum name
                let name = if let Some(TokenTree::Ident(name)) = iter.next() {
                    name.to_string()
                } else {
                    return Err("Expected enum name after 'enum'".to_string());
                };
                // Parse enum variants
                let mut variants = Vec::new();
                let mut found_brace = false;
                while let Some(token) = iter.next() {
                    if let TokenTree::Group(group) = &token {
                        if group.delimiter() == Delimiter::Brace {
                            variants = parse_enum_variants(group.stream())?;
                            found_brace = true;
                            break;
                        }
                    }
                }
                if !found_brace {
                    return Err(format!(
                        "Expected '{{' with variants for enum '{}'",
                        name
                    ));
                }
                decls.push(TypeDecl::Enum { name, variants });
            }
            TokenTree::Ident(ident) if ident.to_string() == "type" => {
                if let Some(TokenTree::Ident(name)) = iter.next() {
                    decls.push(TypeDecl::TypeAlias {
                        name: name.to_string(),
                    });
                } else {
                    return Err(
                        "Expected type alias name after 'type'".to_string()
                    );
                }
            }
            TokenTree::Ident(ident)
                if ident.to_string() == "fn"
                    || (ident.to_string() == "pub")
                    || (ident.to_string() == "async")
                    || (ident.to_string() == "const") =>
            {
                // Parse function signature
                let sig = parse_function_sig(&token, &mut iter)?;
                decls.push(TypeDecl::Function { sig });
            }
            _ => {
                // Skip other tokens for now
            }
        }
    }
    Ok(decls)
}

fn parse_struct_fields(tokens: TokenStream) -> Result<Vec<Field>, String> {
    let mut fields = Vec::new();
    let mut iter = tokens.into_iter().peekable();
    while let Some(token) = iter.next() {
        if let TokenTree::Ident(name) = &token {
            // Expect :
            if let Some(TokenTree::Punct(p)) = iter.next() {
                if p.as_char() == ':' {
                    // Collect type tokens until , or end
                    let mut ty = String::new();
                    while let Some(t) = iter.peek() {
                        match t {
                            TokenTree::Punct(p) if p.as_char() == ',' => {
                                iter.next();
                                break;
                            }
                            _ => {
                                ty.push_str(&token_to_string(
                                    iter.next().unwrap(),
                                ));
                            }
                        }
                    }
                    fields.push(Field {
                        name: name.to_string(),
                        ty: ty.trim().to_string(),
                    });
                } else {
                    return Err(format!(
                        "Expected ':' after field name '{}'",
                        name
                    ));
                }
            } else {
                return Err(format!(
                    "Expected ':' after field name '{}'",
                    name
                ));
            }
        }
    }
    Ok(fields)
}

fn parse_enum_variants(
    tokens: TokenStream,
) -> Result<Vec<EnumVariant>, String> {
    let mut variants = Vec::new();
    let mut iter = tokens.into_iter().peekable();
    while let Some(token) = iter.next() {
        if let TokenTree::Ident(name) = &token {
            // Check for tuple or struct variant
            if let Some(TokenTree::Group(group)) = iter.peek() {
                match group.delimiter() {
                    Delimiter::Parenthesis | Delimiter::Brace => {
                        let group =
                            if let Some(TokenTree::Group(g)) = iter.next() {
                                g
                            } else {
                                continue;
                            };
                        let fields = parse_struct_fields(group.stream())?;
                        variants.push(EnumVariant {
                            name: name.to_string(),
                            fields,
                        });
                        // Skip trailing comma if present
                        if let Some(TokenTree::Punct(p)) = iter.peek() {
                            if p.as_char() == ',' {
                                iter.next();
                            }
                        }
                        continue;
                    }
                    _ => {}
                }
            }
            // Unit variant
            variants.push(EnumVariant {
                name: name.to_string(),
                fields: Vec::new(),
            });
            // Skip trailing comma if present
            if let Some(TokenTree::Punct(p)) = iter.peek() {
                if p.as_char() == ',' {
                    iter.next();
                }
            }
        }
    }
    Ok(variants)
}

fn parse_function_sig(
    first_token: &TokenTree,
    iter: &mut iter::Peekable<impl Iterator<Item = TokenTree>>,
) -> Result<FunctionSig, String> {
    let mut vis = None;
    let mut is_async = false;
    let mut is_const = false;
    let mut name = None;

    // Handle pub/async/const/fn ordering
    let mut tokens = vec![first_token.clone()];
    for _ in 0..3 {
        if let Some(TokenTree::Ident(ident)) = iter.peek() {
            let s = ident.to_string();
            if s == "pub" || s == "async" || s == "const" || s == "fn" {
                tokens.push(iter.next().unwrap());
            } else {
                break;
            }
        }
    }

    let mut tokens_iter = tokens.into_iter().peekable();
    while let Some(token) = tokens_iter.next() {
        match &token {
            TokenTree::Ident(ident) if ident.to_string() == "pub" => {
                vis = Some("pub".to_string())
            }
            TokenTree::Ident(ident) if ident.to_string() == "async" => {
                is_async = true
            }
            TokenTree::Ident(ident) if ident.to_string() == "const" => {
                is_const = true
            }
            TokenTree::Ident(ident) if ident.to_string() == "fn" => {
                // Next token is function name
                if let Some(TokenTree::Ident(fname)) = iter.next() {
                    name = Some(fname.to_string());
                } else {
                    return Err(
                        "Expected function name after 'fn'".to_string()
                    );
                }
            }
            _ => {}
        }
    }
    let name =
        name.ok_or_else(|| "Expected function name after 'fn'".to_string())?;

    // Parse arguments
    let mut args = Vec::new();
    let mut ret = None;
    while let Some(token) = iter.next() {
        match &token {
            TokenTree::Group(group)
                if group.delimiter() == Delimiter::Parenthesis =>
            {
                args = parse_fn_args(group.stream())?;
            }
            TokenTree::Punct(p) if p.as_char() == '-' => {
                // Expect '->' for return type
                if let Some(TokenTree::Punct(p2)) = iter.next() {
                    if p2.as_char() == '>' {
                        // Collect type tokens until '{' or ';'
                        let mut ty = String::new();
                        while let Some(t) = iter.peek() {
                            match t {
                                TokenTree::Group(g)
                                    if g.delimiter() == Delimiter::Brace =>
                                {
                                    break
                                }
                                TokenTree::Punct(p) if p.as_char() == ';' => {
                                    break
                                }
                                _ => {
                                    ty.push_str(&token_to_string(
                                        iter.next().unwrap(),
                                    ));
                                }
                            }
                        }
                        ret = Some(ty.trim().to_string());
                    } else {
                        return Err("Expected '->' for function return type"
                            .to_string());
                    }
                } else {
                    return Err(
                        "Expected '->' for function return type".to_string()
                    );
                }
            }
            TokenTree::Group(group)
                if group.delimiter() == Delimiter::Brace =>
            {
                // Function body, skip
                break;
            }
            TokenTree::Punct(p) if p.as_char() == ';' => {
                // End of signature
                break;
            }
            _ => {}
        }
    }

    Ok(FunctionSig {
        vis,
        is_async,
        is_const,
        name,
        args,
        ret,
    })
}

fn parse_fn_args(tokens: TokenStream) -> Result<Vec<Field>, String> {
    let mut args = Vec::new();
    let mut iter = tokens.into_iter().peekable();
    while let Some(token) = iter.next() {
        if let TokenTree::Ident(name) = &token {
            // Expect :
            if let Some(TokenTree::Punct(p)) = iter.next() {
                if p.as_char() == ':' {
                    // Collect type tokens until , or end
                    let mut ty = String::new();
                    while let Some(t) = iter.peek() {
                        match t {
                            TokenTree::Punct(p) if p.as_char() == ',' => {
                                iter.next();
                                break;
                            }
                            _ => {
                                ty.push_str(&token_to_string(
                                    iter.next().unwrap(),
                                ));
                            }
                        }
                    }
                    args.push(Field {
                        name: name.to_string(),
                        ty: ty.trim().to_string(),
                    });
                } else {
                    return Err(format!(
                        "Expected ':' after argument name '{}'",
                        name
                    ));
                }
            } else {
                return Err(format!(
                    "Expected ':' after argument name '{}'",
                    name
                ));
            }
        }
    }
    Ok(args)
}

fn token_to_string(token: TokenTree) -> String {
    match token {
        TokenTree::Ident(ident) => ident.to_string(),
        TokenTree::Punct(p) => p.to_string(),
        TokenTree::Literal(lit) => lit.to_string(),
        TokenTree::Group(_) => String::from("<group>"),
    }
}

#[cfg(test)]
mod tests {
    use super::token_stream::{Delimiter, Group, TokenStream, TokenTree};
    use super::*;

    #[test]
    fn parses_basic_struct() {
        let tokens = "struct Foo { x: i32 }".parse::<TokenStream>().unwrap();
        let decls = parse_type_decls(tokens).expect("Should parse");
        assert_eq!(
            decls,
            vec![TypeDecl::Struct {
                name: "Foo".into(),
                fields: vec![Field {
                    name: "x".into(),
                    ty: "i32".into()
                }]
            }]
        );
    }

    #[test]
    fn parses_generic_struct() {
        let tokens = "struct Foo<T> { x: T }".parse::<TokenStream>().unwrap();
        let decls = parse_type_decls(tokens).expect("Should parse");
        assert_eq!(
            decls,
            vec![TypeDecl::Struct {
                name: "Foo".into(),
                fields: vec![Field {
                    name: "x".into(),
                    ty: "T".into()
                }]
            }]
        );
    }
}
