//! How a proxy itself failed a forwarded command (Client.hs `ProxyClientError`).
use std::fmt;

use crate::smp::server_error::ErrorType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyClientError {
    /// `ProxyProtocolError`: the proxy answered ERR, as `PROXY NO_SESSION` or a relay it could not reach.
    Protocol(ErrorType),
    /// `ProxyUnexpectedResponse`.
    Unexpected(String),
    /// `ProxyResponseError`: the proxy's answer did not parse.
    Response(ErrorType),
}

/// The `strEncode` text.
impl fmt::Display for ProxyClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(e) => write!(f, "PROTOCOL {e}"),
            Self::Unexpected(s) => write!(f, "UNEXPECTED {s}"),
            Self::Response(e) => write!(f, "SYNTAX {e}"),
        }
    }
}
