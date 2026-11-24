//! `TransportError` as a proxy reports it inside `BROKER TRANSPORT` (Transport.hs `instance Encoding TransportError`).
use std::fmt;

use crate::encoding::DecodeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportError {
    Block,
    Version,
    LargeMsg,
    Session,
    NoAuth,
    Handshake(Handshake),
}

/// Transport.hs `HandshakeError`, written with its Haskell `show`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handshake {
    Parse,
    Identity,
    BadAuth,
    BadService,
}

impl Handshake {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Parse => "PARSE",
            Self::Identity => "IDENTITY",
            Self::BadAuth => "BAD_AUTH",
            Self::BadService => "BAD_SERVICE",
        }
    }
}

impl TransportError {
    pub fn parse(bytes: &[u8]) -> Result<Self, DecodeError> {
        let bad = DecodeError::Invalid("TransportError");
        Ok(match bytes {
            b"BLOCK" => Self::Block,
            b"VERSION" => Self::Version,
            b"LARGE_MSG" => Self::LargeMsg,
            b"SESSION" => Self::Session,
            b"NO_AUTH" => Self::NoAuth,
            b"HANDSHAKE PARSE" => Self::Handshake(Handshake::Parse),
            b"HANDSHAKE IDENTITY" => Self::Handshake(Handshake::Identity),
            b"HANDSHAKE BAD_AUTH" => Self::Handshake(Handshake::BadAuth),
            b"HANDSHAKE BAD_SERVICE" => Self::Handshake(Handshake::BadService),
            _ => return Err(bad),
        })
    }
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Block => f.write_str("BLOCK"),
            Self::Version => f.write_str("VERSION"),
            Self::LargeMsg => f.write_str("LARGE_MSG"),
            Self::Session => f.write_str("SESSION"),
            Self::NoAuth => f.write_str("NO_AUTH"),
            Self::Handshake(h) => write!(f, "HANDSHAKE {}", h.as_str()),
        }
    }
}
