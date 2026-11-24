//! A TLS 1.3 client for the SMP and XFTP transports (ADR-0042); ported from nook sx/tls (AGPL-3.0-only).

mod alert;
mod der;
mod error;
#[cfg(feature = "native")]
mod handshake;
mod identity;
mod info;
#[cfg(feature = "native")]
mod key_schedule;
#[cfg(feature = "native")]
mod reader;
#[cfg(feature = "native")]
mod record;
#[cfg(feature = "native")]
mod shared;
#[cfg(feature = "native")]
mod stream;
mod verify;
#[cfg(feature = "native")]
mod wire;
#[cfg(feature = "native")]
mod writer;

#[cfg(test)]
#[cfg(feature = "native")]
mod testing;
#[cfg(test)]
#[cfg(feature = "native")]
mod tests;

pub use alert::Alert;
pub use der::{CertError, certificate_public_key};
pub use error::TlsError;
pub use identity::{ChainCertificates, IdentityError, chain_id_ca_certs, verify_server_chain};
pub use info::TlsInfo;
#[cfg(feature = "native")]
pub use reader::TlsReader;
#[cfg(feature = "native")]
pub use stream::TlsStream;
#[cfg(feature = "native")]
pub use writer::TlsWriter;
