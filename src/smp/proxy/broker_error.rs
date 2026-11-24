//! What failed between a client, here a proxy, and a relay (Protocol.hs `instance Encoding BrokerErrorType`).
use std::fmt;

use crate::encoding::{Decode, DecodeError, Decoder, Encode, EncodeError};

use super::transport_error::{Handshake, TransportError};

/// `BrokerErrorType`: what failed between a client (here, the proxy) and a relay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerError {
    Response(String),
    Unexpected(String),
    /// The reference writes no detail yet, and drops any it reads.
    Network,
    Transport(TransportError),
    Timeout,
    Host,
    NoService,
}

impl BrokerError {
    /// `serverHostError`'s `brokerHostError`: the relay cannot be used this way at all.
    #[must_use]
    pub const fn is_host(&self) -> bool {
        matches!(self, Self::Host | Self::Transport(TransportError::Version))
    }

    /// `temporaryAgentError`'s `tempBrokerError`.
    #[must_use]
    pub const fn is_temporary(&self) -> bool {
        matches!(
            self,
            Self::Network
                | Self::Timeout
                | Self::Transport(TransportError::Handshake(Handshake::BadService))
        )
    }
}

pub(super) fn split(bytes: &[u8]) -> (&[u8], &[u8]) {
    let at = bytes.iter().position(|b| *b == b' ').unwrap_or(bytes.len());
    let (tag, rest) = bytes.split_at(at);
    (tag, rest.get(1..).unwrap_or_default())
}

pub(super) fn put(out: &mut Vec<u8>, tag: &[u8]) -> Result<(), EncodeError> {
    out.extend_from_slice(tag);
    Ok(())
}

/// A length-prefixed `String` that must use all of `bytes`.
fn whole_string(bytes: &[u8]) -> Result<String, DecodeError> {
    let mut d = Decoder::new(bytes);
    let s = String::decode(&mut d)?;
    d.finish()?;
    Ok(s)
}

impl BrokerError {
    pub fn parse(bytes: &[u8]) -> Result<Self, DecodeError> {
        let (tag, rest) = split(bytes);
        Ok(match tag {
            b"RESPONSE" => Self::Response(whole_string(rest)?),
            b"UNEXPECTED" => Self::Unexpected(whole_string(rest)?),
            b"TRANSPORT" => Self::Transport(TransportError::parse(rest)?),
            b"NETWORK" => Self::Network,
            b"TIMEOUT" => Self::Timeout,
            b"HOST" => Self::Host,
            b"NO_SERVICE" => Self::NoService,
            _ => return Err(DecodeError::Invalid("BrokerErrorType")),
        })
    }

    pub fn encode(&self, out: &mut Vec<u8>) -> Result<(), EncodeError> {
        match self {
            Self::Response(s) => {
                out.extend_from_slice(b"RESPONSE ");
                s.encode(out)
            }
            Self::Unexpected(s) => {
                out.extend_from_slice(b"UNEXPECTED ");
                s.encode(out)
            }
            Self::Transport(e) => {
                out.extend_from_slice(b"TRANSPORT ");
                out.extend_from_slice(e.to_string().as_bytes());
                Ok(())
            }
            Self::Network => put(out, b"NETWORK"),
            Self::Timeout => put(out, b"TIMEOUT"),
            Self::Host => put(out, b"HOST"),
            Self::NoService => put(out, b"NO_SERVICE"),
        }
    }
}

impl fmt::Display for BrokerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Response(s) => write!(f, "RESPONSE {s}"),
            Self::Unexpected(s) => write!(f, "UNEXPECTED {s}"),
            Self::Transport(e) => write!(f, "TRANSPORT {e}"),
            Self::Network => f.write_str("NETWORK"),
            Self::Timeout => f.write_str("TIMEOUT"),
            Self::Host => f.write_str("HOST"),
            Self::NoService => f.write_str("NO_SERVICE"),
        }
    }
}
