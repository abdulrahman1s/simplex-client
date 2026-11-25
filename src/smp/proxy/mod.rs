//! Private message routing: a relay of our own forwards sender commands to the destination relay, which sees only the proxy (Protocol.hs, Client.hs, simplexmq 7.1.4).
mod broker_error;
mod client_error;
#[cfg(any(test, feature = "testing"))]
#[cfg(feature = "native")]
#[allow(clippy::unwrap_used, reason = "test support: a broken fixture panics")]
pub(crate) mod destination;
mod error;
mod fwd;
mod nonce;
mod open;
mod relay;
mod seal;
mod transport_error;
pub(crate) mod wire;

pub use broker_error::BrokerError;
pub use client_error::ProxyClientError;
pub use error::ProxyError;
pub use fwd::{FwdResponse, FwdTransmission};
pub use nonce::{reverse_nonce, transmission_nonce};
pub use open::Proxied;
pub use relay::{ProxiedRelay, session_request};
pub use seal::{Opener, seal};
pub use transport_error::{Handshake, TransportError};
