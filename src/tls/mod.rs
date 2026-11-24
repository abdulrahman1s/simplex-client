//! A TLS 1.3 client for the SMP and XFTP transports (AGPL-3.0-only).

mod alert;
mod app_rx;
mod app_tx;
mod client;
mod der;
mod error;
mod handshake;
mod identity;
mod info;
mod key_schedule;
#[cfg(feature = "native")]
mod reader;
mod record;
#[cfg(feature = "native")]
mod shared;
#[cfg(feature = "native")]
mod stream;
mod verify;
mod wire;
#[cfg(feature = "native")]
mod writer;

#[cfg(test)]
#[cfg(feature = "native")]
pub(crate) mod testing;
#[cfg(test)]
#[cfg(feature = "native")]
mod tests;

pub use alert::Alert;
pub use client::TlsClient;
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
