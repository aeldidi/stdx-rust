use core::fmt;
use std::{
    alloc::{AllocError, Allocator, Global},
    error::Error,
    net::{Ipv4Addr, Ipv6Addr},
};

#[derive(Clone, Copy, Debug)]
pub enum ParseError {
    EmptyHost,
    InvalidIpv4Address,
    InvalidIpv6Address,
    IdnaError,
    InvalidPort,
    InvalidDomainCharacter,
    AllocError(AllocError),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::EmptyHost => write!(f, "empty host"),
            ParseError::InvalidIpv4Address => {
                write!(f, "invalid IPv4 address")
            }
            ParseError::InvalidIpv6Address => {
                write!(f, "invalid IPv6 address")
            }
            ParseError::IdnaError => {
                write!(f, "invalid international domain name")
            }
            ParseError::InvalidPort => write!(f, "invalid port number"),
            ParseError::InvalidDomainCharacter => {
                write!(f, "invalid character in domain name")
            }
            ParseError::AllocError(a) => write!(f, "{}", a),
        }
    }
}

impl Error for ParseError {}

#[derive(Clone, Copy, Debug)]
enum UrlHost {
    None,
    Domain,
    Ipv4(Ipv4Addr),
    Ipv6(Ipv6Addr),
}

#[derive(Clone, Debug)]
pub struct Url<A: Allocator = Global> {
    scheme: Vec<u8, A>,
    username: Option<Vec<u8, A>>,
    host: UrlHost,
    port: Option<u16>,
    path: Option<Vec<u8, A>>,
    query: Option<Vec<u8, A>>,
    fragment: Option<Vec<u8, A>>,
}

// impl<A: Allocator> Url<A> {
//     pub fn parse(input: &str) -> Self {}
// }
