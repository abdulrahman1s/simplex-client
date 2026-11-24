//! SMP client, protocol v15 to v23: addresses, commands, responses, handshake with block encryption, and a reconnecting connection.
//! Ported from simplexmq Protocol.hs, Transport.hs, Client.hs (AGPL-3.0-only).

mod address;
mod auth;
mod block;
mod clock;
mod commands;
mod config;
#[cfg(feature = "native")]
mod connection;
mod error;
mod events;
mod flow;
mod handshake;
mod hello;
mod message;
mod pool;
pub mod proxy;
mod queue_data;
mod responses;
mod server_error;
mod session;
mod signed_key;
#[cfg(feature = "native")]
mod tls_connector;
mod transmission;
mod transport;
mod tunnel;
mod version;

#[cfg(test)]
#[cfg(feature = "native")]
pub(crate) mod tests;

pub use address::{DEFAULT_PORT, SmpServer};
pub use auth::Authorizer;
pub use block::{RcvBlocks, SndBlocks, block_ciphers, max_payload};
pub use clock::{Clock, SystemClock, system_wall_clock};
pub use commands::{Command, NewQueue};
pub use config::ConnConfig;
#[cfg(feature = "native")]
pub use connection::SmpConnection;
pub use error::{HandshakeError, SmpError};
pub use events::{SessionInfo, SmpEvent, SmpEventKind};
pub use flow::{Answer, Backoff, Request};
pub use handshake::{HandshakeConfig, TlsFacts, answer_server_hello, client_handshake, negotiate};
pub use hello::{ServerHello, encode_client_hello};
pub use message::{RcvMsg, decrypt_message, parse_rcv_body, rcv_dh_secret};
pub use pool::{Carrier, Dialer, MAX_PIPES, Pipe, PipeId, PipeInput, SharedPool, TunnelPool};
pub use proxy::{ProxiedRelay, ProxyClientError, ProxyError};
pub use queue_data::{
    ContactLink, MessagingLink, NewNtfCreds, QueueLinkData, QueueMode, QueueReqData,
    SubscriptionMode,
};
pub use responses::{QueueIds, Response, ServerNtfCreds};
pub use server_error::{CommandError, ErrorType};
pub use session::SmpSession;
#[cfg(feature = "native")]
pub use tls_connector::TlsConnector;
pub use transmission::{Incoming, authorized, encode_transmission, pack, parse_payload};
pub use transport::{Closer, Connector, Established, Reader, Wire, Writer};
pub use tunnel::{MAX_FRAME, TunnelError, parse_tunnel_reply, tunnel_hello};
pub use version::{BLOCK_SIZE, MAX_VERSION, MIN_VERSION};
