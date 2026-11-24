//! The `PROXY` payload of ERR (Protocol.hs `instance Encoding ProxyError`).
use std::fmt;

use crate::encoding::{DecodeError, EncodeError};

use super::broker_error::{BrokerError, put, split};
use crate::smp::server_error::ErrorType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyError {
    /// The destination relay answered ERR to the proxy.
    Protocol(Box<ErrorType>),
    /// The proxy could not talk to the destination relay.
    Broker(BrokerError),
    /// The basic auth given to the proxy is wrong.
    BasicAuth,
    /// The proxy has no session with the destination for this id.
    NoSession,
}

impl ProxyError {
    pub fn parse(bytes: &[u8]) -> Result<Self, DecodeError> {
        let (tag, rest) = split(bytes);
        Ok(match tag {
            b"PROTOCOL" => Self::Protocol(Box::new(ErrorType::parse(rest)?)),
            b"BROKER" => Self::Broker(BrokerError::parse(rest)?),
            b"BASIC_AUTH" => Self::BasicAuth,
            b"NO_SESSION" => Self::NoSession,
            _ => return Err(DecodeError::Invalid("ProxyError")),
        })
    }

    pub fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        match self {
            Self::Protocol(e) => {
                out.extend_from_slice(b"PROTOCOL ");
                e.encode(out)
            }
            Self::Broker(e) => {
                out.extend_from_slice(b"BROKER ");
                e.encode(out)
            }
            Self::BasicAuth => put(out, b"BASIC_AUTH"),
            Self::NoSession => put(out, b"NO_SESSION"),
        }
    }
}

/// The `strEncode` text, for logs and messages.
impl fmt::Display for ProxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(e) => write!(f, "PROTOCOL {e}"),
            Self::Broker(e) => write!(f, "BROKER {e}"),
            Self::BasicAuth => f.write_str("BASIC_AUTH"),
            Self::NoSession => f.write_str("NO_SESSION"),
        }
    }
}
